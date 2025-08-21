use std::{net::{TcpListener, TcpStream}, sync::{Arc, Mutex}, thread};
// Import custom thread pool and destination management from the library.
use coretech_wirestorm::{Destinations, ThreadPool,handle_transmitter}; 

// Entry point for the server application.
// Sets up listeners, thread pool, and manages client connections.
fn main() {
    // Bind the main TCP listener for source (transmitter) clients.
    let listener = TcpListener::bind("127.0.0.1:33333")
        .unwrap_or_else(|e| {
            panic!("Failed to bind to port 33333: {}", e);
        });

    // Create a thread pool for handling transmitter connections.
    let pool = ThreadPool::new(2);
    // Shared state for the currently active transmitter connection.
    let active_source = Arc::new(Mutex::new(None::<TcpStream>));
    // Manages all receiver clients.
    let destinations = Destinations::new();
    // Clone the inner Arc<Mutex<Vec<TcpStream>>> for use in destination thread.
    let dest_clone = destinations.clone_inner();

    // Spawn a thread to handle incoming destination (receiver) client connections.
    // Each new connection is added to the shared destinations list.
    thread::spawn(move || {
        let dest_listener = TcpListener::bind("127.0.0.1:44444")
            .unwrap_or_else(|e| panic!("Failed to bind to port 44444: {e}"));

        for stream in dest_listener.incoming() {
            match stream {
                Ok(stream) => {
                    eprintln!("New destination client connected");
                    dest_clone
                        .lock()
                        .unwrap_or_else(|_| panic!("Failed to lock destinations mutex"))
                        .push(stream);
                }
                Err(e) => eprintln!("Destination connection error: {e}"),
            }
        }
    });

    // Accept incoming transmitter (source) connections.
    // Only one transmitter is allowed at a time; others are rejected.
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let dests_clone = destinations.clone_inner();
                let active_clone = Arc::clone(&active_source);

                // Scope for locking and checking the active transmitter.
                {
                    let mut active = active_clone
                        .lock()
                        .unwrap_or_else(|_| panic!("Failed to lock active_source mutex"));

                    // If a transmitter is already active, reject the new connection.
                    if active.is_some() {
                        eprintln!("Source client already connected, ignoring new connection");
                        continue;
                    }

                    // Set the active transmitter to the new stream.
                    *active = Some(
                        stream
                            .try_clone()
                            .unwrap_or_else(|_| panic!("Failed to clone source stream")),
                    );
                }

                // Send the transmitter connection to the thread pool for handling.
                pool.execute(move || {
                    handle_transmitter(stream, dests_clone, active_clone);
                });
            }
            Err(e) => eprintln!("Source connection error: {e}"),
        }
    }
}