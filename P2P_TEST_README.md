# P2P Testing Guide

## Bug Fixed

**Issue**: Pending access requests were not being stored when the owner was online.

**Root Cause**: In [src/dos/service.rs:114-120](src/dos/service.rs#L114-L120), the `request_image_access()` method only stored pending requests if the owner was OFFLINE. Since both clients were online during testing, requests were never saved to Firebase.

**Fix**: Removed the conditional check - pending requests are now always stored regardless of online status.

## How to Test

### Step 1: Rebuild the project
```bash
cargo build --release
```

### Step 2: Start all servers
Run this in a separate terminal and leave it running:
```bash
bash start_servers.sh
```

This will start:
- DoS Server on port 9000
- Client 2 web server (HTTP: 3002, P2P: 7002)
- Client 3 web server (HTTP: 3003, P2P: 7003)

### Step 3: Run the P2P test
In another terminal:
```bash
bash test_p2p.sh
```

Expected output:
```
✅ TEST PASSED - P2P Transfer Successful!
```

You should see emoji-based debug logs in the server terminal showing:
- Client 2: 🔌 [P2P_REQUEST] lines showing the P2P request
- Client 3: 🔐 [P2P_HANDLER] lines showing access verification and image sending

### Step 4: Check Firebase state (optional)
```bash
bash check_firebase.sh
```

This shows pending requests and client status in Firebase.

## Test Flow

1. Client 3 signs in
2. Client 2 signs in
3. Client 2 requests access to claude.png from Client 3
4. Client 3 retrieves pending requests (should now show req_X_client2)
5. Client 3 approves with view_limit=5
6. Client 2 views image via direct P2P connection
7. Success confirmation

## Troubleshooting

### Quick Diagnosis

Run the diagnostic script to check all components:
```bash
bash diagnose_p2p.sh
```

This checks:
- Server accessibility (HTTP ports)
- P2P listener ports
- Firebase client status and P2P ports
- Client3's images and access rights
- Image files on disk

### Common Issues

**"Image not found" error:**

1. **Servers using old code**: Kill and restart servers
   ```bash
   pkill -f dos_server && pkill -f web_server
   bash start_servers.sh
   ```

2. **P2P ports not listening**: Check diagnostic output - ports 7002 and 7003 should show "LISTENING"

3. **Access rights not granted**:
   - Check Firebase shows client2 in claude.png access_rights
   - Try running Steps 3-6 of test again

4. **Wrong image_dir**: Verify config/client3.toml has `image_dir = "test_images/client3"`

**No P2P logs appearing:**

- Make sure you're watching the terminal where `start_servers.sh` is running
- Logs use emojis (🔌 🔐 ✅) for easy visual scanning
- If servers are in background, check with `journalctl` or redirect output to a file

**Test hangs or times out:**

- DoS server might not be running - check port 9000
- Firebase connection issue - verify FIREBASE_DATABASE_URL is set
- Network/firewall blocking localhost connections
