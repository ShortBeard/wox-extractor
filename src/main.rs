/***********
/
/ WOX (World Of Xeen) Extractor 1.0
/ Created by ShortBeard
/ Reference used: https://xeen.fandom.com/wiki/CC_File_Format
/
***********/

mod file_names;

use std::{
    fs::{self, File},
    io::{self, Read, Seek, Write},
    path::Path,
};

use byteorder::{LittleEndian, ReadBytesExt};
use file_names::GameType;
use std::collections::HashMap;
use std::env;
use std::io::Cursor;
use ux::u24;

//Table of content
#[derive(Default, Clone, std::fmt::Debug)]
struct TocItem {
    file_id: u16,
    file_offset: ux::u24, //Use the ux crate to get a custom u24 type
    file_length: u16,
    padding_byte: u8,  //Unused other than for checking decryption validity
    file_name: String, //Determined after we grab all the TOC and match the file ID to its name via a hardcoded hashmap
}

//Individual files referenced by the table of content
#[derive(Default, Clone, std::fmt::Debug)]
struct CcFile {
    file_name: String,
    file_bytes: Vec<u8>,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let input_file_path = &args[1];

    //let file_name: &str = "XEEN.CC";
    let file_name = input_file_path;
    let open_file_result: Result<File, io::Error> = open_cc_file(file_name);
    match open_file_result {
        Ok(mut file) => begin_extraction(&mut file, file_name),
        Err(err) => print!("Error opening file: {}", err),
    };
}

fn get_game_type(file_path: &str) -> GameType {
    let path = Path::new(file_path);
    let file_name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|s| s.to_string().to_lowercase());

    match file_name.as_deref() {
        Some("xeen") => GameType::CLOUDS,
        Some("dark") => GameType::DARKSIDE,
        Some("swrd") => GameType::SWORDS, //currently unsupported
        Some("intro") => GameType::INTRO,
        _ => GameType::UNKNOWN,
    }
}

fn open_cc_file(file_name: &str) -> Result<File, io::Error> {
    let f: File = File::open(file_name)?;
    Ok(f)
}

//Get the bytes from our file and return them as a u8 vector
fn read_bytes(file: &mut File) -> Vec<u8> {
    let mut file_buffer: Vec<u8> = Vec::new();
    let read_result = file.read_to_end(&mut file_buffer);
    match read_result {
        Ok(_) => file_buffer,
        Err(err) => {
            println!("Error while reading file: {} ", err);
            panic!()
        }
    }
}

//Begin extracting the resources
fn begin_extraction(file: &mut File, file_name: &str) {
    let file_buffer: Vec<u8> = read_bytes(file);
    let mut file_cursor: std::io::Cursor<Vec<u8>> = Cursor::new(file_buffer);
    let file_count: u16 = file_cursor.read_u16::<LittleEndian>().unwrap();

    //table of contents is encrypted using XOR algorithm
    let mut table_of_contents_buffer: Vec<u8> = vec![0; (file_count * 8) as usize];
    file_cursor
        .read_exact(&mut table_of_contents_buffer)
        .unwrap();

    //Decrypt the table of contents
    table_of_contents_buffer = decrypt_toc(table_of_contents_buffer);

    //Ensure decrypt was successful by checking if every decrypted 8th byte is a 0
    let decrypt_success = verify_decrypt(&table_of_contents_buffer);
    //let decrypt_success = true;
    match decrypt_success {
        true => {
            println!("Decryption successful.");
            let game_type: GameType = get_game_type(&file_name);
            let toc_items: Vec<TocItem> = read_toc(file_count, table_of_contents_buffer, game_type);
            //set_file_names(&mut toc_items); //Sets file names based on their IDs
            let extract_dir: String = setup_extract_location(file_name);
            let cc_files = extract_files(file_cursor, toc_items);
            let decrypted_cc_files = decrypt_files(cc_files);
            save_files(decrypted_cc_files, extract_dir);
        }
        false => {
            println!("There was an issue while decrypting the file.");
            panic!()
        }
    }
}

//Decypt the table of contents
fn decrypt_toc(mut table_of_content: Vec<u8>) -> Vec<u8> {
    let mut counter_byte = 0xac; //Counter byte always initialized to 0xac to begin decryption
    for i in 0..table_of_content.len() {
        let el = table_of_content[i];
        //Use wrapping_add to avoid overflow errors
        table_of_content[i] = ((el << 2 | el >> 6).wrapping_add(counter_byte)) & 0xff;
        counter_byte = counter_byte.wrapping_add(0x67);
    }

    table_of_content
}

//Ensures the decryption was successful by checking if every 8th byte is a 0.
//Returns true if decrpytion was successful
fn verify_decrypt(table_of_contents: &Vec<u8>) -> bool {
    let mut i = 7; //Start at 8th byte
    while i < table_of_contents.len() {
        if table_of_contents[i] != 0 {
            return false;
        }
        i += 8;
    }
    true
}

//Read the decrypted table of contents
fn read_toc(file_count: u16, table_of_content: Vec<u8>, game_type: GameType) -> Vec<TocItem> {
    let file_name_map: HashMap<u32, String> = file_names::get_file_names(game_type);
    let mut toc_items: Vec<TocItem> = vec![TocItem::default(); file_count as usize];
    let mut file_cursor: std::io::Cursor<Vec<u8>> = Cursor::new(table_of_content);
    let mut i: u16 = 0;
    while i < file_count {
        toc_items[i as usize].file_id = file_cursor.read_u16::<LittleEndian>().unwrap();
        toc_items[i as usize].file_offset =
            u24::new(file_cursor.read_u24::<LittleEndian>().unwrap());
        toc_items[i as usize].file_length = file_cursor.read_u16::<LittleEndian>().unwrap();
        toc_items[i as usize].padding_byte = file_cursor.read_u8().unwrap();
        toc_items[i as usize].file_name =
            get_file_name(toc_items[i as usize].file_id, &file_name_map);
        i += 1
    }

    toc_items
}

fn setup_extract_location(file_name: &str) -> String {
    //Create a new directory to place extracted files into. Use filename as template, remove extension:
    let folder_name: String = String::from(file_name.to_string().replace(".CC", "") + "_extracted");
    let folder_exists: bool = Path::new(&folder_name).exists();

    if folder_exists {
        println!("Wil just use existing folder");
    } else {
        let folder_result = create_extraction_folder(&folder_name);
        match folder_result {
            Ok(folder_name) => {
                println!("{}", folder_name);
            }
            Err(err) => {
                println!(
                    "There was an error while attempting to create the extraction folder: {}",
                    err
                );
            }
        }
    }

    folder_name
}

fn extract_files(
    mut file_cursor: std::io::Cursor<Vec<u8>>,
    toc_items: Vec<TocItem>,
) -> Vec<CcFile> {
    // println!("Now extracting to: {}", directory);
    let mut cc_files: Vec<CcFile> = vec![CcFile::default(); toc_items.len()];

    //let mut file_data: Vec<u8> = vec![0; toc_items.file_length as usize];
    for i in 0..toc_items.len() {
        let mut file_data: Vec<u8> = vec![0; toc_items[i].file_length as usize];
        let seek_result =
            file_cursor.seek(io::SeekFrom::Start(u64::from(toc_items[i].file_offset)));
        match seek_result {
            Ok(_) => match file_cursor.read_exact(&mut file_data) {
                Ok(_) => {
                    cc_files[i].file_name = toc_items[i].file_name.clone();
                    cc_files[i].file_bytes = file_data;
                }
                Err(_) => {
                    println!(
                        "Error while attempting to read {} bytes from position {}",
                        toc_items[i].file_length, toc_items[i].file_offset
                    );
                }
            },
            Err(_) => {
                println!(
                    "Error while attempting to seek to position: {}",
                    toc_items[i].file_offset
                );
            }
        }
    }

    cc_files
}

//Decrypts the files themselves that the table of content points to.
fn decrypt_files(mut cc_files: Vec<CcFile>) -> Vec<CcFile> {
    for cc_file in &mut cc_files {
        for byte in &mut cc_file.file_bytes {
            *byte ^= 0x35; //Decrypt the specific file
        }
    }

    cc_files
}

fn save_files(cc_files: Vec<CcFile>, directory: String) {
    for cc_file in cc_files {
        let file = File::create("./".to_owned() + &directory + "/" + &cc_file.file_name);
        match file {
            Ok(mut f) => match f.write_all(&cc_file.file_bytes) {
                Ok(_) => {}
                Err(err) => {
                    println!(
                        "Error '{}' while attempting to create file: {} ",
                        err, cc_file.file_name
                    );
                }
            },
            Err(err) => {
                println!("Error when creating file: {}", err);
            }
        }
    }
}

fn create_extraction_folder(folder_name: &str) -> Result<String, io::Error> {
    //let folder_name: String = String::from(file_name.to_string().replace(".CC", "") + "_extracted");
    fs::create_dir(&folder_name)?;
    Ok(folder_name.to_owned())
}

fn get_file_name(file_id: u16, file_names: &HashMap<u32, String>) -> String {
    let found_value = file_names.get_key_value(&(file_id as u32));
    match found_value {
        Some((_file_id, file_name)) => file_name.to_string(),
        None => {
            println!("WARNING: Unknown file name for ID {}", file_id);
            String::from("UNKNOWN_FILE") + &file_id.to_string()
        } //Create an UNKNOWN_FILE in the event that the name can't be matched from the hashmap
    }
}
