use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct File {
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
    pub files: Option<Vec<File>>,
    // #[serde(default)]
    // pub path: Option<Vec<String>>,
    // #[serde(default)]
    // #[serde(rename = "root hash")]
    // pub root_hash: Option<String>,
}

#[allow(dead_code)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TrackerResponse {
    pub interval: u64,
    pub peers: ByteBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Peer {
    pub ip: String,
    pub port: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TrackerRequest {
    pub info_hash: [u8; 20],
    pub peer_id: String,
    pub port: u16,
    pub uploaded: usize,
    pub downloaded: usize,
    pub left: usize,
    pub compact: u8,
}
