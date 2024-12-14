use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use sha1::{Digest, Sha1};
use tokio::{fs::File, io::AsyncReadExt};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Torrent {
    /// URL of the tracker
    pub announce: String,
    /// Information about the file(s) being shared
    pub info: Info,
    /// List of lists of URLs
    /// If available, then `announce` is ignored
    /// [spec](http://bittorrent.org/beps/bep_0012.html)
    #[serde(default)]
    #[serde(rename = "announce-list")]
    pub announce_list: Option<Vec<Vec<String>>>,
    /// Comment about the torrent by the creator
    #[serde(default)]
    pub comment: Option<String>,
    /// Name and version of the program used to create the .torrent
    #[serde(default)]
    #[serde(rename = "created by")]
    pub created_by: Option<String>,
    /// Creation date of the torrent
    #[serde(default)]
    #[serde(rename = "creation date")]
    pub creation_date: Option<u64>,
    /// Encoding used for the fields in the torrent
    #[serde(default)]
    pub encoding: Option<String>,
}

impl Torrent {
    pub async fn new(file: PathBuf) -> Torrent {
        let file = File::open(file).await.unwrap();
        let mut buf_reader = tokio::io::BufReader::new(file);
        let mut bytes = Vec::new();
        buf_reader.read_to_end(&mut bytes).await.unwrap();
        serde_bencode::from_bytes::<Torrent>(&bytes).unwrap()
    }
    pub fn info_hash(&self) -> [u8; 20] {
        let bencoded_info = serde_bencode::to_bytes(&self.info).unwrap();
        let info_hash = Sha1::digest(bencoded_info.clone());
        info_hash.into()
    }
    pub fn get_piece_hashes(&self) -> Vec<String> {
        let mut piece_hashes = Vec::new();
        for hash in self.info.pieces.chunks(20) {
            piece_hashes.push(hex::encode(hash));
        }
        piece_hashes
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct TorrentFile {
    /// Path of the file, as a list of strings
    pub path: Vec<String>,
    /// Length of the file in bytes
    pub length: i64,
    /// A 32-character hexadecimal string corresponding to the MD5 sum of the file
    #[serde(default)]
    pub md5sum: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Info {
    /// Name of the file (for single file) or directory (for multi-file)
    pub name: String,
    /// Length of each piece (Common to both mode)
    #[serde(rename = "piece length")]
    pub piece_length: u32,
    /// Concatenation of all 20-byte SHA1 hash values, one per piece (common to both mode)
    pub pieces: ByteBuf,
    // #[serde(default)]
    // pub private: Option<u8>,
    /// Single-file:
    /// A 32-character hexadecimal string corresponding to the MD5 sum of the file
    #[serde(default)]
    pub md5sum: Option<String>,
    /// Single-file:
    /// Length of the file in bytes
    #[serde(default)]
    pub length: Option<usize>,
    #[serde(default)]
    /// Multi-file:
    /// List of files
    pub files: Option<Vec<TorrentFile>>,
    // #[serde(default)]
    // pub path: Option<Vec<String>>,
    // #[serde(default)]
    // #[serde(rename = "root hash")]
    // pub root_hash: Option<String>,
}
