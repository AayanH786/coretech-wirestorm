use std::{io::Write, sync::{mpsc, Arc, Mutex}, thread};
use std::net::TcpStream;

// Holds all connected receiver clients
pub struct Destinations {
    clients: Arc<Mutex<Vec<TcpStream>>>,
}

impl Destinations {
    // Create a new Destinations instance
    pub fn new() -> Self {
        Destinations {
            clients: Arc::new(Mutex::new(Vec::new())),
        }
    }

    //adds a new receiver client
    pub fn add(&self, client: TcpStream) {
        let mut clients = match self.clients.lock() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Failed to lock clients mutex: {}", e);
                return;
            }   
        };
        clients.push(client);
    }

    //broadcast a message to all connected receiver clients
    pub fn broadcast(&self, data: &[u8]) {
        let mut clients = self.clients.lock().unwrap_or_else(|_| panic!("Failed to lock destinations"));
        clients.retain_mut(|client| client.write_all(data).is_ok());
    }

    pub fn clone_inner(&self) -> Arc<Mutex<Vec<TcpStream>>> {
        Arc::clone(&self.clients)
    }
}

pub struct ThreadPool {
    // A vector to hold the workers in the pool
    workers: Vec<Worker>,
    // holds the sender end of the channel to send jobs to the workers
    sender: Option<mpsc::Sender<Job>>,
}

//the job type will hold the closure that will be recieved by the execute method.
type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    // Create a new thread pool with the specified number of threads. 
    pub fn new(size: usize) -> ThreadPool {
        //make sure thread pool size is > 0, otherwise panic
        assert!(size > 0, "Thread pool size must be greater than zero");

        //create the channels that we will use
        let (sender,receiver) = mpsc::channel();

        //wrap the receiver in a mutex so it can be safely shared between threads, and an Arc so it can be cloned safely. 
        let receiver = Arc::new(Mutex::new(receiver));

        //create a vector (list) of threads that are mutable (can be changed) and with capacity size.
        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            //create some workers and store them in the workers vector
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        // return the created threadpool, which includes workers and the sender (only the value held inside is needed)
        ThreadPool { workers, sender: Some(sender) }
    }
    //this lets me send a task into the threadpool for execution by a thread.
    pub fn execute<F>(&self, f: F)
    where
    //closure can be called at least once (FNOnce), safely transferred across threads (Send), static means it only contains static references
    //FnOnce represents a closure that takes no parameters. 
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

//this allows the thread pool to be dropped
impl Drop for ThreadPool {
    fn drop(&mut self) {
        //take the sender out of the option, which will close the channel
        drop(self.sender.take()); 
        //we want to shutdown all workers, which we do by waiting for the current threads to finish (without errors, unwrap)
        //then these workers are dropped via the drain feature of vectors, the worker is removed from the pool.
        for worker in self.workers.drain(..) {
            println!("Shutting down worker {}", worker.id);
            if let Err(e) = worker.thread.join() {
                eprintln!("Worker {} thread failed to join: {:?}", worker.id, e);
            }
        }
    }
}

//define a new data struture called worker, which represents a single worker in the pool
struct Worker {
    //id for debugging purposes
    id: usize,
    //holds the thread that will be used
    thread: thread::JoinHandle<()>,
}

//worker methods
impl Worker {
    //create a new worker that holds a thread, the receiver is held within an arc and a mutex as seen earlier
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        //create a thread using the thread::spawn function
        let thread = thread::spawn(move || {
            //loop forever
            loop {
                //the receiver passes in a job, this job is locked (to acquire the mutex)
                //then we unwrap to check if there is an error with acquiring a lock
                //then we call recv to get the job from the mutex.
                //recv blocks the thread until a job is available, and mutex ensures only 1 worker tries to request a job.
                let message = match receiver.lock() {
                    Ok(guard) => guard.recv(),
                    Err(e) => {
                        eprintln!("Worker {id} failed to lock the receiver: {e}");
                        break;
                    }
                };

                //check if we got a job or an error
                match message {
                    //if we got a clean job, we execute it
                    Ok(job) => {
                        println!("Worker {id} got a job; executing.");
                        job();
                    }
                    Err(_) => {
                        //if we got an error, we print it
                        println!("Worker {id} got an error; shutting down.");
                        break;
                    }
                }
            }
        });
        //return the created worker
        Worker {id, thread}
    }
}