use anyhow::Result;
use bittorrust::{peer::Peer, torrent::Torrent, tracker::TrackerRequest};
use std::net::{Ipv4Addr, SocketAddrV4};

#[tokio::main]
async fn main() -> Result<()> {
    let decoded_torrent = Torrent::new("sample.torrent").await;
    let info_hash = decoded_torrent.info_hash();
    let mut piece_hashes = Vec::new();
    for hash in decoded_torrent.info.pieces.chunks(20) {
        piece_hashes.push(hex::encode(hash));
    }

    let peer_id = "00112233445566778899";
    let port = 6881;
    let uploaded = 0;
    let downloaded = 0;
    let left = decoded_torrent.info.length.unwrap();
    let compact = 1;

    let req = TrackerRequest {
        info_hash,
        peer_id: peer_id.into(),
        port,
        uploaded,
        downloaded,
        left,
        compact,
    };

    let info_hash_url = TrackerRequest::url_encode(info_hash);
    let tracker_response =
        TrackerRequest::request(&req, info_hash_url, &decoded_torrent.announce).await;
    let peers = tracker_response
        .peers
        .chunks(6)
        .map(|chunk| {
            let ip = Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]);
            let port = u16::from_be_bytes([chunk[4], chunk[5]]);
            SocketAddrV4::new(ip, port)
        })
        .collect::<Vec<_>>();
    let peer = Peer { socket: peers[0] };
    let stream = Peer::handshake(peer, info_hash).await;

    // Downloading a piece
    Peer::download_torrent(stream, decoded_torrent, piece_hashes).await;
    Ok(())
}
