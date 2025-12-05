# How to Run and Test the Directory of Services (DoS)

## Prerequisites

Make sure you have the Firebase environment variable set:
```bash
export FIREBASE_DATABASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/"
```

## Step 1: Populate Firebase (One-time setup)

Clear old data and populate with fresh client records:
```bash
# Clear Firebase
curl -X DELETE "$FIREBASE_DATABASE_URL/clients.json"

# Populate with sample data
cargo run --example populate_firebase
```

## Step 2: Run the DoS Server

In a terminal window, run:
```bash
cargo run --bin dos_server
```

You should see:
```
[INFO] ==============================================
[INFO]   Directory of Services (DoS) Server
[INFO] ==============================================
[INFO] Firebase URL: https://...
[INFO] Bind address: 127.0.0.1:9000
[INFO] ==============================================
[INFO] DoS server listening on 127.0.0.1:9000
```

Leave this running!

## Step 3: Test the DoS (In another terminal)

Run the comprehensive test suite:
```bash
cargo run --example test_dos
```

This will test:
- ✅ Client sign-up (3 clients)
- ✅ List online clients
- ✅ Register images
- ✅ Image access requests (owner online)
- ✅ Update access rights
- ✅ Client sign-out
- ✅ Image access requests (owner offline) - stored as pending
- ✅ Sign-in with pending notifications
- ✅ Request expiration (requester goes offline)
- ✅ Peer failure reporting
- ✅ Final online client list

## Quick Test Commands

### Test 1: Sign up a new client
Client IDs are automatically assigned as incremental numbers (client_1, client_2, client_3, etc.)

```bash
# Create a simple test client
cat > /tmp/test_client.rs << 'EOF'
use cloud_p2p::client::DosClient;

#[tokio::main]
async fn main() {
    let mut client = DosClient::new("127.0.0.1:9000".to_string(), "MyTestClient".to_string());

    match client.sign_up("192.168.1.200".to_string()).await {
        Ok(id) => println!("✅ Signed up! Client ID: {}", id),  // e.g., "client_1"
        Err(e) => println!("❌ Error: {}", e),
    }
}
EOF
```

### Test 2: Check Firebase Console
Visit your Firebase Console:
https://console.firebase.google.com/project/distributed-p2p/database/distributed-p2p-default-rtdb/data

You should see:
- `/clients/` - All registered clients
- `/pending_requests/` - Any pending access requests
- `/offline_notifications/` - Notifications for offline clients

## Troubleshooting

### Issue: "Failed to initialize DoS - missing field"
**Solution:** Clear and repopulate Firebase:
```bash
curl -X DELETE "$FIREBASE_DATABASE_URL/clients.json"
cargo run --example populate_firebase
```

### Issue: "Connection refused"
**Solution:** Make sure DoS server is running on port 9000:
```bash
netstat -tulpn | grep 9000
```

### Issue: "Firebase URL not set"
**Solution:** Export the environment variable:
```bash
export FIREBASE_DATABASE_URL="https://distributed-p2p-default-rtdb.europe-west1.firebasedatabase.app/"
```

## What Each Component Does

### DoS Server (`dos_server`)
- Listens on port 9000
- Manages client registration
- Tracks online/offline status
- Handles image access requests
- Stores pending requests when owner is offline
- Sends notifications when clients sign back in

### Test Suite (`test_dos`)
- Simulates 3 clients
- Tests all DoS functionality
- Verifies pending request handling
- Tests request expiration logic

### Population Script (`populate_firebase`)
- Creates 5 sample clients in Firebase
- Each client has 2 sample images
- All clients start offline

## Next Steps

Once the DoS is running and tested:

1. **Integrate with existing clients** - Modify your client code to use `DosClient` for:
   - Signing in on startup
   - Registering images
   - Requesting access to peer images
   - Reporting peer failures

2. **Check Firebase data** - Verify client records in Firebase Console

3. **Test P2P scenarios**:
   - Client A registers an image
   - Client B requests access while Client A is online
   - Client A approves
   - Client B can now access the image

4. **Test offline scenarios**:
   - Client A goes offline
   - Client B requests access
   - Request stored as pending
   - Client A signs back in
   - Client A receives notification
   - Client A approves
   - Client B gets notified (if online)
