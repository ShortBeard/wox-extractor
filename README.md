# wox-extractor
A resource extractor for MM4-5: World of Xeen written in Rust

<h3>Building:</h3>
1. Clone repo</br>
2. Run <strong>cargo build --release</strong></br>
3. Executable will be found in <strong>target</strong> directory</br>

<h3>Usage:</h3>
1. Run <strong>.\wox-extractor XEEN.CC</strong> (or whichever CC file you want to extract)</br>
2. An extracted folder will be created in the same directory as the .CC file which contains all extracted resource files.</br>
</br>
<p>Please note that though this utility technically does support Swords of Xeen (SWRD.CC) it does not know the names and formats of the output files, so they will all be named as "unknown" with their ID.</p>
