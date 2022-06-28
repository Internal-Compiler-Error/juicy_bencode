# juicy_bencode
A little parser for [bencode](https://www.bittorrent.org/beps/bep_0003.html#bencoding) using the Nom library. **Nom eats input 
byte by bytes, and bencode is such juicy input!**

The crate provides both more individual parses for parsing out individual bencode items or just a blob.

# TL; DR
You have a bencoded blob containing the torrent information for totally legal files, 

```rust
// pub enum BencodeItemView<'a> {
//     Integer(i64),
//     ByteString(&'a [u8]),
//     List(Vec<BencodeItemView<'a>>),
//     Dictionary(BTreeMap<&'a [u8], BencodeItemView<'a>>),
// }

use juicy_bencode::parse_bencode_dict;
fn main () -> Result<(), Box<dyn Error>>{
    // the library uses byte slices
    let text: &[u8] = input();
    // now you can do totally legal things with the info!
    let parsed_tree: BencodeItemView = parse_bencode_dict(text)?;
}

```
