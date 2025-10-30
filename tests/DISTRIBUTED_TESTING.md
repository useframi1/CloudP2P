# Distributed Multi-Machine Testing Guide

This guide explains how to test CloudP2P across multiple physical machines.

## Overview

The `distributed_test.sh` script orchestrates testing across multiple machines using SSH. It can:
- Deploy code and configs to remote machines
- Start/stop servers and clients remotely
- Collect logs from all machines
- Run coordinated distributed tests

## Prerequisites

### 1. Machine Setup

You need **6 machines** (or fewer if you want to combine roles):
- **3 machines** for servers
- **3 machines** for clients

All machines should:
- Be on the same network
- Have Rust and Cargo installed
- Have the CloudP2P code deployed
- Be accessible via SSH

### 2. SSH Access

Set up passwordless SSH access to all machines:

```bash
# Generate SSH key if you don't have one
ssh-keygen -t rsa -b 4096

# Copy your public key to each machine
ssh-copy-id user@192.168.1.10
ssh-copy-id user@192.168.1.11
ssh-copy-id user@192.168.1.12
ssh-copy-id user@192.168.1.20
ssh-copy-id user@192.168.1.21
ssh-copy-id user@192.168.1.22
```

### 3. Network Configuration

Ensure all machines can communicate:
```bash
# Open firewall port 8001 on all server machines
sudo ufw allow 8001/tcp

# Test connectivity from client machines
nc -zv 192.168.1.10 8001
nc -zv 192.168.1.11 8001
nc -zv 192.168.1.12 8001
```

## Configuration

### 1. Edit the Script

Open `tests/distributed_test.sh` and update the machine configurations:

```bash
# Server machines - replace with your actual IPs and paths
declare -A SERVER_MACHINES=(
    [1]="your_user@192.168.1.10:/home/your_user/CloudP2P"
    [2]="your_user@192.168.1.11:/home/your_user/CloudP2P"
    [3]="your_user@192.168.1.12:/home/your_user/CloudP2P"
)

# Client machines - replace with your actual IPs and paths
declare -A CLIENT_MACHINES=(
    [1]="your_user@192.168.1.20:/home/your_user/CloudP2P"
    [2]="your_user@192.168.1.21:/home/your_user/CloudP2P"
    [3]="your_user@192.168.1.22:/home/your_user/CloudP2P"
)
```

### 2. Update Config Files

Update your server config files to use actual IPs instead of `127.0.0.1`:

**config/server1.toml:**
```toml
[server]
id = 1
address = "192.168.1.10:8001"  # Server 1's IP

[peers]
peers = [
    { id = 2, address = "192.168.1.11:8001" },
    { id = 3, address = "192.168.1.12:8001" }
]
# ... rest of config
```

**config/client1.toml:**
```toml
[client]
name = "Client1"
server_addresses = [
    "192.168.1.10:8001",
    "192.168.1.11:8001",
    "192.168.1.12:8001"
]
# ... rest of config
```

### 3. Deploy Code to Machines

First, manually copy the CloudP2P code to each machine:

```bash
# On your local machine, for each remote machine:
rsync -avz --exclude target --exclude .git . user@192.168.1.10:~/CloudP2P/
rsync -avz --exclude target --exclude .git . user@192.168.1.11:~/CloudP2P/
rsync -avz --exclude target --exclude .git . user@192.168.1.12:~/CloudP2P/
rsync -avz --exclude target --exclude .git . user@192.168.1.20:~/CloudP2P/
rsync -avz --exclude target --exclude .git . user@192.168.1.21:~/CloudP2P/
rsync -avz --exclude target --exclude .git . user@192.168.1.22:~/CloudP2P/
```

## Usage

### Full Automated Test

Run the complete test suite:

```bash
./tests/distributed_test.sh test
```

This will:
1. Verify SSH connectivity
2. Deploy configs to all machines
3. Build the project on each machine
4. Start all servers and wait for leader election
5. Start all clients
6. Run tests and collect results
7. Stop everything and collect logs

### Manual Step-by-Step Testing

#### 1. Deploy
Deploy configs and build on all machines:
```bash
./tests/distributed_test.sh deploy
```

#### 2. Start Everything
Start all servers and clients:
```bash
./tests/distributed_test.sh start
```

#### 3. Monitor
Watch logs in real-time on individual machines:
```bash
# SSH into a server machine
ssh user@192.168.1.10
tail -f ~/CloudP2P/logs/server1.log

# SSH into a client machine
ssh user@192.168.1.20
tail -f ~/CloudP2P/logs/client1.log
```

#### 4. Collect Logs
Gather logs from all machines to local machine:
```bash
./tests/distributed_test.sh logs
```

Logs will be in `test_results/distributed_logs/`

#### 5. Stop Everything
Stop all servers and clients:
```bash
./tests/distributed_test.sh stop
```

## Manual Testing (Without Script)

If you prefer complete manual control:

### On Each Server Machine

```bash
# Machine 1 (192.168.1.10)
ssh user@192.168.1.10
cd ~/CloudP2P
cargo build --release
./target/release/server -c config/server1.toml

# Machine 2 (192.168.1.11)
ssh user@192.168.1.11
cd ~/CloudP2P
cargo build --release
./target/release/server -c config/server2.toml

# Machine 3 (192.168.1.12)
ssh user@192.168.1.12
cd ~/CloudP2P
cargo build --release
./target/release/server -c config/server3.toml
```

### On Each Client Machine

```bash
# Machine 4 (192.168.1.20)
ssh user@192.168.1.20
cd ~/CloudP2P
cargo build --release
./target/release/client -c config/client1.toml

# Machine 5 (192.168.1.21)
ssh user@192.168.1.21
cd ~/CloudP2P
cargo build --release
./target/release/client -c config/client2.toml

# Machine 6 (192.168.1.22)
ssh user@192.168.1.22
cd ~/CloudP2P
cargo build --release
./target/release/client -c config/client3.toml
```

## Verification

Check that the distributed system is working:

### 1. Leader Election
```bash
# Check server logs for leader election
grep "won election" test_results/distributed_logs/server*.log
```

### 2. Task Completion
```bash
# Check client logs for completed tasks
grep "completed successfully" test_results/distributed_logs/client*.log | wc -l
```

### 3. Encrypted Files
```bash
# SSH into client machines and check output directories
ssh user@192.168.1.20 "ls -lh ~/CloudP2P/user-data/outputs/"
```

## Test Scenarios

### Basic Distributed Operation
Tests that servers across machines can elect a leader and process client requests.

### Leader Failure
1. Start all servers on different machines
2. Let them elect a leader
3. Kill the leader machine (or just the server process)
4. Verify remaining servers re-elect
5. Verify clients continue working

### Network Partition Simulation
Temporarily block network communication to simulate partitions:
```bash
# On server machine, block specific IPs
sudo iptables -A INPUT -s 192.168.1.11 -j DROP
sudo iptables -A OUTPUT -d 192.168.1.11 -j DROP

# Later, restore connectivity
sudo iptables -D INPUT -s 192.168.1.11 -j DROP
sudo iptables -D OUTPUT -d 192.168.1.11 -j DROP
```

## Troubleshooting

### SSH Connection Issues
```bash
# Test SSH connectivity
ssh user@192.168.1.10 echo "Connected"

# Check SSH key permissions
chmod 600 ~/.ssh/id_rsa
```

### Port Already in Use
```bash
# Kill existing processes on remote machine
ssh user@192.168.1.10 "pkill -f server"
```

### Build Failures
```bash
# Ensure Rust is installed on remote machines
ssh user@192.168.1.10 "rustc --version"
```

### Network Connectivity
```bash
# Test if port is open
nc -zv 192.168.1.10 8001

# Check firewall
ssh user@192.168.1.10 "sudo ufw status"
```

## Tips

1. **Use tmux/screen**: When manually starting processes, use tmux or screen so they continue running after you disconnect:
   ```bash
   ssh user@192.168.1.10
   tmux new -s server
   cd ~/CloudP2P
   ./target/release/server -c config/server1.toml
   # Press Ctrl+B, then D to detach
   ```

2. **Monitor Resource Usage**: Check CPU/memory on machines during stress tests:
   ```bash
   ssh user@192.168.1.10 "top -b -n 1 | head -20"
   ```

3. **Sync Clocks**: Ensure all machines have synchronized clocks (important for distributed systems):
   ```bash
   sudo apt-get install ntp
   sudo systemctl start ntp
   ```

4. **Use Longer Timeouts**: Network latency between machines is higher than localhost, so you may need to increase timeout values in your configs.
