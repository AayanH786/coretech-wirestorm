#![warn(missing_docs)]

//! Fast and easy queue abstraction.
//!
//! Provides an abstraction over a queue.  When the abstraction is used
//! there are these advantages:
//! - Fast
//! - [`Easy`]
//!
//! [`Easy`]: http://thatwaseasy.example.com

use std::{sync::{mpsc, Arc, Mutex}, io::{Write,Read,BufReader}, thread};
use std::net::TcpStream;

const CTMP_HEADER_LEN: usize = 8;
const CTMP_PAD: u8 = 0x00;
const CTMP_MAX_PAYLOAD_SIZE: usize = 65536; //16KiB
const CTMP_MAGIC_BYTE: u8 = 0xCC;



/// Holds all connected receiver clients and provides thread-safe methods to manage them.
///
/// The `Destinations` struct wraps a vector of `TcpStream` objects in an `Arc<Mutex<...>>`,
/// allowing safe concurrent access and modification from multiple threads. It is used to
/// manage the set of receiver clients in a networked application, such as a broadcast server.
///
/// # Examples
///
/// ```rust
/// let destinations = Destinations::new();
/// destinations.add(client_stream);
/// let receivers = destinations.clone_inner();
/// ```
pub struct Destinations {
    receivers: Arc<Mutex<Vec<TcpStream>>>,
}


impl Destinations {
    /// Creates a new, empty `Destinations` instance.
    ///
    /// # Returns
    ///
    /// A `Destinations` object with an empty list of receiver clients.
    pub fn new() -> Self {
        Destinations {
            receivers: Arc::new(Mutex::new(Vec::new())),
        }
    }
    /// Adds a new receiver client to the set.
    ///
    /// # Arguments
    ///
    /// * `client` - A `TcpStream` representing the receiver client to add.
    pub fn add(&self, client: TcpStream) {
        let mut clients = match self.receivers.lock() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Failed to lock clients mutex: {}", e);
                return;
            }   
        };
        clients.push(client);
    }
    /// Returns a clone of the internal `Arc<Mutex<Vec<TcpStream>>>`.
    ///
    /// This allows other threads to access or modify the list of receiver clients.
    ///
    /// # Returns
    ///
    /// An `Arc<Mutex<Vec<TcpStream>>>` pointing to the internal vector of clients.
    pub fn clone_inner(&self) -> Arc<Mutex<Vec<TcpStream>>> {
        Arc::clone(&self.receivers)
    }
}

/// A thread pool for executing jobs concurrently.
///
/// The `ThreadPool` struct manages a fixed number of worker threads and a channel for sending jobs to them.
/// It provides methods to create a new pool, execute jobs, and cleanly shut down all workers.
///
/// # Examples
///
/// ```rust
/// let pool = ThreadPool::new(4);
/// pool.execute(|| println!("Hello from a worker thread!"));
/// ```
pub struct ThreadPool {

    // A vector to hold the workers in the pool
    workers: Vec<Worker>,
    // holds the sender end of the channel to send jobs to the workers
    sender: Option<mpsc::Sender<Job>>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    // Create a new thread pool with the specified number of threads. 
    /// Creates a new thread pool with the specified number of worker threads.
    ///
    /// # Arguments
    ///
    /// * `size` - The number of worker threads to spawn. Must be greater than zero.
    ///
    /// # Panics
    ///
    /// Panics if `size` is zero.
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0, "Thread pool size must be greater than zero");

        let (sender,receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));


        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool { workers, sender: Some(sender) }
    }
    //this lets me send a task into the threadpool for execution by a thread.
    /// Sends a job to the thread pool for execution by a worker thread.
    ///
    /// # Arguments
    ///
    /// * `f` - A closure or function to execute. Must be `FnOnce`, `Send`, and `'static`.
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
        {
            let job = Box::new(f);
            
            if let Some(sender) = &self.sender {
                if let Err(e) = sender.send(job) {
                    eprintln!("Failed to send job to thread pool: {}", e);
                }
            } else {
                eprintln!("Thread pool has been shut down, cannot send job.");
            }
        }
}
/// Cleans up the thread pool and joins all worker threads when the pool is dropped.
///
/// The `Drop` implementation for `ThreadPool` ensures that all worker threads are properly shut down
/// and joined before the pool is destroyed. This prevents resource leaks and ensures a clean shutdown.
/// The sender channel is closed, and each worker is joined in turn.
impl Drop for ThreadPool {
    fn drop(&mut self) {
        //take the sender out of the option, which will close the channel
        drop(self.sender.take()); 
        for worker in self.workers.drain(..) {
            println!("Shutting down worker {}", worker.id);
            if let Err(e) = worker.thread.join() {
                eprintln!("Worker {} thread failed to join: {:?}", worker.id, e);
            }
        }
    }
}
/// Represents a single worker in the thread pool.
///
/// Each `Worker` has a unique ID and owns a thread that executes jobs received from the thread pool.
pub struct Worker {
    /// The worker's unique identifier (for debugging and management).
    id: usize,
    /// The thread handle for the worker's execution thread.
    thread: thread::JoinHandle<()>,
}
impl Worker {
    /// Creates a new worker thread for the thread pool.
    ///
    /// # Arguments
    ///
    /// * `id` - The worker's unique identifier.
    /// * `receiver` - Shared receiver for job messages.
    ///
    /// # Returns
    ///
    /// A new `Worker` instance with its own thread.
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        //create a thread using the thread::spawn function
        let thread = thread::spawn(move || {
            loop {
                
                let message = match receiver.lock() {
                    Ok(guard) => guard.recv(),
                    Err(e) => {
                        eprintln!("Worker {id} failed to lock the receiver: {e}");
                        break;
                    }
                };

                match message {
                    Ok(job) => {
                        println!("Worker {id} got a job; executing.");
                        job();
                    }
                    Err(_) => {
                        println!("Worker {id} got an error; shutting down.");
                        break;
                    }
                }
            }
        });
        Worker {id, thread}
    }
}

/// Validates a message header for protocol correctness.
///
/// Checks magic byte, padding, and payload length. Returns the payload length and sensitivity flag if valid.
///
/// # Arguments
/// * `header` - A byte slice representing the message header.
///
/// # Returns
/// * `Ok((u16, bool))` - The payload length and sensitivity flag.
/// * `Err(String)` - An error message if validation fails.
pub fn validate_header(header: &[u8]) -> Result<(u16,bool), String> {

        println!("Received header: {:?}", header);

        // Validate magic byte
        if header[0] != CTMP_MAGIC_BYTE {
            return Err("Invalid magic byte".into());
        }

        let sensitive = (header[1] & 0x40) != 0; // bit 1
        let length = u16::from_be_bytes([header[2],header[3]]) as usize;
        println!("length: {}", length);

        if header[6..8] != [CTMP_PAD, CTMP_PAD] {
            return Err("Invalid padding".into());
        }

        if !sensitive && header[4..6] != [0x00;2]{
            return Err("Invalid Padding for non sensitive headers".into())
        }

        if length == 0 || length > CTMP_MAX_PAYLOAD_SIZE as usize {
            return Err(format!("Invalid payload length: {}", length));
        }

        return Ok((length as u16, sensitive));
        
}

/// Broadcasts a message to all destination clients.
///
/// Builds a frame from the header and payload, then sends it to all connected destinations.
///
/// # Arguments
/// * `header` - The message header bytes.
/// * `payload` - The message payload bytes.
/// * `destinations` - Shared list of destination clients.
pub fn broadcast_message(header: &[u8], payload: &[u8], destinations: Arc<Mutex<Vec<TcpStream>>>) {
    let mut frame = Vec::with_capacity(CTMP_HEADER_LEN + payload.len());
    frame.extend_from_slice(&header);
    frame.extend_from_slice(&payload);

        let mut dests = destinations
                .lock()
                .unwrap_or_else(|_| panic!("Failed to lock destinations mutex"));
        dests.retain_mut(|dest| dest.write_all(&frame).is_ok());
}

/// Computes and verifies the checksum of a message.
///
/// Calculates the checksum over the header and payload using the protocol's algorithm.
///
/// # Arguments
/// * `header` - The message header bytes.
/// * `payload` - The message payload bytes.
///
/// # Returns
/// * `u16` - The computed checksum value.
pub fn verify_checksum(header: &[u8], payload: &[u8]) -> u16 {

    // Set checksum bytes in header to magic byte for calculation
    let mut checksum_header = header.to_owned();
    checksum_header[4] = CTMP_MAGIC_BYTE;
    checksum_header[5] = CTMP_MAGIC_BYTE;

    let mut sum: u32 = 0;

    // Sum header as u16 chunks
    for chunk in checksum_header.chunks(2) {
        let val = if chunk.len() == 2 {
            u16::from_be_bytes([chunk[0], chunk[1]])
        } else {
            u16::from_be_bytes([chunk[0], 0])
        };
        sum += val as u32;
    }

    // Sum payload as u16 chunks
    for chunk in payload.chunks(2) {
        let val = if chunk.len() == 2 {
            u16::from_be_bytes([chunk[0], chunk[1]])
        } else {
            u16::from_be_bytes([chunk[0], 0])
        };
        sum += val as u32;
    }

    // Fold carry bits
    sum = (sum & 0xFFFF) + (sum >> 16);

    // Convert to one's complement
    !(sum as u16)
}

//this function will handle the transmitter
/// Handles a transmitter client, reading messages and broadcasting them.
///
/// Reads headers and payloads from the source client, validates them, and broadcasts valid messages to all destinations.
/// If a sensitive message fails checksum validation, it is dropped.
///
/// # Arguments
/// * `stream` - The TCP stream for the transmitter client.
/// * `destinations` - Shared list of destination clients.
/// * `active_source` - Shared state for the active source client.
pub fn handle_transmitter(
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
        
        let (length, sensitive) = match validate_header(&header) {
            Ok(result) => result,
            Err(e) => {
                eprintln!("Error validating header: {}", e);
                break;
            }
        };

        let mut payload = vec![0u8; length as usize];
        if let Err(e) = buf_reader.read_exact(&mut payload) {
            eprintln!("Failed to read payload: {}", e);
            break;
        }

        let checksum_in_msg = u16::from_be_bytes([header[4], header[5]]);

        // If sensitive, validate checksum
        if sensitive {

            let checksum_computed = verify_checksum(&header, &payload);
            if checksum_computed != checksum_in_msg {
                eprintln!("Invalid checksum for sensitive message, dropping");
                continue;
            }
        }

        broadcast_message(&header, &payload, destinations.clone());
    }

    // Clear active source when done
    let mut active = active_source
        .lock()
        .unwrap_or_else(|_| panic!("Failed to lock active source mutex"));
    *active = None;
    eprintln!("Source client disconnected");
}