# Full System Test Guide

This guide shows how to run the complete CloudP2P system with DoS integration.

## System Architecture

```
┌─────────────┐
│ DoS Server  │ ← Firebase (client registry, access control)
│  port 9000  │
└──────┬──────┘
       │
       ├──────────┬──────────┬──────────┐
       │          │          │          │
┌──────▼───┐ ┌───▼────┐ ┌───▼────┐ ┌───▼────┐
│ Server 1 │ │Server 2│ │Server 3│ │Web API │
│ :8001    │ │ :8002  │ │ :8003  │ │ :3000  │
└──────────┘ └────────┘ └────────┘ └────────┘
     ▲            ▲          ▲          ▲
     │            │          │          │
┌────┴────┬───────┴──────────┴──────────┘
│         │         │
Client1  Client2  Client3
(CLI)    (CLI)    (Web UI)
```

## Prerequisites

```bash
# Set Firebase URL
export FIREBASE_DATABASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/"

# Build everything
cargo build --release
```

## Step 1: Start DoS Server

In **Terminal 1**:
```bash
./restart_dos.sh
```

Or manually:
```bash
pkill -f dos_server
curl -s -X DELETE "${FIREBASE_DATABASE_URL}.json"
sleep 2
export FIREBASE_DATABASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/"
cargo run --bin dos_server
```

You should see:
```
[INFO]   Directory of Services (DoS) Server
[INFO] Firebase URL: https://...
[INFO] DoS server listening on 127.0.0.1:9000
```

## Step 2: Start Processing Servers

In **Terminal 2, 3, 4**:
```bash
# Terminal 2
cargo run --bin server -- --config config/server1.toml

# Terminal 3
cargo run --bin server -- --config config/server2.toml

# Terminal 4
cargo run --bin server -- --config config/server3.toml
```

Servers will:
- Start leader election
- Establish peer connections
- Wait for client requests

## Step 3: Test with CLI Clients

### Option A: Simple Test (1 client)

In **Terminal 5**:
```bash
cargo run --bin client -- --config config/client1.toml
```

The client will:
1. **Auto-register with DoS** (gets `client_1` ID)
2. Discover leader server
3. Send images for encryption
4. Receive encrypted results

### Option B: Multiple Clients

In **Terminal 5, 6, 7**:
```bash
# Terminal 5
cargo run --bin client -- --config config/client1.toml --client-id 1

# Terminal 6
cargo run --bin client -- --config config/client2.toml --client-id 2

# Terminal 7
cargo run --bin client -- --config config/client3.toml --client-id 3
```

Each client gets incremental IDs: `client_1`, `client_2`, `client_3`

## Step 4: Test with Web Interface

### Start Web Server

In **Terminal 8**:
```bash
cargo run --bin web_server
```

### Access Web UI

Open browser: `http://127.0.0.1:3000`

The web interface will:
1. **Auto-register with DoS on first use**
2. Upload images via browser
3. Send to server cluster for encryption
4. Download encrypted results

## Verification

### Check DoS Registration

```bash
# List all online clients
curl -s "${FIREBASE_DATABASE_URL}/clients.json" | python3 -m json.tool
```

You should see:
```json
{
  "client_1": {
    "client_id": "client_1",
    "status": "Online",
    "ip_address": "127.0.0.1",
    "images": {},
    "peers": []
  },
  "client_2": { ... },
  "client_3": { ... }
}
```

### Check Request IDs

```bash
# View pending requests
curl -s "${FIREBASE_DATABASE_URL}/pending_requests.json" | python3 -m json.tool
```

Request IDs follow format: `req_1_client_2`, `req_2_client_1`, etc.

## Testing Scenarios

### Scenario 1: Basic Encryption Flow

1. Start DoS + 3 servers
2. Start client1
3. Client auto-registers as `client_1`
4. Client sends image to leader
5. Leader processes and returns encrypted image
6. Client saves result

### Scenario 2: Leader Election

1. Start DoS + 3 servers
2. Wait for leader election (check logs)
3. Kill current leader: `pkill -f server1`
4. Watch servers elect new leader
5. Start client - should find new leader automatically

### Scenario 3: Multi-Client Load

1. Start DoS + 3 servers
2. Start 3 clients simultaneously
3. All register with different IDs
4. Servers distribute load among clients
5. Check metrics: `cat metrics/machine_1/client_1.log`

### Scenario 4: Web + CLI Mix

1. Start DoS + 3 servers
2. Start 2 CLI clients
3. Start web server
4. Use web UI to upload images
5. All clients work concurrently

## Troubleshooting

### Issue: "Client ID not found"

**Cause:** Firebase has old/corrupted data

**Fix:**
```bash
pkill -f dos_server
curl -X DELETE "${FIREBASE_DATABASE_URL}.json"
sleep 2
cargo run --bin dos_server
```

### Issue: "No leader available"

**Cause:** Servers haven't finished election

**Fix:** Wait 3-5 seconds for election to complete

### Issue: "Failed to connect to DoS"

**Cause:** DoS server not running

**Fix:** Check Terminal 1, restart DoS server

### Issue: Port already in use

**Fix:**
```bash
# Kill all processes
pkill -f dos_server
pkill -f "cargo run --bin server"
pkill -f "cargo run --bin client"
pkill -f web_server

# Restart as needed
```

## Performance Testing

### Stress Test with Metrics

```bash
# Terminal 1: DoS
cargo run --bin dos_server

# Terminals 2-4: Servers
cargo run --bin server -- --config config/server1.toml
cargo run --bin server -- --config config/server2.toml
cargo run --bin server -- --config config/server3.toml

# Terminal 5: Stress client
cargo run --bin client -- \
  --config config/client_stress.toml \
  --client-id 1 \
  --metrics-output metrics/stress_test.json

# Check metrics
cat metrics/stress_test.json | python3 -m json.tool
```

## Monitoring

### Watch DoS Activity

```bash
# In DoS terminal, you'll see:
[INFO] Processing ClientSignUp: name=Client1
[INFO] Client signed up successfully: id=client_1
[INFO] Processing ImageAccessRequest: from=client_2, to=client_1
[INFO] Assigned request ID: req_1_client_2
```

### Watch Server Activity

```bash
# In server terminals, you'll see:
[INFO] Leader elected: Server 1
[INFO] Received encryption request from client_1
[INFO] Processed image: secret.jpg
```

### Watch Client Activity

```bash
# In client terminals, you'll see:
[INFO] Signing up with DoS: name=Client1
[INFO] Sign-up successful: client_id=client_1
[INFO] Discovered leader: 127.0.0.1:8001
[INFO] Sending image for encryption...
[INFO] Received encrypted result
```

## Next Steps

Once basic testing works:

1. **P2P Image Sharing**: Implement client-to-client image sharing using DoS access control
2. **Distributed Across Machines**: Deploy servers on different machines (update IPs in configs)
3. **Fault Tolerance Testing**: Kill servers/clients, verify automatic recovery
4. **Load Balancing**: Test with many concurrent clients
5. **Web UI Enhancement**: Add client registration UI, image gallery, access control

## Quick Commands Reference

```bash
# Start everything locally
./restart_dos.sh                                    # Terminal 1
cargo run --bin server -- --config config/server1.toml  # Terminal 2
cargo run --bin server -- --config config/server2.toml  # Terminal 3
cargo run --bin server -- --config config/server3.toml  # Terminal 4
cargo run --bin client -- --config config/client1.toml  # Terminal 5

# Or with web interface
cargo run --bin web_server                          # Terminal 5

# Stop everything
pkill -f "dos_server|server|client|web_server"
```
