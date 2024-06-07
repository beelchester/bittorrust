use sha1::{Digest, Sha1};
use std::{
    collections::{HashMap, HashSet},
    net::{Ipv4Addr, SocketAddrV4},
    sync::Arc,
};
use tokio::{
    fs::{File, OpenOptions},
    task,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Mutex,
};
use tokio::{net::TcpStream, time::sleep};
use urlencoding::encode_binary;
mod torrent;
use torrent::Torrent;

use crate::torrent::TrackerResponse;

const MAX_CONCURRENT_REQUESTS: usize = 5;
const DEFAULT_BLOCK_LENGTH: u32 = 16 * 1024;

async fn handshake(addr: SocketAddrV4, info_hash: [u8; 20]) -> TcpStream {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let peer_id: [u8; 20] = [0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9];
    let mut buffer: Vec<u8> = Vec::with_capacity(68);

    buffer.push(19);
    buffer.extend("BitTorrent protocol".as_bytes());
    buffer.extend(&[0_u8; 8]);
    buffer.extend(&info_hash);
    buffer.extend(&peer_id);
    stream.write_all(&buffer).await.unwrap();
    stream.read_exact(&mut buffer).await.unwrap();
    let peer_id = &buffer[48..];
    let phex = hex::encode(peer_id);
    println!("hex Peer ID: {}", phex);
    stream
}

//TODO: refactor arguments
async fn request_block(
    stream: Arc<Mutex<TcpStream>>,
    piece_index: u32,
    piece_length: u32,
    offset: u32,
    block_task_id: u64,
    current_block_tasks: Arc<Mutex<HashSet<u64>>>,
    download_piece_buf: Arc<Mutex<HashMap<u32, Vec<u8>>>>,
    block_index: u32,
) {
    println!(
        "--------------- started b {}, for {}",
        block_task_id, piece_index
    );
    // if block_index == 0 {
    //     println!("huge sleep for b {}", block_task_id);
    //     sleep(tokio::time::Duration::from_millis(1000)).await;
    // }
    let mut stream = stream.lock().await;
    println!("{},{},{}", piece_length, offset, DEFAULT_BLOCK_LENGTH);
    // This'll be 2^14 (16 * 1024) for all blocks except the last one.
    let block_length = if piece_length - offset > DEFAULT_BLOCK_LENGTH {
        DEFAULT_BLOCK_LENGTH
    } else {
        // For the last block
        println!("last b");
        piece_length - offset
    };
    println!("final b {}", block_length);
    let mut request_message = vec![0; 17];
    request_message[0..4].copy_from_slice(&(13u32.to_be_bytes()));
    request_message[4] = 6;
    request_message[5..9].copy_from_slice(&(piece_index.to_be_bytes()));
    request_message[9..13].copy_from_slice(&(offset.to_be_bytes()));
    request_message[13..17].copy_from_slice(&(block_length.to_be_bytes()));
    println!("{:?}", request_message);
    stream.write_all(&request_message).await.unwrap();

    // Recieve piece data
    let mut piece_header = vec![0; 13];
    stream.read_exact(&mut piece_header).await.unwrap();
    println!(
        "ph is {:?}",
        u32::from_be_bytes(piece_header[0..4].try_into().unwrap())
    );
    assert_eq!(piece_header[4], 7);
    let mut block_data = vec![0; block_length as usize];
    stream.read_exact(&mut block_data).await.unwrap();
    println!("bl len is {:?}", block_data.len());
    println!("off is {}", offset);

    let mut d_buf = download_piece_buf.lock().await;
    d_buf.insert(block_index, block_data);
    println!("dp of {piece_index} is {:?}", d_buf.len());

    let mut temp_current_block_tasks = current_block_tasks.lock().await;
    println!("pending tasks: {:?}", temp_current_block_tasks);
    temp_current_block_tasks.remove(&block_task_id);
    println!("pending tasks: {:?}", temp_current_block_tasks);
    println!(
        "--------------- ended b {}, for {}",
        block_task_id, piece_index
    );
}

async fn download_piece(
    stream: Arc<Mutex<TcpStream>>,
    piece_index: u32,
    piece_length: u32,
    current_block_tasks: Arc<Mutex<HashSet<u64>>>,
) {
    println!("***** started piece download {}", piece_index);
    // sending peer details request
    // dividing pieces into blocks
    let mut offset: u32 = 0;
    let mut tasks = vec![];
    let download_piece_buf = Arc::new(Mutex::new(HashMap::new()));
    let mut block_index = 0;

    while offset < piece_length {
        let block_task_id = rand::random();
        let mut temp_current_block_tasks = current_block_tasks.lock().await;
        while temp_current_block_tasks.len() >= MAX_CONCURRENT_REQUESTS {
            println!("max reached b waiting {}", block_task_id);
            drop(temp_current_block_tasks);
            sleep(tokio::time::Duration::from_millis(100)).await;
            temp_current_block_tasks = current_block_tasks.lock().await;
        }
        temp_current_block_tasks.insert(block_task_id);
        drop(temp_current_block_tasks);
        let task = task::spawn(request_block(
            stream.clone(),
            piece_index,
            piece_length,
            offset,
            block_task_id,
            current_block_tasks.clone(),
            download_piece_buf.clone(),
            block_index,
        ));
        tasks.push(task);
        offset += DEFAULT_BLOCK_LENGTH;
        block_index += 1;
    }
    for task in tasks {
        task.await.unwrap();
    }
    let piece_data = download_piece_buf.lock().await;
    let mut o = 0;
    loop {
        if o == block_index {
            break;
        }
        let data = piece_data.get(&o).unwrap();
        let output_path = format!("test-piece-{}", piece_index);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(output_path)
            .await
            .expect("Failed to open file");

        file.write_all(data).await.expect("Failed to write to file");
        o += 1;
    }
    println!("file written from piece");
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    let file = File::open("sample.torrent").await.unwrap();
    let mut buf_reader = tokio::io::BufReader::new(file);
    let mut bytes = Vec::new();
    buf_reader.read_to_end(&mut bytes).await.unwrap();
    let decoded = serde_bencode::from_bytes::<Torrent>(&bytes).unwrap();
    println!("decoded torrent {:?}", decoded);
    let bencoded_info = serde_bencode::to_bytes(&decoded.info).unwrap();
    let info_hash = Sha1::digest(bencoded_info.clone());

    println!("info hash {:?}", hex::encode(info_hash));
    println!("piece length {:?}", decoded.info.piece_length);
    println!("length {:?}", decoded.info.length);
    println!("tracker url {:?}", decoded.announce);
    println!("total pieces {:?}", decoded.info.pieces.len());
    println!("piece hashes:");
    for hash in decoded.info.pieces.chunks(20) {
        println!("{:?}", hex::encode(hash));
    }
    let info_hash_encoded = encode_binary(&info_hash).into_owned();
    println!("{}", info_hash_encoded);
    let tracker_url = decoded.announce.as_str();

    let peer_id = "00112233445566778899";
    let port = 6881;
    let uploaded = 0;
    let downloaded = 0;
    let left = decoded.info.length.unwrap();
    let compact = 1;

    // let req = TrackerRequest {
    //     info_hash: info_hash.into(),
    //     peer_id: peer_id.into(),
    //     port,
    //     uploaded,
    //     downloaded,
    //     left,
    //     compact,
    // };
    //
    let mut params = HashMap::new();

    // Define other parameters
    // let event = "started";

    // Create a hashmap for the query parameters
    params.insert("peer_id", peer_id.to_string());
    params.insert("port", port.to_string());
    params.insert("uploaded", uploaded.to_string());
    params.insert("downloaded", downloaded.to_string());
    params.insert("left", left.to_string());
    params.insert("compact", compact.to_string());
    // params.insert("event", event.to_string());

    let mut paramstr = String::new();
    params.iter().for_each(|(x, y)| {
        paramstr.push_str(&format!("&{}={}", x, y));
    });

    let f = format!(
        "{}?info_hash={}{}",
        tracker_url, info_hash_encoded, paramstr
    );

    println!("request is {}", f.clone());

    let body = reqwest::get(f).await.unwrap().bytes().await.unwrap();
    println!("body is {:?}", body);
    let res_decoded = serde_bencode::from_bytes::<TrackerResponse>(&body).unwrap();
    println!("response is {:?}", res_decoded);
    let ip_port_pairs = res_decoded
        .peers
        .chunks(6)
        .map(|chunk| {
            let ip = Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]);
            let port = u16::from_be_bytes([chunk[4], chunk[5]]);
            SocketAddrV4::new(ip, port)
        })
        .collect::<Vec<_>>();

    // for ip_port in ip_port_pairs {
    //     println!("{}", ip_port);
    // }
    //
    let mut stream = handshake(ip_port_pairs[0], info_hash.into()).await;

    // Downloading a piece

    // getting the bitfield
    // getting bitfield message prefix that indicates the total size of the bitfield message
    // excluding itself(4 bytes)
    let mut bitfield_len_buf = [0; 4];
    stream.read_exact(&mut bitfield_len_buf).await.unwrap();
    let bf = u32::from_be_bytes(bitfield_len_buf) as usize;
    println!("bitfield len gave {}", bf);
    println!("bitfield len buf is {:?}", bitfield_len_buf);

    if bf == 0 {
        println!("No bitfield message received.");
    }

    // getting the message id

    let mut bitfield_msg_buf = vec![0; bf];
    stream.read_exact(&mut bitfield_msg_buf).await.unwrap();
    println!("bitfield msg buf is {:?}", bitfield_msg_buf);
    if bitfield_msg_buf[0] != 5 {
        panic!(
            "Expected bitfield message, but got message ID: {}",
            bitfield_msg_buf[0]
        );
    }
    let _bitfield = &bitfield_msg_buf[1..];

    // sending interested message
    let interested_message = [0, 0, 0, 1, 2];
    stream.write_all(&interested_message).await.unwrap();

    // receive unchoke message, no payload

    let mut unchoke_buf = vec![0; 5];
    stream.read_exact(&mut unchoke_buf).await.unwrap();
    println!("unchoke msg buf is {:?}", unchoke_buf);
    if unchoke_buf[4] != 1 {
        panic!(
            "Expected unchoke message, but got message ID: {}",
            unchoke_buf[4]
        );
    }
    let mut i = 0;
    let mut total_len = 0;
    let file_len: u32 = decoded.info.length.unwrap() as u32;
    let total_pieces = decoded.info.pieces.len() / 20;
    let p_size = decoded.info.piece_length;
    let pending_tasks: Arc<Mutex<HashSet<u64>>> = Arc::new(Mutex::new(HashSet::new()));
    let stream: Arc<Mutex<TcpStream>> = Arc::new(Mutex::new(stream));

    let mut ptasks = vec![];
    loop {
        println!("i set to {} tp is {}", i, total_pieces);
        if i == (total_pieces as u32) {
            break;
        }
        total_len += p_size;
        let temp_file_len = if total_len > file_len {
            // last piece
            p_size - (total_len - file_len)
        } else {
            p_size
        };
        println!("fl set to {} tl is {}", file_len, total_len);
        let ptask = task::spawn(download_piece(
            Arc::clone(&stream),
            i,
            temp_file_len as u32,
            pending_tasks.clone(),
        ));
        println!("pushed task {}", i);
        ptasks.push(ptask);

        i += 1;
    }
    for ptask in ptasks {
        ptask.await.unwrap();
    }

    Ok(())
}
