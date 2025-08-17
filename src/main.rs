//see task 1 for explanations
use std::{fs, io::{prelude::*, BufReader}, net::{TcpListener, TcpStream}, time::Duration};
//import the threads module
use std::thread;
//import the threadpool module from my lib.rs file.
use coretech_wirestorm::ThreadPool; 

fn main() {
    let listener = TcpListener::bind("127.0.0.1:33333").unwrap_or_else(|err| {
        eprintln!("Failed to bind to address: {}", err);
        std::process::exit(1);
    });
    // create a thread pool with 4 threads
    let pool = ThreadPool::new(4); 

    //listener.incoming returns an iterator over the incoming connections
    //each stream represents an open connection between the server and a client
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    //handle the connection in a thread from the pool if the stream is valid
                    pool.execute(|| {
                        handle_connection(stream);
                    });
                }
                Err(e) => {
                    eprintln!("Failed to accept connection: {e}");
                }
            }
        }
}

//this function will handle incoming tcp connections
fn handle_connection(mut stream: TcpStream) {
    //create a new buffered reader that reads from the stream (doesnt consume it)
    let buf_reader = BufReader::new(&stream);

    //read the first line of the http request, and it should contain the request method and path (for us, GET and /)
    let request_line = match buf_reader.lines().next() {
        //success, we can unwrap the line
        Some(Ok(line)) => line,
        //opened the connection but failed to read the line
        Some(Err(e)) => {
            eprintln!("Failed to read from connection {e}");
            return;
        }
        //connection was closed before any data was received
        None => {
            eprintln!("Connection Closed before request was received");
            return;
        }
    };

    //check which path the request was for

    let (filename, status_line) = match &request_line[..] {
        //if root path, send the hello file
        "GET / HTTP/1.1" => ("hello.html", "HTTP/1.1 200 OK"),
        //if for the sleep subpath, simulate a long response
        "GET /sleep HTTP/1.1" => {
            thread::sleep(Duration::from_secs(5));
            ("hello.html", "HTTP/1.1 200 OK")
        }
        //if for any other path, return a 404
        _ => ("404.html", "HTTP/1.1 404 NOT FOUND"),

    };
    //try to read the webpage that was asked
    let contents = match fs::read_to_string(filename) {
        Ok(contents) => contents,
        Err(e) => {
            eprintln!("Failed to read {}: {}", filename, e);
            return;
        }
    };
    let length = contents.len();

    //response will hold a successful message, and the contents of the html page
    let response = format! (
        "{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}"
    );

    //try send the response, if the error returns true, send an error message
    if let Err(e) = stream.write_all(response.as_bytes()) {
        eprintln!("Failed to write to connection: {e}");
    } else {
        println!("Response sent successfully");
    }


}
