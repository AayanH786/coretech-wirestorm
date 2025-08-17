use std::{io::{prelude::*, BufReader}, net::{TcpListener, TcpStream}, sync::{Arc, Mutex}, thread};
//import the threadpool module from my lib.rs file.
use coretech_wirestorm::{Destinations, ThreadPool}; 

fn main() {

    let listener = TcpListener::bind("127.0.0.1:33333")
    .unwrap_or_else(|e| {
        panic!("Failed to bind to port 33333: {}", e);});

    // create a thread pool with 4 threads
    let pool = ThreadPool::new(2); 
    let destinations = Destinations::new();
    let dest_clone = destinations.clone_inner();


    thread::spawn(move || {
        let dest_listener = TcpListener::bind("127.0.0.1:44444")
        .unwrap_or_else(|e| panic!("Failed to bind to port 44444: {e}"));

    for stream in dest_listener.incoming() {
        match stream {
            Ok(stream) => {
                eprintln!("New destination client connected");
                dest_clone.lock().unwrap_or_else(|_| panic!("Failed to lock destinations mutex")).push(stream);
            }
            Err(e) => eprintln!("Destination connection error: {e}"),
        }
    }
    });

    let mut source_connected = false;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if source_connected {
                    eprintln!("Source client already connected, ignoring new connection");
                    continue;
                }
                source_connected = true;
                eprintln!("New source client connected");
                let dests_clone = destinations.clone_inner();
                pool.execute(move || {
                    handle_source_connection(stream, dests_clone);
                });
            }
            Err(e) => eprintln!("Source connection error: {e}"),
        }
    }
}

//this function will handle incoming tcp connections
fn handle_source_connection(stream: TcpStream, destinations: Arc<Mutex<Vec<TcpStream>>>) {
    let mut buf_reader = BufReader::new(&stream);
    let mut buffer = Vec::new();

    loop {
        match buf_reader.read_until(b'\n', &mut buffer) {
            Ok(0) => {
                eprintln!("Source client disconnected");
                break; // connection closed
            }
            Ok(_) => {
                let mut dests = destinations
                .lock()
                .unwrap_or_else(|_| panic!("Failed to lock destinations mutex"));

                dests.retain_mut(|dest| dest.write_all(&buffer).is_ok());

                buffer.clear();
            }
            Err(e) => {
                eprintln!("Error reading from source client: {e}");
                break; // exit on error
            }
        }
    }
    eprintln!("Source client disconnected")
}
