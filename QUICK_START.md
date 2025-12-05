# Quick Start Guide - CloudP2P Testing

## ONE-TIME SETUP

```bash
cd ~/Documents/Distrubuted-Systems/cloud-p2p-image-sharing

# 1. Setup test images (already done!)
./setup_images.sh

# 2. Build everything
cargo build --release
```

---

## RUNNING THE SYSTEM (7 Terminals)

### Terminal 1: DoS Server
```bash
cd ~/Documents/Distrubuted-Systems/cloud-p2p-image-sharing
export FIREBASE_DATABASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/"
curl -X DELETE "${FIREBASE_DATABASE_URL}.json"
sleep 2
cargo run --bin dos_server
```

### Terminal 2-4: Processing Servers
```bash
# Terminal 2
cargo run --bin server -- --config config/server1.toml

# Terminal 3
cargo run --bin server -- --config config/server2.toml

# Terminal 4
cargo run --bin server -- --config config/server3.toml
```

**Wait 5 seconds for leader election**

### Terminal 5-7: Web Clients
```bash
# Terminal 5 - Client 1
cargo run --bin web_server -- --config config/client1.toml --port 3001

# Terminal 6 - Client 2
cargo run --bin web_server -- --config config/client2.toml --port 3002

# Terminal 7 - Client 3
cargo run --bin web_server -- --config config/client3.toml --port 3003
```

---

## TESTING (Terminal 8)

```bash
cd ~/Documents/Distrubuted-Systems/cloud-p2p-image-sharing

# Run complete demo
./demo_test.sh
```

---

## MANUAL TESTING

### Sign Up
```bash
curl -X POST http://127.0.0.1:3001/api/signup
curl -X POST http://127.0.0.1:3002/api/signup
curl -X POST http://127.0.0.1:3003/api/signup
```

### View Online Peers
```bash
curl http://127.0.0.1:3001/api/online-peers | python3 -m json.tool
```

### My Images
```bash
curl http://127.0.0.1:3001/api/my-images | python3 -m json.tool
```

### Encrypt Image
```bash
curl -X POST http://127.0.0.1:3001/api/encrypt-image/image1.jpg | python3 -m json.tool
```

### View Shared Images
```bash
curl http://127.0.0.1:3002/api/shared-images | python3 -m json.tool
```

### Sign Out
```bash
curl -X POST http://127.0.0.1:3003/api/signout
```

---

## FAULT TOLERANCE TEST

### Kill Leader (Press Ctrl+C in Terminal 2)
- Watch Terminals 3-4 for new leader election
- System continues working!

### Try Encrypting Again
```bash
curl -X POST http://127.0.0.1:3001/api/encrypt-image/image2.jpg | python3 -m json.tool
```
**Should still work with new leader!**

---

## CHECK FIREBASE

```bash
export FIREBASE_DATABASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/"

# View all clients
curl -s "${FIREBASE_DATABASE_URL}/clients.json" | python3 -m json.tool

# View pending requests
curl -s "${FIREBASE_DATABASE_URL}/pending_requests.json" | python3 -m json.tool

# View metadata (client & request counters)
curl -s "${FIREBASE_DATABASE_URL}/metadata.json" | python3 -m json.tool
```

---

## STOP EVERYTHING

```bash
pkill -f "dos_server|cargo run --bin server|cargo run --bin web_server"
```

---

## API ENDPOINTS

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/health` | GET | Health check |
| `/api/signup` | POST | Register client with DoS |
| `/api/signout` | POST | Sign out (go offline) |
| `/api/status` | GET | Check if signed in |
| `/api/online-peers` | GET | List all online clients |
| `/api/my-images` | GET | List my images |
| `/api/encrypt-image/:name` | POST | Encrypt image at server |
| `/api/shared-images` | GET | View all shared images |
| `/api/request-image?image_id=X&owner_id=Y` | POST | Request access (max 3 times) |
| `/api/pending-requests` | GET | View pending requests |
| `/api/respond-request/:id/:approved` | POST | Approve/deny request |

---

## FEATURES IMPLEMENTED

✅ Client sign-up/sign-out with DoS
✅ Incremental client IDs (client_1, client_2, client_3)
✅ Incremental request IDs (req_1_client_2, req_2_client_1, etc.)
✅ View online peers
✅ Encrypt images at server side
✅ Request image access with 3-attempt limit
✅ Pending requests when owner offline
✅ Request expiration if requester goes offline
✅ View shared images from all clients
✅ **FAULT TOLERANCE**: Automatic server failover with leader election
✅ Firebase persistence for all data

---

## FILE STRUCTURE

```
test_images/
├── client1/          # Client 1's images (3 images)
├── client2/          # Client 2's images (3 images)
├── client3/          # Client 3's images (3 images)
├── encrypted/        # Encrypted images saved here
└── decrypted/        # Decrypted images (future)

config/
├── server1.toml      # Server 1 config (127.0.0.1:8001)
├── server2.toml      # Server 2 config (127.0.0.1:8002)
├── server3.toml      # Server 3 config (127.0.0.1:8003)
├── client1.toml      # Client 1 config
├── client2.toml      # Client 2 config
└── client3.toml      # Client 3 config
```

---

## TROUBLESHOOTING

| Problem | Solution |
|---------|----------|
| "Failed to connect to DoS" | Start DoS server (Terminal 1) |
| "Failed to encrypt image" | Wait for leader election (5 sec) |
| "Image not found" | Run `./setup_images.sh` |
| "Already signed up" | Restart web server or sign out first |
| "Port already in use" | Run `pkill -f "dos_server\|server\|web_server"` |

---

For detailed instructions, see **[COMPLETE_TESTING_GUIDE.md](COMPLETE_TESTING_GUIDE.md)**
