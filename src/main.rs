use anyhow::Result;
use bittorrust::{peer::Peer, torrent::Torrent, tracker::TrackerRequest};
use clap::{command, Parser, Subcommand};
use std::{
    collections::HashSet,
    net::{Ipv4Addr, SocketAddrV4},
    path::PathBuf,
    sync::Arc,
};
use tokio::{stream, sync::Mutex};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
#[clap(rename_all = "snake_case")]
enum Command {
    Decode {
        value: String,
    },
    Info {
        torrent: PathBuf,
    },
    Peers {
        torrent: PathBuf,
    },
    Handshake {
        torrent: PathBuf,
        // peer: String,
    },
    DownloadPiece {
        #[arg(short)]
        output: PathBuf,
        torrent: PathBuf,
        piece: u32,
    },
    Download {
        #[arg(short)]
        output: PathBuf,
        torrent: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Decode { value } => {
            let (decoded, _) = bittorrust::bencode_parser::decode(&value);
            println!("{}", decoded);
        }
        Command::Info { torrent } => {
            let decoded_torrent = Torrent::new(torrent).await;
            println!("Tracker URL: {}", decoded_torrent.announce);
            println!("Length: {}", decoded_torrent.info.length.unwrap());
            let info_hash = decoded_torrent.info_hash();
            println!("Info Hash: {}", hex::encode(info_hash));
            println!("Piece Length: {}", decoded_torrent.info.piece_length);
            println!("Piece Hashes: {:?}", decoded_torrent.get_piece_hashes());
        }
        Command::Peers { torrent } => {
            let decoded_torrent = Torrent::new(torrent).await;
            let info_hash = decoded_torrent.info_hash();
            let info_hash_url = TrackerRequest::url_encode(info_hash);
            let req = TrackerRequest::new(&decoded_torrent, info_hash);
            let tracker_response =
                TrackerRequest::request(&req, info_hash_url, &decoded_torrent.announce).await;
            let peers = tracker_response.get_peers();
            println!("peers: {:?}", peers);
        }
        Command::Handshake { torrent } => {
            //TODO: get peer socket from args
            // $ ./your_bittorrent.sh handshake sample.torrent <peer_ip>:<peer_port>
            let decoded_torrent = Torrent::new(torrent).await;
            let info_hash = decoded_torrent.info_hash();
            let info_hash_url = TrackerRequest::url_encode(info_hash);
            let req = TrackerRequest::new(&decoded_torrent, info_hash);
            let tracker_response =
                TrackerRequest::request(&req, info_hash_url, &decoded_torrent.announce).await;
            let peers = tracker_response.get_peers();
            let peer = Peer { socket: peers[0] };
            let _ = Peer::handshake(peer, info_hash).await;
        }
        Command::DownloadPiece {
            output,
            torrent,
            piece,
        } => {
            let decoded_torrent = Torrent::new(torrent).await;
            let info_hash = decoded_torrent.info_hash();
            let info_hash_url = TrackerRequest::url_encode(info_hash);
            let piece_hashes = decoded_torrent.get_piece_hashes();
            let req = TrackerRequest::new(&decoded_torrent, info_hash);
            let tracker_response =
                TrackerRequest::request(&req, info_hash_url, &decoded_torrent.announce).await;
            let peers = tracker_response.get_peers();
            let peer = Peer { socket: peers[0] };
            let stream = Arc::new(Mutex::new(Peer::handshake(peer, info_hash).await));
            let pending_tasks: Arc<Mutex<HashSet<u64>>> = Arc::new(Mutex::new(HashSet::new()));
            Peer::download_piece(
                stream,
                piece,
                decoded_torrent.info.piece_length,
                pending_tasks,
                piece_hashes.get(piece as usize).unwrap().to_string(),
                None,
                output,
            )
            .await;
        }
        Command::Download { output, torrent } => {
            let decoded_torrent = Torrent::new(torrent).await;
            let info_hash = decoded_torrent.info_hash();
            let piece_hashes = decoded_torrent.get_piece_hashes();
            let req = TrackerRequest::new(&decoded_torrent, info_hash);
            let info_hash_url = TrackerRequest::url_encode(info_hash);
            let tracker_response =
                TrackerRequest::request(&req, info_hash_url, &decoded_torrent.announce).await;
            let peers = tracker_response.get_peers();
            let peer = Peer { socket: peers[0] };
            let stream = Peer::handshake(peer, info_hash).await;
            Peer::download_torrent(stream, decoded_torrent, piece_hashes, output, None).await;
        }
    };
    Ok(())
}
