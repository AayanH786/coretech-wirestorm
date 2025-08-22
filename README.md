
# Operation WIRE STORM - CoreTech Relay Server

## Mission Overview

This repository contains my solution for the CoreTech Security WIRE STORM challenge. The goal was to build a high-speed TCP relay server in Rust, implementing the CoreTech Message Protocol (CTMP) to securely and efficiently forward sensitive messages between a single source client and multiple destination clients.

## What I Accomplished
- Built a multi-threaded TCP relay server in Rust, supporting one source client and multiple destination clients.
- Implemented the CTMP protocol, including header validation, length checks, and (for sensitive messages) checksum validation.
- Designed a custom thread pool and thread-safe client management using Rust's standard library.
- Ensured messages are broadcast in order and invalid messages are dropped according to protocol rules.
- Documented the code and provided clear build and usage instructions for operational validation.

## How to Build and Run (Ubuntu 24.04 LTS)

1. **Install Rust:**
   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env
   ```
2. **Clone the repository:**
   ```sh
   git clone https://github.com/AayanH786/coretech-wirestorm.git
   cd coretech-wirestorm
   ```
3. **Build the project:**
   ```sh
   cargo build --release
   ```
4. **Run the server:**
   ```sh
   cargo run --release
   ```
   - The server listens for source connections on `127.0.0.1:33333` and destination connections on `127.0.0.1:44444`.


## Usage and Validation
- Connect a single source client to port 33333.
- Connect one or more destination clients to port 44444.
- Send CTMP messages from the source; valid messages are broadcast to all destinations in order.

### How to Verify Your Solution with the Provided Tests
1. Ensure your server is running (`cargo run --release`).
2. Open a new Terminal and locate the provided Python test script (`tests.py`).
3. Run the tests using Python 3.12:
   ```sh
   python3 tests.py
   ```
4. The tests will automatically connect to your server, send a variety of CTMP messages, and check the results.
5. If all tests pass, you will see a success message. If any test fails, review the error output and adjust your implementation as needed.

**Note:** No additional Python libraries are required. The tests are self-contained and designed for Ubuntu 24.04 LTS.

## Potential Limitations
- Only one source client is allowed at a time; additional sources are rejected.
- No authentication or encryption; all clients on localhost can connect.
- No persistent message storage; messages are relayed live only.
- Uses threads for concurrency; async runtimes may scale better for very high connection counts.
- Error logs are printed to stderr; no advanced logging or monitoring is included.
- Graceful exit of server upon resolution.
- Time out connected transmitter after an extended period of time with no activity.

## Challenge Context
This project was developed for the CoreTech Security WIRE STORM graduate challenge. The solution is designed to be readable, efficient, and well-documented, meeting all requirements for operational validation and submission.
