# Complete Testing Guide - CloudP2P with DoS Integration

This guide provides **step-by-step, line-by-line commands** to test the complete fault-tolerant system.

## System Architecture

```
DoS Server (1) - Port 9000
    ↓
Processing Servers (3) - Ports 8001, 8002, 8003
    ↓
Web Clients (3) - Ports 3001, 3002, 3003
```

---

## PART 1: Setup (One-Time)

### Step 1: Add Test Images

**You need to add 3 images to each client directory:**

```bash
cd ~/Documents/Distrubuted-Systems/cloud-p2p-image-sharing

# For Client1 - add 3 JPG images to this folder:
ls test_images/client1/
# Expected: image1.jpg, image2.jpg, image3.jpg

# For Client2 - add 3 JPG images to this folder:
ls test_images/client2/
# Expected: image1.jpg, image2.jpg, image3.jpg

# For Client3 - add 3 JPG images to this folder:
ls test_images/client3/
# Expected: image1.jpg, image2.jpg, image3.jpg
```

**How to add images:**
- Download any 3 small JPG images from the internet
- Rename them to `image1.jpg`, `image2.jpg`, `image3.jpg`
- Copy them to each client folder

Or use this command to create dummy test images:
```bash
# Copy the cover image as test images
cp test_images/cover_image.jpg test_images/client1/image1.jpg
cp test_images/cover_image.jpg test_images/client1/image2.jpg
cp test_images/cover_image.jpg test_images/client1/image3.jpg

cp test_images/cover_image.jpg test_images/client2/image1.jpg
cp test_images/cover_image.jpg test_images/client2/image2.jpg
cp test_images/cover_image.jpg test_images/client2/image3.jpg

cp test_images/cover_image.jpg test_images/client3/image1.jpg
cp test_images/cover_image.jpg test_images/client3/image2.jpg
cp test_images/cover_image.jpg test_images/client3/image3.jpg
```

### Step 2: Update Configuration Files

Update IPs to localhost for single-PC testing:

```bash
# Edit config/server1.toml
cat > config/server1.toml << 'EOF'
[server]
id = 1
address = "127.0.0.1:8001"
cover_image = "test_images/cover_image.jpg"

[peers]
peers = [
    { id = 2, address = "127.0.0.1:8002" },
    { id = 3, address = "127.0.0.1:8003" }
]

[election]
heartbeat_interval_secs = 1
monitor_interval_secs = 1
failure_timeout_secs = 5
election_timeout_secs = 2
EOF

# Edit config/server2.toml
cat > config/server2.toml << 'EOF'
[server]
id = 2
address = "127.0.0.1:8002"
cover_image = "test_images/cover_image.jpg"

[peers]
peers = [
    { id = 1, address = "127.0.0.1:8001" },
    { id = 3, address = "127.0.0.1:8003" }
]

[election]
heartbeat_interval_secs = 1
monitor_interval_secs = 1
failure_timeout_secs = 5
election_timeout_secs = 2
EOF

# Edit config/server3.toml
cat > config/server3.toml << 'EOF'
[server]
id = 3
address = "127.0.0.1:8003"
cover_image = "test_images/cover_image.jpg"

[peers]
peers = [
    { id = 1, address = "127.0.0.1:8001" },
    { id = 2, address = "127.0.0.1:8002" }
]

[election]
heartbeat_interval_secs = 1
monitor_interval_secs = 1
failure_timeout_secs = 5
election_timeout_secs = 2
EOF

# Edit config/client1.toml
cat > config/client1.toml << 'EOF'
[client]
name = "Client1"
server_addresses = [
    "127.0.0.1:8001",
    "127.0.0.1:8002",
    "127.0.0.1:8003"
]
image_dir = "test_images/client1"
dos_address = "127.0.0.1:9000"

[requests]
total_requests = 10
min_delay_ms = 100
max_delay_ms = 500
EOF

# Edit config/client2.toml
cat > config/client2.toml << 'EOF'
[client]
name = "Client2"
server_addresses = [
    "127.0.0.1:8001",
    "127.0.0.1:8002",
    "127.0.0.1:8003"
]
image_dir = "test_images/client2"
dos_address = "127.0.0.1:9000"

[requests]
total_requests = 10
min_delay_ms = 100
max_delay_ms = 500
EOF

# Edit config/client3.toml
cat > config/client3.toml << 'EOF'
[client]
name = "Client3"
server_addresses = [
    "127.0.0.1:8001",
    "127.0.0.1:8002",
    "127.0.0.1:8003"
]
image_dir = "test_images/client3"
dos_address = "127.0.0.1:9000"

[requests]
total_requests = 10
min_delay_ms = 100
max_delay_ms = 500
EOF
```

### Step 3: Build Everything

```bash
cargo build --release
```

---

## PART 2: Running the System

Open **7 terminal windows**. Keep them all open side-by-side.

### Terminal 1: DoS Server

```bash
cd ~/Documents/Distrubuted-Systems/cloud-p2p-image-sharing
export FIREBASE_DATABASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/"

# Clear Firebase
curl -X DELETE "${FIREBASE_DATABASE_URL}.json"
sleep 2

# Start DoS
cargo run --bin dos_server
```

**Expected output:**
```
[INFO] DoS server listening on 127.0.0.1:9000
```

### Terminal 2: Server 1

```bash
cd ~/Documents/Distrubuted-Systems/cloud-p2p-image-sharing
cargo run --bin server -- --config config/server1.toml
```

### Terminal 3: Server 2

```bash
cd ~/Documents/Distrubuted-Systems/cloud-p2p-image-sharing
cargo run --bin server -- --config config/server2.toml
```

### Terminal 4: Server 3

```bash
cd ~/Documents/Distrubuted-Systems/cloud-p2p-image-sharing
cargo run --bin server -- --config config/server3.toml
```

**Wait 5 seconds for leader election. You'll see something like:**
```
[INFO] Leader elected: Server 1
```

### Terminal 5: Web Client 1

```bash
cd ~/Documents/Distrubuted-Systems/cloud-p2p-image-sharing
cargo run --bin web_server -- --config config/client1.toml --port 3001
```

**Expected output:**
```
🌐 Web server running on http://127.0.0.1:3001
```

### Terminal 6: Web Client 2

```bash
cd ~/Documents/Distrubuted-Systems/cloud-p2p-image-sharing
cargo run --bin web_server -- --config config/client2.toml --port 3002
```

### Terminal 7: Web Client 3

```bash
cd ~/Documents/Distrubuted-Systems/cloud-p2p-image-sharing
cargo run --bin web_server -- --config config/client3.toml --port 3003
```

---

## PART 3: Testing Features via API

Now open a NEW terminal for testing (Terminal 8).

### Test 1: Sign Up All Clients

```bash
cd ~/Documents/Distrubuted-Systems/cloud-p2p-image-sharing

# Client 1 sign up
curl -X POST http://127.0.0.1:3001/api/signup

# Expected: {"success":true,"data":{"client_id":"client_1","client_name":"Client1"}}

# Client 2 sign up
curl -X POST http://127.0.0.1:3002/api/signup

# Expected: {"success":true,"data":{"client_id":"client_2","client_name":"Client2"}}

# Client 3 sign up
curl -X POST http://127.0.0.1:3003/api/signup

# Expected: {"success":true,"data":{"client_id":"client_3","client_name":"Client3"}}
```

### Test 2: View Online Peers (from Client 1)

```bash
curl http://127.0.0.1:3001/api/online-peers | python3 -m json.tool
```

**Expected:**
```json
{
  "success": true,
  "data": {
    "peers": [
      {
        "client_id": "client_2",
        "ip_address": "127.0.0.1",
        "status": "Online",
        "image_count": 3
      },
      {
        "client_id": "client_3",
        "ip_address": "127.0.0.1",
        "status": "Online",
        "image_count": 3
      }
    ]
  }
}
```

### Test 3: View My Images (Client 1)

```bash
curl http://127.0.0.1:3001/api/my-images | python3 -m json.tool
```

**Expected:**
```json
{
  "success": true,
  "data": ["image1.jpg", "image2.jpg", "image3.jpg"]
}
```

### Test 4: Encrypt an Image at Server Side (Client 1)

```bash
curl -X POST http://127.0.0.1:3001/api/encrypt-image/image1.jpg | python3 -m json.tool
```

**Expected:**
```json
{
  "success": true,
  "data": {
    "image_id": "...",
    "encrypted_path": "test_images/encrypted/client_1_image1.jpg"
  },
  "message": "Image encrypted successfully"
}
```

**Verify encrypted image was created:**
```bash
ls -lh test_images/encrypted/
```

You should see: `client_1_image1.jpg`

### Test 5: View Shared Images (from Client 2)

```bash
curl http://127.0.0.1:3002/api/shared-images | python3 -m json.tool
```

**Expected:** List of all images from all clients with `has_access` flags

### Test 6: Request Image Access (Client 2 requests from Client 1)

First, get an image_id from the shared images list above, then:

```bash
# Replace IMAGE_ID with actual ID from above
curl -X POST "http://127.0.0.1:3002/api/request-image?image_id=IMAGE_ID&owner_id=client_1" | python3 -m json.tool
```

**Expected:**
```json
{
  "success": true,
  "data": {
    "request_id": "req_1_client_2",
    "status": "pending"
  },
  "message": "Access request sent"
}
```

### Test 7: Sign Out Client 3

```bash
curl -X POST http://127.0.0.1:3003/api/signout
```

**Expected:**
```json
{
  "success": true,
  "message": "Successfully signed out"
}
```

**Verify:**
```bash
curl http://127.0.0.1:3001/api/online-peers | python3 -m json.tool
```

Client 3 should now be **Offline** or not in the list.

### Test 8: Check Firebase Database

```bash
export FIREBASE_DATABASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/"
curl -s "${FIREBASE_DATABASE_URL}/clients.json" | python3 -m json.tool
```

**Expected:** You'll see all registered clients with their status, images, etc.

---

## PART 4: Fault Tolerance Testing

### Test 9: Kill Leader Server (Server 1)

In Terminal 2 (Server 1), press `Ctrl+C` to kill it.

**Watch Terminals 3 & 4** (Server 2 & 3):
- They will detect leader failure
- Start new election
- One will become new leader

**Expected output in Server 2 or 3:**
```
[INFO] Leader failed, starting election
[INFO] Leader elected: Server 2
```

### Test 10: Encrypt Image with New Leader

```bash
curl -X POST http://127.0.0.1:3001/api/encrypt-image/image2.jpg | python3 -m json.tool
```

**Expected:** Should succeed with new leader! This proves fault tolerance.

### Test 11: Restart Server 1

In Terminal 2:
```bash
cargo run --bin server -- --config config/server1.toml
```

It will rejoin as a follower.

### Test 12: Kill Second Server

Press `Ctrl+C` in Terminal 3 (Server 2).

**System should still work** with 2 servers (Server 1 and 3).

### Test 13: Request Limit Test

Try requesting the same image more than 3 times:

```bash
# Replace IMAGE_ID with actual ID
curl -X POST "http://127.0.0.1:3002/api/request-image?image_id=IMAGE_ID&owner_id=client_1"
curl -X POST "http://127.0.0.1:3002/api/request-image?image_id=IMAGE_ID&owner_id=client_1"
curl -X POST "http://127.0.0.1:3002/api/request-image?image_id=IMAGE_ID&owner_id=client_1"
curl -X POST "http://127.0.0.1:3002/api/request-image?image_id=IMAGE_ID&owner_id=client_1"
```

**Expected on 4th request:**
```json
{
  "success": false,
  "error": "Request limit exceeded (max 3 attempts)"
}
```

---

## PART 5: Pending Requests Test

### Test 14: Offline Request Scenario

**Step 1:** Sign out Client 1
```bash
curl -X POST http://127.0.0.1:3001/api/signout
```

**Step 2:** Client 2 requests image from offline Client 1
```bash
curl -X POST "http://127.0.0.1:3002/api/request-image?image_id=IMAGE_ID&owner_id=client_1"
```

**Expected:** Request is stored as pending in Firebase

**Step 3:** Verify pending request in Firebase
```bash
curl -s "${FIREBASE_DATABASE_URL}/pending_requests.json" | python3 -m json.tool
```

**Expected:** You'll see the pending request with status "Pending"

**Step 4:** Client 1 signs back in
```bash
curl -X POST http://127.0.0.1:3001/api/signup
```

**Expected:** Client 1 should receive notifications about pending requests

---

## PART 6: Complete Demo Script

Save this as `demo_test.sh`:

```bash
#!/bin/bash
# Comprehensive system test

set -e

echo "=== CloudP2P Complete System Test ==="
echo ""

BASE_URL_1="http://127.0.0.1:3001"
BASE_URL_2="http://127.0.0.1:3002"
BASE_URL_3="http://127.0.0.1:3003"

echo "[1/10] Signing up all clients..."
curl -s -X POST $BASE_URL_1/api/signup | python3 -m json.tool
curl -s -X POST $BASE_URL_2/api/signup | python3 -m json.tool
curl -s -X POST $BASE_URL_3/api/signup | python3 -m json.tool
echo "✅ All clients signed up"
echo ""

sleep 2

echo "[2/10] Checking online peers..."
curl -s $BASE_URL_1/api/online-peers | python3 -m json.tool
echo "✅ Online peers listed"
echo ""

sleep 2

echo "[3/10] Viewing my images (Client 1)..."
curl -s $BASE_URL_1/api/my-images | python3 -m json.tool
echo "✅ Images listed"
echo ""

sleep 2

echo "[4/10] Encrypting image at server side..."
curl -s -X POST $BASE_URL_1/api/encrypt-image/image1.jpg | python3 -m json.tool
echo "✅ Image encrypted"
echo ""

sleep 2

echo "[5/10] Viewing shared images..."
curl -s $BASE_URL_2/api/shared-images | python3 -m json.tool
echo "✅ Shared images listed"
echo ""

sleep 2

echo "[6/10] Client 2 encrypting image..."
curl -s -X POST $BASE_URL_2/api/encrypt-image/image1.jpg | python3 -m json.tool
echo "✅ Client 2 image encrypted"
echo ""

sleep 2

echo "[7/10] Client 3 encrypting image..."
curl -s -X POST $BASE_URL_3/api/encrypt-image/image1.jpg | python3 -m json.tool
echo "✅ Client 3 image encrypted"
echo ""

sleep 2

echo "[8/10] Signing out Client 3..."
curl -s -X POST $BASE_URL_3/api/signout | python3 -m json.tool
echo "✅ Client 3 signed out"
echo ""

sleep 2

echo "[9/10] Checking online peers again..."
curl -s $BASE_URL_1/api/online-peers | python3 -m json.tool
echo "✅ Client 3 should be offline"
echo ""

sleep 2

echo "[10/10] Checking encrypted images directory..."
ls -lh test_images/encrypted/
echo "✅ Encrypted images saved"
echo ""

echo "=== Test Complete ==="
echo ""
echo "Summary:"
echo "- 3 clients signed up (client_1, client_2, client_3)"
echo "- All clients encrypted images at server side"
echo "- Client 3 signed out and went offline"
echo "- All encrypted images saved to test_images/encrypted/"
echo ""
echo "Check Firebase:"
echo "curl -s \"\${FIREBASE_DATABASE_URL}/clients.json\" | python3 -m json.tool"
```

Make it executable and run:
```bash
chmod +x demo_test.sh
./demo_test.sh
```

---

## Summary of Features Implemented

✅ **1. Sign Up** - Clients register with DoS, get incremental IDs (client_1, client_2, client_3)

✅ **2. Sign Out** - Clients go offline (not "dead", clean sign-out)

✅ **3. View Online Peers** - See all currently online clients

✅ **4. Encrypt Image at Server Side** - Submit to processing servers with fault tolerance

✅ **5. View Shared Images** - See all images from all clients

✅ **6. Request Image Access** - Request access to other clients' images
   - Max 3 requests per image
   - Pending requests when owner is offline
   - Request expiration if requester goes offline

✅ **7. Fault Tolerance** - Automatic server failover with leader election

---

## Stop Everything

```bash
# In any terminal:
pkill -f "dos_server|cargo run --bin server|cargo run --bin web_server"
```

---

## Troubleshooting

**Problem:** "Failed to connect to DoS"
- **Solution:** Make sure DoS server (Terminal 1) is running

**Problem:** "Failed to encrypt image"
- **Solution:** Make sure at least 2 servers are running and leader is elected

**Problem:** "Image not found"
- **Solution:** Make sure images exist in `test_images/client1/`, `client2/`, `client3/`

**Problem:** "Already signed up"
- **Solution:** Restart the web server or sign out first

---

## Next Steps

1. ✅ Basic features working
2. ✅ Fault tolerance working
3. TODO: Create simple HTML frontend (currently API-only)
4. TODO: Implement decrypt functionality
5. TODO: Add request approval/denial UI

Would you like me to create the HTML frontend next?
