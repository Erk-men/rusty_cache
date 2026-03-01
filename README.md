# RustyCache 🦀⚡

A lightweight, multi-threaded, and asynchronous Key-Value store built entirely in Rust. Designed to explore system programming, memory safety without garbage collection, and custom TCP network protocols.

## Features
- **In-Memory Engine:** Fast data retrieval using pre-allocated, thread-safe HashMaps.
- **Concurrency:** Safe shared state management using Rust's `Arc` and `Mutex` to prevent race conditions.
- **Async Networking:** Non-blocking TCP socket handling powered by `tokio`.
- **AOF Persistence:** Real-time disk logging (Append-Only File) for zero data loss and crash recovery.

## How to Run
1. Start the server: `cargo run`
2. Connect via TCP client: `nc 127.0.0.1 6379`
3. Send commands:
   - `SET [key] [value]`
   - `GET [key]`
