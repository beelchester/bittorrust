use std::{
    collections::{HashMap, HashSet},
    net::SocketAddrV4,
    path::PathBuf,
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use tokio::{
    fs::OpenOptions,
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::Mutex,
    task,
    time::sleep,
};

use crate::{torrent::Torrent, DEFAULT_BLOCK_LENGTH, MAX_CONCURRENT_REQUESTS};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Peer {
    pub socket: SocketAddrV4,
}

impl Peer {
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
            "ph: {:?}",
            u32::from_be_bytes(piece_header[0..4].try_into().unwrap())
        );
        assert_eq!(piece_header[4], 7);
        let mut block_data = vec![0; block_length as usize];
        stream.read_exact(&mut block_data).await.unwrap();
        println!("bl len: {:?}", block_data.len());
        println!("off: {}", offset);

        let mut d_buf = download_piece_buf.lock().await;
        d_buf.insert(block_index, block_data);
        println!("dp of {piece_index}: {:?}", d_buf.len());

        let mut temp_current_block_tasks = current_block_tasks.lock().await;
        println!("pending tasks: {:?}", temp_current_block_tasks);
        temp_current_block_tasks.remove(&block_task_id);
        println!("pending tasks: {:?}", temp_current_block_tasks);
        println!(
            "--------------- ended b {}, for {}",
            block_task_id, piece_index
        );
    }

    pub async fn download_piece(
        stream: Arc<Mutex<TcpStream>>,
        piece_index: u32,
        piece_length: u32,
        current_block_tasks: Arc<Mutex<HashSet<u64>>>,
        piece_hash: String,
        whole_file_buf: Option<Arc<Mutex<HashMap<u32, Vec<u8>>>>>,
        output_path: PathBuf,
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
            let task = task::spawn(Self::request_block(
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
        let block_data_hashmap = download_piece_buf.lock().await;
        let mut piece_data_buf = Vec::with_capacity(piece_length.try_into().unwrap());
        let mut o = 0;
        loop {
            if o == block_index {
                break;
            }
            let block_data = block_data_hashmap.get(&o).unwrap();
            piece_data_buf.extend(block_data);
            println!(
                "********* len of piece {piece_index} buf: {:?}",
                piece_data_buf.len()
            );
            o += 1;
        }
        let piece_buf_hash = Sha1::digest(piece_data_buf.clone());
        assert_eq!(
            piece_hash,
            hex::encode(piece_buf_hash), //NOTE: not sure if comparing hex values of hashes is good
            "integrity of piece {piece_index} failed"
        );
        match whole_file_buf {
            Some(buf) => {
                let mut b = buf.lock().await;
                b.insert(piece_index, piece_data_buf);
            }
            None => {
                let mut file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(output_path)
                    .await
                    .expect("failed to open file");

                file.write_all(&piece_data_buf)
                    .await
                    .expect("failed to write to file");
                println!("file written from piece");
            }
        }
    }

    pub async fn handshake(peer: Peer, info_hash: [u8; 20]) -> TcpStream {
        let mut stream = TcpStream::connect(peer.socket).await.unwrap();
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
    pub async fn download_torrent(
        mut stream: TcpStream,
        torrent: Torrent,
        piece_hashes: Vec<String>,
        output_path: PathBuf,
        only_piece: Option<u32>,
    ) {
        // getting the bitfield
        // getting bitfield message prefix that indicates the total size of the bitfield message
        // excluding itself(4 bytes)
        let mut bitfield_len_buf = [0; 4];
        stream.read_exact(&mut bitfield_len_buf).await.unwrap();
        let bf = u32::from_be_bytes(bitfield_len_buf) as usize;
        println!("bitfield len gave {}", bf);
        println!("bitfield len buf: {:?}", bitfield_len_buf);

        if bf == 0 {
            println!("No bitfield message received.");
        }

        // getting the message id

        let mut bitfield_msg_buf = vec![0; bf];
        stream.read_exact(&mut bitfield_msg_buf).await.unwrap();
        println!("bitfield msg buf: {:?}", bitfield_msg_buf);
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
        println!("unchoke msg buf: {:?}", unchoke_buf);
        if unchoke_buf[4] != 1 {
            panic!(
                "Expected unchoke message, but got message ID: {}",
                unchoke_buf[4]
            );
        }
        let mut piece_index = only_piece.unwrap_or_default();
        let mut total_len = 0;
        let file_len: u32 = torrent.info.length.unwrap() as u32;
        let total_pieces = torrent.info.pieces.len() / 20;
        let p_size = torrent.info.piece_length;
        let pending_tasks: Arc<Mutex<HashSet<u64>>> = Arc::new(Mutex::new(HashSet::new()));
        let stream: Arc<Mutex<TcpStream>> = Arc::new(Mutex::new(stream));
        let whole_file_buf_lock = Arc::new(Mutex::new(HashMap::new()));

        let mut ptasks = vec![];
        loop {
            println!("i set to {} tp: {}", piece_index, total_pieces);
            if piece_index == (total_pieces as u32) {
                break;
            }
            total_len += p_size;
            let temp_file_len = if total_len > file_len {
                // last piece
                p_size - (total_len - file_len)
            } else {
                p_size
            };
            println!("fl set to {} tl: {}", file_len, total_len);
            let piece_hash = piece_hashes.get(piece_index as usize).unwrap();
            let ptask = task::spawn(Self::download_piece(
                Arc::clone(&stream),
                piece_index,
                temp_file_len,
                pending_tasks.clone(),
                piece_hash.to_string(),
                if only_piece.is_some() {
                    None
                } else {
                    Some(whole_file_buf_lock.clone())
                },
                output_path.clone(),
            ));
            println!("pushed task {}", piece_index);
            ptasks.push(ptask);

            piece_index += 1;
        }
        for ptask in ptasks {
            ptask.await.unwrap();
        }

        let mut temp = 0;
        loop {
            if temp == piece_index {
                break;
            }
            let whole_file_buf = whole_file_buf_lock.lock().await;
            if whole_file_buf.len() > 0 {
                let mut file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&output_path)
                    .await
                    .expect("failed to open file");
                let p_buf = &whole_file_buf.get(&temp).unwrap();
                file.write_all(p_buf)
                    .await
                    .expect("failed to write to file");
                println!("whole file written");
                temp += 1;
            };
        }
    }
}
