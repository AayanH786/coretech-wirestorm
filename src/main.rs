use std::{io::{prelude::*, BufReader,Read}, net::{TcpListener, TcpStream}, sync::{Arc, Mutex}, thread};
//import the threadpool module from my lib.rs file.
use coretech_wirestorm::{Destinations, ThreadPool}; 

const CTMP_HEADER_LEN: usize = 8;
const CTMP_MAGIC_BYTE: u8 = 0xCC;
const CTMP_PAD: u8 = 0x00;
const CTMP_MAX_PAYLOAD_SIZE: usize = 65536; //16KiB

fn main() {
    // bind the TcpListener to the port, panic if failed to bind (due to being in use or other reason)
    let listener = TcpListener::bind("127.0.0.1:33333")
    .unwrap_or_else(|e| {
        panic!("Failed to bind to port 33333: {}", e);});

    // create a thread pool with given size (2 for now) and 
    let pool = ThreadPool::new(2); 
    //active_source holds the currently active transmitter connection.
    let active_source = Arc::new(Mutex::new(None::<TcpStream>));
    //holds all receivers to broadcast messages to.
    let destinations = Destinations::new();
    //clone the inner vector of destinations to push into the thread.
    let dest_clone = destinations.clone_inner();

    //create threads for destination clients. note that I currently don't check if a thread panics (do more research on concurrent to see if this can happen)
    thread::spawn(move || {
        let dest_listener = TcpListener::bind("127.0.0.1:44444")
        .unwrap_or_else(|e| panic!("Failed to bind to port 44444: {e}"));

    //for each destination connection, add the new connection to the destinations vector so we know who is added
    for stream in dest_listener.incoming() {
        match stream {
            //if the stream is ok, push it to the destinations
            Ok(stream) => {
                eprintln!("New destination client connected");
                //unlock the destinations vector and add the new stream
                dest_clone
                .lock()
                .unwrap_or_else(|_| panic!("Failed to lock destinations mutex"))
                .push(stream);
            }
            Err(e) => eprintln!("Destination connection error: {e}"),
        }
    }
    });

    // check incoming transmitter connections and see if we already have one.
    for stream in listener.incoming() {
    match stream {
        Ok(stream) => {
            //grab the current receivers and the current active transmitter (shouldnt this be defined later?)
            let dests_clone = destinations.clone_inner();
            let active_clone = Arc::clone(&active_source);

            // wrap the lock and checks in a scope so the borrow ends
            {
                //check if there is an active transmitter, if there is ignore the new one.
                let mut active = active_clone
                    .lock()
                    .unwrap_or_else(|_| panic!("Failed to lock active_source mutex"));

                //check if there is an active transmitter, if there is ignore the new one.
                if active.is_some() {
                    eprintln!("Source client already connected, ignoring new connection");
                    continue;
                }

                //set the active transmitter to the new stream
                *active = Some(
                    stream
                        .try_clone()
                        .unwrap_or_else(|_| panic!("Failed to clone source stream")),
                );
            }


            //send the connection to the thread pool for handling
            pool.execute(move || {
                handle_source_connection(stream, dests_clone, active_clone);
            });
        }
        Err(e) => eprintln!("Source connection error: {e}"),
    }
}

}

//this function will handle the transmitter
fn handle_source_connection(
    stream: TcpStream,
    destinations: Arc<Mutex<Vec<TcpStream>>>,
    active_source: Arc<Mutex<Option<TcpStream>>>,
) {
    let mut buf_reader = BufReader::new(&stream);
    let mut header = [0u8; CTMP_HEADER_LEN];

    loop {
        // Read the fixed-size header
        if let Err(e) = buf_reader.read_exact(&mut header) {
            eprintln!("Failed to read header: {}", e);
            break;
        }
        println!("Received header: {:?}", header);

        // Validate magic byte
        if header[0] != CTMP_MAGIC_BYTE {
            eprintln!("Invalid magic byte: {:02X}", header[0]);
            break;
        }

        // Extract fields
        let options = header[1];
        let sensitive = (options & 0x40) != 0; // bit 1
        let length = u16::from_be_bytes([header[2], header[3]]) as usize;
        let checksum_in_msg = u16::from_be_bytes([header[4], header[5]]);
        let padding = &header[6..8];

        // Validate padding
        if padding != [CTMP_PAD, CTMP_PAD] {
            eprintln!("Invalid padding in header");
            break;
        }

        // Validate length
        if length == 0 || length > CTMP_MAX_PAYLOAD_SIZE {
            eprintln!("Invalid payload length: {}", length);
            break;
        }

        // Read payload
        let mut payload = vec![0u8; length];
        if let Err(e) = buf_reader.read_exact(&mut payload) {
            eprintln!("Failed to read payload: {}", e);
            break;
        }

        // If sensitive, validate checksum
        if sensitive {
            let mut header_for_checksum = header.clone();
            header_for_checksum[4] = 0xCC;
            header_for_checksum[5] = 0xCC;

            let mut sum: u32 = 0;
            for chunk in header_for_checksum.chunks(2).chain(payload.chunks(2)) {
                let val = if chunk.len() == 2 {
                    u16::from_be_bytes([chunk[0], chunk[1]]) as u32
                } else {
                    (chunk[0] as u32) << 8
                };
                sum = sum.wrapping_add(val);
            }

            while (sum >> 16) != 0 {
                sum = (sum & 0xFFFF) + (sum >> 16);
            }

            let checksum_computed = !(sum as u16);
            if checksum_computed != checksum_in_msg {
                eprintln!("Invalid checksum for sensitive message, dropping");
                continue;
            } else {
                // Build full frame for broadcast
                let mut frame = Vec::with_capacity(CTMP_HEADER_LEN + length);
                frame.extend_from_slice(&header);
                frame.extend_from_slice(&payload);

                // Broadcast to destinations
                let mut dests = destinations
                    .lock()
                    .unwrap_or_else(|_| panic!("Failed to lock destinations mutex"));
                dests.retain_mut(|dest| dest.write_all(&frame).is_ok());
                eprintln!("broadcasted sensitive message")
            }
        } else {
            // Build full frame for broadcast
                let mut frame = Vec::with_capacity(CTMP_HEADER_LEN + length);
                frame.extend_from_slice(&header);
                frame.extend_from_slice(&payload);

                // Broadcast to destinations
                let mut dests = destinations
                    .lock()
                    .unwrap_or_else(|_| panic!("Failed to lock destinations mutex"));
                dests.retain_mut(|dest| dest.write_all(&frame).is_ok());
                eprintln!("broadcasted none sensitive message")
        }

    }

    // Clear active source when done
    let mut active = active_source
        .lock()
        .unwrap_or_else(|_| panic!("Failed to lock active source mutex"));
    *active = None;
    eprintln!("Source client disconnected");
}


