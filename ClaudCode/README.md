# Cloud P2P Image Sharing System

A distributed peer-to-peer cloud system for controlled sharing of encrypted images with fault tolerance and load balancing.

## Features

-   **Modified Bully Election Algorithm** for leader election
-   **Load Balancing** via distributed work election
-   **Fault Tolerance** with automatic recovery
-   **Image Encryption** using steganography
-   **Discovery Service** for online user tracking
-   **State Synchronization** across servers

## Architecture

-   3 Server Nodes (P2P coordination)
-   Modified Bully election with priority based on load, reliability, response time
-   Heartbeat-based failure detection
-   Automatic state recovery

## Building

```bash
cargo build --release
```

## Running

### Development (Localhost)

```bash
# Terminal 1
cargo run --bin server -- --config config/server1.toml

# Terminal 2
cargo run --bin server -- --config config/server2.toml

# Terminal 3
cargo run --bin server -- --config config/server3.toml

# Terminal 4 (Client)
cargo run --bin client
```

### Production (Different Machines)

1. Edit `config/server*.toml` files with actual IP addresses
2. Deploy binary to each machine
3. Run on each machine:

```bash
./target/release/server --config config/server1.toml
```

## Configuration

See `config/` directory for server configurations.

## Testing

```bash
cargo test
```

## License

MIT
