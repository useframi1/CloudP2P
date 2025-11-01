# CloudP2P Stress Testing - Quick Start

## TL;DR

```bash
# 1. Build (on all machines)
cargo build --release

# 2. Setup SSH keys (from controller)
ssh-copy-id user@server1
ssh-copy-id user@client1
# ... for all machines

# 3. Configure (edit these files)
config/client_stress.toml          # Server addresses
scripts/config/stress_test.conf    # MACHINE_ID (unique per client!)
scripts/config/fault_sim.conf      # Server SSH details
scripts/config/aggregate.conf      # Client machine list

# 4. Start servers (on each server)
./target/release/server --config config/server1.toml

# 5. Start clients (on each client machine)
./scripts/stress_test.sh

# 6. Start fault simulation (on controller, parallel to step 5)
./scripts/fault_simulation.sh

# 7. Aggregate results (after tests complete)
./scripts/aggregate_metrics.sh
cat aggregated_metrics/final_report.txt
```

## Configuration Checklist

### On Each Server Machine

- [ ] Build: `cargo build --release`
- [ ] Create `config/serverN.toml` with correct ID and peer addresses
- [ ] Start server: `./target/release/server --config config/serverN.toml`

### On Each Client Machine

- [ ] Build: `cargo build --release`
- [ ] Create `test_images/` directory with 3 images
- [ ] Copy and edit `config/client_stress.toml` (server addresses)
- [ ] Edit `scripts/config/stress_test.conf`:
  - **IMPORTANT:** Set unique `MACHINE_ID` (1, 2, 3, ...)
  - Set `NUM_CLIENTS`, `REQUESTS_PER_CLIENT`
  - Set `MIN_DELAY_MS`, `MAX_DELAY_MS`

### On Controller Machine

- [ ] Setup SSH keys to all servers and clients
- [ ] Edit `scripts/config/fault_sim.conf`:
  - Server SSH details (`SERVER_N_HOST`, `SERVER_N_CONFIG`, etc.)
  - Timing: `FAULT_INTERVAL_SECS`, `RESTART_DELAY_SECS`, `NUM_CYCLES`
- [ ] Edit `scripts/config/aggregate.conf`:
  - List all client machines in `CLIENT_MACHINES` array

## File Structure

```
CloudP2P/
├── config/
│   ├── client_stress.toml         # Client config (edit server addresses)
│   └── server*.toml                # Server configs
├── scripts/
│   ├── stress_test.sh              # Run on clients
│   ├── fault_simulation.sh         # Run on controller
│   ├── aggregate_metrics.sh        # Run on controller
│   └── config/
│       ├── stress_test.conf        # Client machine params
│       ├── fault_sim.conf          # Server SSH details
│       └── aggregate.conf          # Client machine list
├── test_images/                    # Place 3 images here
├── metrics/                        # Auto-created by stress test
└── docs/
    └── STRESS_TESTING.md           # Full documentation
```

## Common Issues

| Issue | Solution |
|-------|----------|
| "Connection refused" | Check servers are running, firewall open |
| SSH fails | Run `ssh-copy-id user@host` |
| No metrics generated | Check logs in `metrics/machine_*_client_*.log` |
| High failure rate | Reduce NUM_CLIENTS or increase delays |
| Python error | Ensure Python 3 is installed |

## Typical Test Scenario

**Setup:**
- 3 servers (different machines)
- 3 client machines (100 clients each = 300 total)
- 1000 requests per client = 300,000 total requests
- Random delay: 100-2000ms between requests
- Fault simulation: 30s interval, 10s downtime, 3 cycles

**Expected Duration:**
- Client tests: ~25-30 minutes
- Fault simulation: ~6 minutes (3 servers × 40s × 3 cycles)

**Expected Results:**
- Latency: P50 < 300ms, P99 < 1000ms
- Load balancing: ~33% per server
- Failure rate: <1%

## Monitoring During Test

```bash
# On client machine - watch active clients
watch -n 5 'pgrep -f "client --config" | wc -l'

# On client machine - tail specific client log
tail -f metrics/machine_1_client_1.log

# On server - monitor resources
ssh server1 "top -bn1 | head -20"

# On controller - watch fault simulation log
tail -f fault_events.log
```

## Quick Commands

```bash
# Count total requests in all metrics
find metrics -name "*.json" -exec jq '.aggregated_stats.total_requests' {} + | awk '{s+=$1} END {print s}'

# Check failure rate for a specific client
jq '.aggregated_stats.failure_rate' metrics/machine_1_client_1.json

# List top failure reasons across all clients
find metrics -name "*.json" -exec jq -r '.aggregated_stats.failure_reasons | to_entries[] | "\(.value) \(.key)"' {} + | sort -rn | head -10

# Get average latency from all clients
find metrics -name "*.json" -exec jq '.aggregated_stats.latency_avg_ms' {} + | awk '{s+=$1; c++} END {print s/c}'
```

## Need Help?

See the full guide: [docs/STRESS_TESTING.md](./STRESS_TESTING.md)
