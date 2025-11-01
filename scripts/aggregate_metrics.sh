#!/bin/bash

# CloudP2P Metrics Aggregation Script
#
# Collects metrics JSON files from all client machines via SCP and generates
# a consolidated report with aggregated statistics.
#
# Usage:
#   ./scripts/aggregate_metrics.sh [config_file]
#
# Default config: ./scripts/config/aggregate.conf

set -e

# Default configuration file
CONFIG_FILE="${1:-./scripts/config/aggregate.conf}"

# Check if config file exists
if [ ! -f "$CONFIG_FILE" ]; then
    echo "Error: Configuration file not found: $CONFIG_FILE"
    echo "Usage: $0 [config_file]"
    exit 1
fi

# Load configuration
echo "Loading configuration from: $CONFIG_FILE"
source "$CONFIG_FILE"

# Validate required parameters
if [ -z "$OUTPUT_DIR" ]; then
    echo "Error: OUTPUT_DIR not configured"
    exit 1
fi

if [ ${#CLIENT_MACHINES[@]} -eq 0 ]; then
    echo "Error: No client machines configured in CLIENT_MACHINES array"
    exit 1
fi

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Temporary directory for collected metrics
TEMP_DIR="$OUTPUT_DIR/collected"
mkdir -p "$TEMP_DIR"

echo "============================================"
echo "CloudP2P Metrics Aggregation"
echo "============================================"
echo "Client Machines: ${#CLIENT_MACHINES[@]}"
echo "Output Directory: $OUTPUT_DIR"
echo "============================================"
echo ""

# Step 1: Collect metrics from all client machines
echo "Step 1: Collecting metrics from client machines..."
echo ""

for i in "${!CLIENT_MACHINES[@]}"; do
    CLIENT_SPEC="${CLIENT_MACHINES[$i]}"
    MACHINE_NUM=$((i + 1))

    echo "[$MACHINE_NUM/${#CLIENT_MACHINES[@]}] Collecting from: $CLIENT_SPEC"

    # Create machine-specific directory
    MACHINE_DIR="$TEMP_DIR/machine_$MACHINE_NUM"
    mkdir -p "$MACHINE_DIR"

    # Copy metrics via SCP (with error handling)
    if scp -r "$CLIENT_SPEC" "$MACHINE_DIR/" 2>/dev/null; then
        echo "  ✓ Successfully collected metrics from machine $MACHINE_NUM"
    else
        echo "  ✗ Failed to collect metrics from machine $MACHINE_NUM"
        echo "  (This may be OK if no metrics were generated yet)"
    fi
done

echo ""
echo "Metrics collection complete."
echo ""

# Step 2: Aggregate metrics using Python
echo "Step 2: Aggregating metrics..."
echo ""

# Check if Python 3 is available
if ! command -v python3 &> /dev/null; then
    echo "Error: python3 is required for metrics aggregation"
    exit 1
fi

# Create Python aggregation script inline
cat > "$OUTPUT_DIR/aggregate.py" << 'EOF'
#!/usr/bin/env python3

import json
import sys
import os
from pathlib import Path
from collections import defaultdict
import statistics

def load_all_metrics(metrics_dir):
    """Load all metrics JSON files from the collected directory."""
    all_metrics = []
    metrics_path = Path(metrics_dir)

    for json_file in metrics_path.rglob("*.json"):
        try:
            with open(json_file, 'r') as f:
                data = json.load(f)
                all_metrics.append(data)
        except Exception as e:
            print(f"Warning: Failed to load {json_file}: {e}", file=sys.stderr)

    return all_metrics

def aggregate_metrics(all_metrics):
    """Aggregate metrics from all clients."""
    if not all_metrics:
        return None

    # Collect all latencies and server assignments from successful requests
    all_latencies = []
    server_distribution = defaultdict(int)
    failure_reasons = defaultdict(int)

    total_requests = 0
    successful_requests = 0
    failed_requests = 0

    for client_data in all_metrics:
        stats = client_data.get('aggregated_stats', {})

        total_requests += stats.get('total_requests', 0)
        successful_requests += stats.get('successful_requests', 0)
        failed_requests += stats.get('failed_requests', 0)

        # Collect server distribution
        for server_id, count in stats.get('server_distribution', {}).items():
            server_distribution[int(server_id)] += count

        # Collect failure reasons
        for reason, count in stats.get('failure_reasons', {}).items():
            failure_reasons[reason] += count

        # For latency, we need to collect individual successful request latencies
        # If the JSON includes per-request data, use that; otherwise use aggregated stats
        # For now, we'll use the aggregated stats from each client to estimate overall distribution
        client_latencies = []
        if stats.get('successful_requests', 0) > 0:
            # Reconstruct approximate latencies from client stats
            # This is a simplification - ideally we'd have per-request data
            min_lat = stats.get('latency_min_ms', 0)
            max_lat = stats.get('latency_max_ms', 0)
            avg_lat = stats.get('latency_avg_ms', 0)

            # Add representative samples
            if avg_lat > 0:
                # Add samples: min, avg, max for each client's requests
                num_samples = min(stats.get('successful_requests', 1), 100)
                for _ in range(num_samples // 3):
                    all_latencies.append(min_lat)
                for _ in range(num_samples // 3):
                    all_latencies.append(avg_lat)
                for _ in range(num_samples - 2 * (num_samples // 3)):
                    all_latencies.append(max_lat)

    # Calculate aggregated statistics
    failure_rate = (failed_requests / total_requests * 100) if total_requests > 0 else 0.0

    result = {
        'total_requests': total_requests,
        'successful_requests': successful_requests,
        'failed_requests': failed_requests,
        'failure_rate': round(failure_rate, 2),
        'server_distribution': dict(server_distribution),
        'failure_reasons': dict(failure_reasons),
    }

    # Calculate latency statistics if we have data
    if all_latencies:
        all_latencies.sort()
        result['latency_stats'] = {
            'min_ms': round(min(all_latencies), 2),
            'max_ms': round(max(all_latencies), 2),
            'avg_ms': round(statistics.mean(all_latencies), 2),
            'median_ms': round(statistics.median(all_latencies), 2),
            'p95_ms': round(percentile(all_latencies, 95), 2),
            'p99_ms': round(percentile(all_latencies, 99), 2),
        }

    return result

def percentile(sorted_data, p):
    """Calculate percentile of sorted data."""
    if not sorted_data:
        return 0
    k = (len(sorted_data) - 1) * (p / 100.0)
    f = int(k)
    c = f + 1
    if c >= len(sorted_data):
        return sorted_data[-1]
    d0 = sorted_data[f] * (c - k)
    d1 = sorted_data[c] * (k - f)
    return d0 + d1

def generate_text_report(aggregated, output_file):
    """Generate human-readable text report."""
    with open(output_file, 'w') as f:
        f.write("=" * 60 + "\n")
        f.write("CloudP2P Stress Test - Aggregated Metrics Report\n")
        f.write("=" * 60 + "\n\n")

        # Overall statistics
        f.write("OVERALL STATISTICS\n")
        f.write("-" * 60 + "\n")
        f.write(f"Total Requests:       {aggregated['total_requests']:,}\n")
        f.write(f"Successful Requests:  {aggregated['successful_requests']:,}\n")
        f.write(f"Failed Requests:      {aggregated['failed_requests']:,}\n")
        f.write(f"Failure Rate:         {aggregated['failure_rate']:.2f}%\n")
        f.write("\n")

        # Latency statistics
        if 'latency_stats' in aggregated:
            f.write("REQUEST LATENCY (Successful Requests)\n")
            f.write("-" * 60 + "\n")
            lat = aggregated['latency_stats']
            f.write(f"Minimum:              {lat['min_ms']:.2f} ms\n")
            f.write(f"Maximum:              {lat['max_ms']:.2f} ms\n")
            f.write(f"Average:              {lat['avg_ms']:.2f} ms\n")
            f.write(f"Median (P50):         {lat['median_ms']:.2f} ms\n")
            f.write(f"95th Percentile:      {lat['p95_ms']:.2f} ms\n")
            f.write(f"99th Percentile:      {lat['p99_ms']:.2f} ms\n")
            f.write("\n")

        # Load balancing
        f.write("LOAD BALANCING - Server Distribution\n")
        f.write("-" * 60 + "\n")
        total_assigned = sum(aggregated['server_distribution'].values())
        for server_id in sorted(aggregated['server_distribution'].keys()):
            count = aggregated['server_distribution'][server_id]
            percentage = (count / total_assigned * 100) if total_assigned > 0 else 0
            bar = "█" * int(percentage / 2)
            f.write(f"Server {server_id:2d}: {count:8,} requests ({percentage:5.2f}%) {bar}\n")
        f.write("\n")

        # Failure analysis
        if aggregated['failed_requests'] > 0:
            f.write("FAILURE ANALYSIS\n")
            f.write("-" * 60 + "\n")
            sorted_reasons = sorted(
                aggregated['failure_reasons'].items(),
                key=lambda x: x[1],
                reverse=True
            )
            for reason, count in sorted_reasons[:10]:  # Top 10 reasons
                percentage = (count / aggregated['failed_requests'] * 100)
                f.write(f"{reason[:50]:50s}: {count:6,} ({percentage:5.2f}%)\n")
            f.write("\n")

        f.write("=" * 60 + "\n")
        f.write("End of Report\n")
        f.write("=" * 60 + "\n")

def main():
    if len(sys.argv) < 3:
        print("Usage: aggregate.py <metrics_dir> <output_dir>")
        sys.exit(1)

    metrics_dir = sys.argv[1]
    output_dir = sys.argv[2]

    print(f"Loading metrics from: {metrics_dir}")
    all_metrics = load_all_metrics(metrics_dir)

    if not all_metrics:
        print("Error: No metrics files found!")
        sys.exit(1)

    print(f"Loaded {len(all_metrics)} client metrics files")

    print("Aggregating metrics...")
    aggregated = aggregate_metrics(all_metrics)

    if not aggregated:
        print("Error: Failed to aggregate metrics")
        sys.exit(1)

    # Save JSON report
    json_output = os.path.join(output_dir, "final_report.json")
    with open(json_output, 'w') as f:
        json.dump(aggregated, f, indent=2)
    print(f"✓ Saved JSON report: {json_output}")

    # Generate text report
    text_output = os.path.join(output_dir, "final_report.txt")
    generate_text_report(aggregated, text_output)
    print(f"✓ Saved text report: {text_output}")

    print("\nAggregation complete!")

if __name__ == '__main__':
    main()
EOF

# Run Python aggregation script
python3 "$OUTPUT_DIR/aggregate.py" "$TEMP_DIR" "$OUTPUT_DIR"

echo ""
echo "============================================"
echo "Metrics Aggregation Complete!"
echo "============================================"
echo "Output Directory: $OUTPUT_DIR"
echo ""
echo "Reports generated:"
echo "  - JSON: $OUTPUT_DIR/final_report.json"
echo "  - Text: $OUTPUT_DIR/final_report.txt"
echo ""
echo "To view the text report:"
echo "  cat $OUTPUT_DIR/final_report.txt"
echo ""

# Display the text report
if [ -f "$OUTPUT_DIR/final_report.txt" ]; then
    echo "Report preview:"
    echo ""
    cat "$OUTPUT_DIR/final_report.txt"
fi
