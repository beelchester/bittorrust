use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddrV4},
};

use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use urlencoding::encode_binary;

use crate::torrent::{self, Torrent};

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

impl TrackerRequest {
    pub fn new(torrent: &Torrent, info_hash: [u8; 20]) -> TrackerRequest {
        let peer_id = "00112233445566778899";
        let port = 6881;
        let uploaded = 0;
        let downloaded = 0;
        let left = torrent.info.length.unwrap();
        let compact = 1;

        TrackerRequest {
            info_hash,
            peer_id: peer_id.into(),
            port,
            uploaded,
            downloaded,
            left,
            compact,
        }
    }
    pub fn url_encode(info_hash: [u8; 20]) -> String {
        let info_hash_url_encoded = encode_binary(&info_hash).into_owned();
        info_hash_url_encoded
    }
    pub async fn request(&self, info_hash_url: String, tracker_url: &String) -> TrackerResponse {
        let mut params = HashMap::new();

        // Define other parameters
        // let event = "started";

        // Create a hashmap for the query parameters
        params.insert("peer_id", self.peer_id.to_string());
        params.insert("port", self.port.to_string());
        params.insert("uploaded", self.uploaded.to_string());
        params.insert("downloaded", self.downloaded.to_string());
        params.insert("left", self.left.to_string());
        params.insert("compact", self.compact.to_string());
        // params.insert("event", event.to_string());

        let mut paramstr = String::new();
        params.iter().for_each(|(x, y)| {
            paramstr.push_str(&format!("&{}={}", x, y));
        });

        let f = format!("{}?info_hash={}{}", tracker_url, info_hash_url, paramstr);

        println!("request: {}", f.clone());

        let body = reqwest::get(f).await.unwrap().bytes().await.unwrap();
        println!("body: {:?}", body);
        serde_bencode::from_bytes::<TrackerResponse>(&body).unwrap()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TrackerResponse {
    pub interval: u64,
    pub peers: ByteBuf,
}

impl TrackerResponse {
    pub fn get_peers(&self) -> Vec<SocketAddrV4> {
        self.peers
            .chunks(6)
            .map(|chunk| {
                let ip = Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]);
                let port = u16::from_be_bytes([chunk[4], chunk[5]]);
                SocketAddrV4::new(ip, port)
            })
            .collect::<Vec<_>>()
    }
}
