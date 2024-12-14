pub mod bencode_parser;
pub mod peer;
pub mod torrent;
pub mod tracker;

pub const MAX_CONCURRENT_REQUESTS: usize = 5;
pub const DEFAULT_BLOCK_LENGTH: u32 = 16 * 1024;
