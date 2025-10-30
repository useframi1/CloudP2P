# CloudP2P Distributed System - Test Suite

Comprehensive integration tests for validating fault tolerance, leader election, and system reliability of the CloudP2P distributed system.

## Overview

This test suite validates the following aspects of the distributed system:

- **Leader Election**: Modified Bully Algorithm with load-based priority
- **Fault Tolerance**: Server failure detection and recovery
- **Load Balancing**: Task distribution across servers
- **Client Retry Logic**: Automatic recovery from failures
- **System Resilience**: Operation under stress and failure conditions

## Prerequisites

### Required Software

1. **Rust & Cargo**: For building the project
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **ImageMagick** (optional): For generating test images
   ```bash
   # macOS
   brew install imagemagick

   # Ubuntu/Debian
   sudo apt-get install imagemagick
   ```

### Build the Project

```bash
cd /path/to/CloudP2P
cargo build --release
```

## Test Structure

```
tests/
├── integration_test.sh      # Main test orchestration script
├── verify_results.sh         # Result verification utility
└── README.md                 # This file

config/test/
├── server1.toml              # Test server 1 configuration
├── server2.toml              # Test server 2 configuration
├── server3.toml              # Test server 3 configuration
├── client_test1.toml         # Test client 1 configuration
├── client_test2.toml         # Test client 2 configuration
└── client_stress.toml        # Stress test client configuration

test_results/
├── logs/                     # Server and client logs
│   ├── server1.log
│   ├── server2.log
│   ├── server3.log
│   ├── TestClient1.log
│   └── ...
└── verification.log          # Result verification log
```

## Running Tests

### Run All Tests

```bash
cd tests
./integration_test.sh
```

This will execute all 10 test scenarios and provide a summary of results.

### Run Individual Test

To run a specific test, modify the `main()` function in `integration_test.sh` to comment out unwanted tests:

```bash
# Edit integration_test.sh and comment out tests you don't want to run
run_test "Basic Leader Election" test_basic_leader_election
# run_test "Basic Task Processing" test_basic_task_processing
# ... (comment out other tests)
```

### Verify Results

After running tests, verify the encrypted output files:

```bash
./verify_results.sh
```

## Test Scenarios

### 1. Basic Leader Election
**Purpose**: Verify that servers can elect a leader using the Modified Bully Algorithm

**Steps**:
1. Start all 3 servers
2. Wait for election to complete
3. Verify exactly one leader was elected

**Expected Outcome**: ✓ One server becomes leader within 5 seconds

---

### 2. Basic Task Processing
**Purpose**: Verify end-to-end task processing with a single client

**Steps**:
1. Start all 3 servers
2. Start one client with 5 requests
3. Wait for client completion
4. Verify encrypted files created

**Expected Outcome**: ✓ All tasks complete successfully and encrypted images are saved

---

### 3. Concurrent Clients
**Purpose**: Verify system handles multiple concurrent clients

**Steps**:
1. Start all 3 servers
2. Start 2 clients simultaneously
3. Verify both clients complete their tasks

**Expected Outcome**: ✓ Both clients complete tasks without interference

---

### 4. Leader Failure & Re-election
**Purpose**: Verify system recovers when leader fails

**Steps**:
1. Start all 3 servers
2. Identify leader
3. Start client
4. Kill leader during client execution
5. Verify re-election occurs
6. Verify client completes tasks

**Expected Outcome**: ✓ New leader elected, client recovers and completes tasks

---

### 5. Worker Server Failure
**Purpose**: Verify system continues when non-leader server fails

**Steps**:
1. Start all 3 servers
2. Identify and kill a non-leader server
3. Start client
4. Verify tasks complete

**Expected Outcome**: ✓ System continues with remaining servers

---

### 6. Multiple Server Failures
**Purpose**: Verify system operates with only one server remaining

**Steps**:
1. Start all 3 servers
2. Kill 2 servers
3. Start client with only 1 server available
4. Verify tasks complete

**Expected Outcome**: ✓ Single server handles all tasks

---

### 7. Server Recovery
**Purpose**: Verify server can rejoin cluster after failure

**Steps**:
1. Start all 3 servers
2. Stop one server
3. Restart the stopped server
4. Verify it rejoins the cluster
5. Start client and verify system works

**Expected Outcome**: ✓ Recovered server participates in cluster

---

### 8. Rapid Leader Changes
**Purpose**: Verify system stability under repeated leader failures

**Steps**:
1. Start all servers and client
2. Repeatedly kill and restart leader
3. Verify client completes some tasks

**Expected Outcome**: ✓ System maintains operation despite chaos

---

### 9. High Concurrent Load
**Purpose**: Verify system handles high request rate

**Steps**:
1. Start all 3 servers
2. Start stress client (2 req/sec for 15 seconds)
3. Verify all tasks complete
4. Verify all servers still running

**Expected Outcome**: ✓ System handles load without failures

---

### 10. Client Retry Mechanism
**Purpose**: Verify client retry logic when servers are unavailable

**Steps**:
1. Start only 1 server initially
2. Start client
3. Start remaining servers during client execution
4. Verify client discovers servers and completes tasks

**Expected Outcome**: ✓ Client retries and discovers servers successfully

---

## Test Configuration

### Server Configuration (test/server*.toml)

```toml
[server]
id = 1
address = "127.0.0.1:9001"

[peers]
peers = [
    { id = 2, address = "127.0.0.1:9002" },
    { id = 3, address = "127.0.0.1:9003" }
]

[election]
heartbeat_interval_secs = 1      # Heartbeat every 1 second
election_timeout_secs = 2        # Wait 2 seconds for election responses
failure_timeout_secs = 4         # Consider peer failed after 4 seconds
monitor_interval_secs = 1        # Check for failures every 1 second
```

**Note**: Test configurations use ports 9001-9003 (different from production 8001-8003) to avoid conflicts.

### Client Configuration (test/client*.toml)

```toml
[client]
name = "TestClient1"
server_addresses = [
    "127.0.0.1:9001",
    "127.0.0.1:9002",
    "127.0.0.1:9003"
]

[requests]
rate_per_second = 0.5            # 1 request every 2 seconds
duration_seconds = 10.0          # Run for 10 seconds (5 total requests)
request_processing_ms = 5000     # Expected processing time
load_per_request = 0.1           # Load per request
```

## Interpreting Results

### Success Indicators

- **Leader Election**: Logs show "won election" and "acknowledges X as LEADER"
- **Task Completion**: Logs show "completed successfully" for each task
- **Fault Recovery**: Logs show "LEADER X appears to have failed" followed by "initiating election"
- **Client Retry**: Logs show "Retry attempt X/3" when recovering from failures

### Checking Logs

All logs are saved in `test_results/logs/`:

```bash
# View server logs
tail -f test_results/logs/server1.log

# View client logs
tail -f test_results/logs/TestClient1.log

# Search for errors
grep -r "ERROR\|FAILED" test_results/logs/

# Check for leader elections
grep -r "won election" test_results/logs/
```

### Common Log Patterns

```
✓ Success:
  - "Server X won election! (lowest priority score: Y)"
  - "Task #X completed successfully"
  - "Saved encrypted image for task #X"
  - "Encryption VERIFIED for task #X"

✗ Errors:
  - "Task #X FAILED after 3 attempts"
  - "No leader found for task #X"
  - "Failed to send response to client"
```

## Troubleshooting

### Tests Fail to Start

**Issue**: "Failed to bind to address"
- **Cause**: Ports 9001-9003 already in use
- **Solution**: Kill existing processes:
  ```bash
  lsof -ti:9001,9002,9003 | xargs kill -9
  ```

### Build Failures

**Issue**: Cargo build fails
- **Solution**: Clean and rebuild:
  ```bash
  cargo clean
  cargo build --release
  ```

### No Test Image

**Issue**: "test_image.jpg not found"
- **Solution**: Create a test image:
  ```bash
  # Using ImageMagick
  convert -size 800x600 xc:blue user-data/uploads/test_image.jpg

  # Or copy any JPEG image
  cp /path/to/image.jpg user-data/uploads/test_image.jpg
  ```

### Tests Hang

**Issue**: Tests don't complete
- **Cause**: Server/client processes not terminating
- **Solution**: Manual cleanup:
  ```bash
  pkill -f "cloud-p2p-simple"
  pkill -f "server"
  pkill -f "client"
  ```

### Intermittent Failures

**Issue**: Tests pass sometimes but fail other times
- **Cause**: Race conditions in timing
- **Solution**: Increase timeouts in test configurations
  ```toml
  [election]
  failure_timeout_secs = 6  # Increase from 4
  ```

## Performance Metrics

### Expected Timings

| Test | Duration | Description |
|------|----------|-------------|
| Leader Election | 3-5s | Initial election with 3 servers |
| Re-election | 4-6s | After leader failure |
| Task Processing | 1-2s | Per steganography task |
| Client Retry | 5s | Between retry attempts |

### Resource Usage

- **Memory**: ~10MB per server process
- **CPU**: Low (~5%) during idle, ~20-30% during task processing
- **Network**: Minimal (<1MB/s for heartbeats and small images)

## Extending Tests

### Adding New Test Scenarios

1. Create a new test function in `integration_test.sh`:
   ```bash
   test_your_scenario() {
       print_info "Test: Your scenario description"

       # Test implementation
       start_all_servers
       # ... your test logic

       cleanup
       return 0  # or 1 for failure
   }
   ```

2. Add to the main test runner:
   ```bash
   run_test "Your Scenario" test_your_scenario
   ```

### Creating Custom Configurations

1. Copy an existing config:
   ```bash
   cp config/test/client_test1.toml config/test/client_custom.toml
   ```

2. Modify parameters as needed

3. Use in tests:
   ```bash
   start_client "CustomClient" "$TEST_CONFIG_DIR/client_custom.toml"
   ```

## Known Limitations

1. **Timing Sensitivity**: Tests rely on sleep statements and may need adjustment on slower systems
2. **Port Conflicts**: Tests use fixed ports 9001-9003
3. **Single Machine**: All processes run on localhost (no true network partition testing)
4. **Image Verification**: Automated text extraction verification not yet implemented

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Integration Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Install ImageMagick
        run: sudo apt-get install -y imagemagick
      - name: Run Tests
        run: |
          cd tests
          ./integration_test.sh
```

## Support

For issues or questions:
- Check logs in `test_results/logs/`
- Review test output for specific failure reasons
- Ensure all prerequisites are installed
- Try running tests individually to isolate issues

## License

Same as the main CloudP2P project.
