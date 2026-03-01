# RustyCache 🦀⚡ (V2.0 - Distributed Edition)

A high-performance, asynchronous Key-Value store built in Rust. Originally a single-node in-memory cache, it has evolved into a fault-tolerant distributed system featuring Leader-Follower replication, real-time TCP broadcasting, and AOF (Append-Only File) persistence.

## Architectural Features
- **Distributed Replication:** Master-Slave (Leader-Follower) architecture for data redundancy and high availability.
- **Asynchronous Broadcasting:** Real-time data synchronization across nodes using Tokio's broadcast channels over TCP.
- **In-Memory Engine:** Blazing fast data retrieval using pre-allocated, thread-safe HashMaps (`Arc` and `Mutex`).
- **Hardware Optimized:** Tested to handle 30,000+ Requests Per Second (RPS) with minimal memory footprint (RSS < 5MB).
- **AOF Persistence:** Real-time disk logging for zero data loss during simulated system crashes.

## How to Run the Distributed Cluster

**1. Start the Leader Node:**
\`\`\`bash
cargo run -- 6379 leader
\`\`\`

**2. Start a Follower Node (in a new terminal):**
\`\`\`bash
cargo run -- 6380 follower 127.0.0.1:6379
\`\`\`

**3. Test the Replication:**
- Connect to Leader (`nc 127.0.0.1 6379`) and `SET` a key.
- Connect to Follower (`nc 127.0.0.1 6380`) and `GET` the same key instantly!
