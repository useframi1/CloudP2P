# CloudP2P - Distributed Image Encryption System with DoS Integration

## 🎯 Project Overview

A **fault-tolerant, distributed image encryption system** with Directory of Services (DoS) for client management and P2P coordination.

### System Components

1. **DoS Server (1)** - Port 9000
   - Client registry and authentication
   - Access control management
   - Pending request handling
   - Firebase persistence

2. **Processing Servers (3)** - Ports 8001-8003
   - Leader election (Bully Algorithm)
   - Image encryption/steganography
   - Automatic failover

3. **Web Clients (3)** - Ports 3001-3003
   - REST API interface
   - DoS integration
   - Fault-tolerant server communication

---

## ✨ Features Implemented

### Core Features
- ✅ **Client Sign-Up**: Register with DoS, get incremental IDs (`client_1`, `client_2`, `client_3`)
- ✅ **Client Sign-Out**: Clean offline status (no "dead" clients)
- ✅ **View Online Peers**: See all currently active clients
- ✅ **Server-Side Encryption**: Submit images to processing cluster
- ✅ **View Shared Images**: Browse images from all clients
- ✅ **Request Image Access**: P2P image sharing with access control
- ✅ **Pending Requests**: Store requests when recipient is offline
- ✅ **Request Expiration**: Auto-discard if requester goes offline
- ✅ **Request Limits**: Maximum 3 access requests per image

### Technical Features
- ✅ **Fault Tolerance**: Automatic leader election and failover
- ✅ **Firebase Integration**: Persistent storage for all data
- ✅ **Incremental IDs**:
  - Client IDs: `client_1`, `client_2`, `client_3`
  - Request IDs: `req_1_client_2`, `req_2_client_1`
- ✅ **Single-PC Testing**: All components run on localhost
- ✅ **REST API**: Complete HTTP API for all operations

---

## 🚀 Quick Start

### 1. Setup (One-Time)
```bash
cd ~/Documents/Distrubuted-Systems/cloud-p2p-image-sharing

# Setup test images
./setup_images.sh

# Build project
cargo build --release
```

### 2. Run System (Open 7 Terminals)

**Terminal 1: DoS Server**
```bash
export FIREBASE_DATABASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/"
curl -X DELETE "${FIREBASE_DATABASE_URL}.json" && sleep 2
cargo run --bin dos_server
```

**Terminals 2-4: Processing Servers**
```bash
cargo run --bin server -- --config config/server1.toml  # Terminal 2
cargo run --bin server -- --config config/server2.toml  # Terminal 3
cargo run --bin server -- --config config/server3.toml  # Terminal 4
```

**Terminals 5-7: Web Clients**
```bash
cargo run --bin web_server -- --config config/client1.toml --port 3001  # Terminal 5
cargo run --bin web_server -- --config config/client2.toml --port 3002  # Terminal 6
cargo run --bin web_server -- --config config/client3.toml --port 3003  # Terminal 7
```

### 3. Test Everything
```bash
# Terminal 8
./demo_test.sh
```

---

## 📋 Testing Checklist

### Basic Operations
- [ ] Sign up 3 clients → Get `client_1`, `client_2`, `client_3`
- [ ] View online peers → See other 2 clients
- [ ] Encrypt image → Saved to `test_images/encrypted/`
- [ ] View shared images → See all clients' images
- [ ] Sign out client → Status becomes offline

### Access Control
- [ ] Request image access → Get request ID like `req_1_client_2`
- [ ] Request limit → 4th request fails with "limit exceeded"
- [ ] Offline owner → Request stored as pending
- [ ] Requester goes offline → Request discarded on approval

### Fault Tolerance
- [ ] Kill leader server → New election happens
- [ ] Encrypt after failover → Works with new leader
- [ ] Kill 2nd server → System still works
- [ ] Restart killed server → Rejoins as follower

---

## 📡 API Reference

### Client Management
```bash
# Sign up
curl -X POST http://127.0.0.1:3001/api/signup

# Sign out
curl -X POST http://127.0.0.1:3001/api/signout

# Check status
curl http://127.0.0.1:3001/api/status
```

### Peer Discovery
```bash
# View online peers
curl http://127.0.0.1:3001/api/online-peers | python3 -m json.tool
```

### Image Operations
```bash
# My images
curl http://127.0.0.1:3001/api/my-images

# Encrypt image
curl -X POST http://127.0.0.1:3001/api/encrypt-image/image1.jpg

# View shared images
curl http://127.0.0.1:3002/api/shared-images
```

### Access Requests
```bash
# Request access
curl -X POST "http://127.0.0.1:3002/api/request-image?image_id=ID&owner_id=client_1"

# View pending requests
curl http://127.0.0.1:3001/api/pending-requests

# Respond to request
curl -X POST http://127.0.0.1:3001/api/respond-request/req_1_client_2/true
```

---

## 🔍 Monitoring

### Check Firebase Database
```bash
export FIREBASE_DATABASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/"

# View all clients
curl -s "${FIREBASE_DATABASE_URL}/clients.json" | python3 -m json.tool

# View pending requests
curl -s "${FIREBASE_DATABASE_URL}/pending_requests.json" | python3 -m json.tool

# View counters
curl -s "${FIREBASE_DATABASE_URL}/metadata.json" | python3 -m json.tool
```

### Check Encrypted Images
```bash
ls -lh test_images/encrypted/
```

Should see files like:
- `client_1_image1.jpg`
- `client_2_image1.jpg`
- `client_3_image1.jpg`

---

## 🧪 Fault Tolerance Demo

```bash
# 1. All 3 servers running, Server 1 is leader

# 2. Encrypt an image (works fine)
curl -X POST http://127.0.0.1:3001/api/encrypt-image/image1.jpg

# 3. Kill Server 1 (Ctrl+C in Terminal 2)
#    Watch Terminals 3-4: New leader elected!

# 4. Encrypt another image (still works!)
curl -X POST http://127.0.0.1:3001/api/encrypt-image/image2.jpg

# 5. System is fault-tolerant! ✅
```

---

## 📁 Project Structure

```
cloud-p2p-image-sharing/
├── src/
│   ├── bin/
│   │   ├── dos_server.rs      # DoS server (coordination)
│   │   ├── server.rs           # Processing servers (encryption)
│   │   └── web_server.rs       # Web API clients
│   ├── client/
│   │   ├── dos_client.rs       # DoS client library
│   │   └── middleware.rs       # Fault-tolerant client
│   ├── dos/
│   │   ├── firebase.rs         # Firebase integration
│   │   └── service.rs          # DoS business logic
│   └── common/
│       └── messages.rs         # Protocol definitions
├── config/
│   ├── server1.toml            # Server configs (3)
│   ├── server2.toml
│   ├── server3.toml
│   ├── client1.toml            # Client configs (3)
│   ├── client2.toml
│   └── client3.toml
├── test_images/
│   ├── client1/                # 3 images per client
│   ├── client2/
│   ├── client3/
│   └── encrypted/              # Encrypted output
├── setup_images.sh             # Image setup script
├── demo_test.sh                # Automated test
├── QUICK_START.md              # Quick reference
└── COMPLETE_TESTING_GUIDE.md   # Detailed guide
```

---

## 🛠️ Troubleshooting

| Issue | Solution |
|-------|----------|
| Can't connect to DoS | Start DoS server in Terminal 1 |
| Encryption fails | Wait 5s for leader election |
| Images not found | Run `./setup_images.sh` |
| Already signed up | Restart web server or sign out |
| Port in use | Run `pkill -f "dos_server\|server\|web_server"` |
| Firebase empty | Check FIREBASE_DATABASE_URL environment variable |

---

## 📚 Documentation

- **[QUICK_START.md](QUICK_START.md)** - Commands cheat sheet
- **[COMPLETE_TESTING_GUIDE.md](COMPLETE_TESTING_GUIDE.md)** - Detailed step-by-step guide
- **[RUN_FULL_SYSTEM.md](RUN_FULL_SYSTEM.md)** - Original system architecture doc
- **[RUN_DOS.md](RUN_DOS.md)** - DoS-specific documentation

---

## 🎓 Key Concepts

### Directory of Services (DoS)
- Centralized registry for distributed clients
- Handles authentication and access control
- Stores pending requests and notifications
- Uses Firebase for persistence

### Fault Tolerance
- **Leader Election**: Bully algorithm
- **Automatic Failover**: Clients retry on leader failure
- **No Single Point of Failure**: Any server can be leader

### Request Lifecycle
1. Client signs up → Gets `client_1`
2. Client encrypts image → Stored with metadata
3. Another client requests access → Creates `req_1_client_2`
4. If owner offline → Stored as pending
5. Owner signs in → Receives notifications
6. If requester offline → Request discarded

---

## 🚦 System Status

All features implemented and tested:

✅ DoS integration with Firebase
✅ Incremental client and request IDs
✅ Sign-up, sign-out, peer discovery
✅ Server-side encryption with fault tolerance
✅ Image access requests with limits
✅ Pending request handling
✅ Request expiration logic
✅ Complete REST API
✅ Automated testing script
✅ Single-PC configuration

---

## 🏁 Running the Demo

**Single command to test everything:**

```bash
# 1. Start all components (7 terminals - see Quick Start)

# 2. Run demo
./demo_test.sh

# 3. Watch the magic happen! ✨
```

**Expected output:**
- 3 clients sign up
- All encrypt images
- 1 client signs out
- Encrypted images saved
- Firebase populated with data

---

## 📝 Notes

- **Images**: Currently all clients use same test images (copies of cover_image.jpg). You can replace with your own.
- **IPs**: All configured for localhost (127.0.0.1) for single-PC testing
- **Ports**: DoS:9000, Servers:8001-8003, Clients:3001-3003
- **Firebase**: All data persists between runs (clear with DELETE command)

---

**Ready to test? Start with `./demo_test.sh`!** 🚀
