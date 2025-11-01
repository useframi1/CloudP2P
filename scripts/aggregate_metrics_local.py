#!/usr/bin/env python3

import argparse
import os
import json
import statistics
import matplotlib.pyplot as plt
from pathlib import Path
from collections import defaultdict


def load_all_metrics(metrics_dir):
    """Load all metrics from metrics directory, including machine subdirectories."""
    data = []

    # Check if metrics_dir contains machine subdirectories
    for entry in os.listdir(metrics_dir):
        entry_path = os.path.join(metrics_dir, entry)

        if os.path.isdir(entry_path) and entry.startswith("machine_"):
            # Load all JSON files from machine subdirectory
            print(f"📂 Loading metrics from {entry}/")
            for filename in os.listdir(entry_path):
                if filename.endswith(".json"):
                    filepath = os.path.join(entry_path, filename)
                    try:
                        with open(filepath, "r") as f:
                            data.append(json.load(f))
                    except Exception as e:
                        print(f"Warning: Failed to load {filepath}: {e}")
        elif entry.endswith(".json"):
            # Also support legacy flat structure
            try:
                with open(entry_path, "r") as f:
                    data.append(json.load(f))
            except Exception as e:
                print(f"Warning: Failed to load {entry_path}: {e}")

    print(f"✅ Loaded {len(data)} metric files")
    return data


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
        stats = client_data.get("aggregated_stats", {})

        total_requests += stats.get("total_requests", 0)
        successful_requests += stats.get("successful_requests", 0)
        failed_requests += stats.get("failed_requests", 0)

        # Collect server distribution
        for server_id, count in stats.get("server_distribution", {}).items():
            server_distribution[int(server_id)] += count

        # Collect failure reasons
        for reason, count in stats.get("failure_reasons", {}).items():
            failure_reasons[reason] += count

        # For latency, reconstruct approximate distribution from client stats
        if stats.get("successful_requests", 0) > 0:
            min_lat = stats.get("latency_min_ms", 0)
            max_lat = stats.get("latency_max_ms", 0)
            avg_lat = stats.get("latency_avg_ms", 0)

            # Add representative samples: min, avg, max
            if avg_lat > 0:
                num_samples = min(stats.get("successful_requests", 1), 100)
                for _ in range(num_samples // 3):
                    all_latencies.append(min_lat)
                for _ in range(num_samples // 3):
                    all_latencies.append(avg_lat)
                for _ in range(num_samples - 2 * (num_samples // 3)):
                    all_latencies.append(max_lat)

    # Calculate aggregated statistics
    failure_rate = (failed_requests / total_requests * 100) if total_requests > 0 else 0.0

    result = {
        "total_requests": total_requests,
        "successful_requests": successful_requests,
        "failed_requests": failed_requests,
        "failure_rate": round(failure_rate, 2),
        "server_distribution": dict(server_distribution),
        "failure_reasons": dict(failure_reasons),
    }

    # Calculate latency statistics if we have data
    if all_latencies:
        all_latencies.sort()
        result["latency_stats"] = {
            "min_ms": round(min(all_latencies), 2),
            "max_ms": round(max(all_latencies), 2),
            "avg_ms": round(statistics.mean(all_latencies), 2),
            "median_ms": round(statistics.median(all_latencies), 2),
            "p95_ms": round(percentile(all_latencies, 95), 2),
            "p99_ms": round(percentile(all_latencies, 99), 2),
        }

    return result, all_latencies


def generate_text_report(aggregated, output_file):
    """Generate human-readable text report."""
    with open(output_file, "w") as f:
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
        if "latency_stats" in aggregated:
            f.write("REQUEST LATENCY (Successful Requests)\n")
            f.write("-" * 60 + "\n")
            lat = aggregated["latency_stats"]
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
        total_assigned = sum(aggregated["server_distribution"].values())
        for server_id in sorted(aggregated["server_distribution"].keys()):
            count = aggregated["server_distribution"][server_id]
            percentage = (count / total_assigned * 100) if total_assigned > 0 else 0
            bar = "█" * int(percentage / 2)
            f.write(
                f"Server {server_id:2d}: {count:8,} requests ({percentage:5.2f}%) {bar}\n"
            )
        f.write("\n")

        # Failure analysis
        if aggregated["failed_requests"] > 0:
            f.write("FAILURE ANALYSIS\n")
            f.write("-" * 60 + "\n")
            sorted_reasons = sorted(
                aggregated["failure_reasons"].items(), key=lambda x: x[1], reverse=True
            )
            for reason, count in sorted_reasons[:10]:  # Top 10 reasons
                percentage = count / aggregated["failed_requests"] * 100
                f.write(f"{reason[:50]:50s}: {count:6,} ({percentage:5.2f}%)\n")
            f.write("\n")

        f.write("=" * 60 + "\n")
        f.write("End of Report\n")
        f.write("=" * 60 + "\n")


def generate_plots(all_latencies, output_plot):
    """Generate latency distribution plot."""
    if not all_latencies:
        print("⚠️  No latency data available for plotting")
        return

    plt.figure(figsize=(10, 6))
    plt.hist(all_latencies, bins=50, edgecolor="black", alpha=0.7)
    plt.title("Request Latency Distribution", fontsize=14, fontweight="bold")
    plt.xlabel("Latency (ms)", fontsize=12)
    plt.ylabel("Frequency", fontsize=12)
    plt.grid(axis="y", alpha=0.3)

    # Add statistics as text
    avg_lat = statistics.mean(all_latencies)
    median_lat = statistics.median(all_latencies)
    p95_lat = percentile(sorted(all_latencies), 95)

    stats_text = f"Avg: {avg_lat:.1f}ms\nMedian: {median_lat:.1f}ms\nP95: {p95_lat:.1f}ms"
    plt.text(
        0.98,
        0.97,
        stats_text,
        transform=plt.gca().transAxes,
        fontsize=10,
        verticalalignment="top",
        horizontalalignment="right",
        bbox=dict(boxstyle="round", facecolor="wheat", alpha=0.5),
    )

    plt.tight_layout()
    plt.savefig(output_plot, dpi=150)
    plt.close()


def main():
    parser = argparse.ArgumentParser(
        description="Aggregate client metrics into a single report"
    )
    parser.add_argument("--metrics-dir", required=True)
    parser.add_argument("--output-json", required=True)
    parser.add_argument("--output-plot", required=True)
    args = parser.parse_args()

    # Load metrics
    print(f"Loading metrics from: {args.metrics_dir}")
    metrics = load_all_metrics(args.metrics_dir)

    if not metrics:
        print("❌ No metrics files found!")
        return 1

    # Aggregate
    print("Aggregating metrics...")
    result = aggregate_metrics(metrics)

    if result is None:
        print("❌ Failed to aggregate metrics")
        return 1

    aggregated, all_latencies = result

    # Generate outputs
    # 1. JSON report
    with open(args.output_json, "w") as f:
        json.dump(aggregated, f, indent=2)
    print(f"✅ JSON report saved: {args.output_json}")

    # 2. Text report
    text_output = args.output_json.replace(".json", ".txt")
    generate_text_report(aggregated, text_output)
    print(f"✅ Text report saved: {text_output}")

    # 3. Plot
    generate_plots(all_latencies, args.output_plot)
    print(f"✅ Plot saved: {args.output_plot}")

    # Display text report
    print("\n" + "=" * 60)
    print("REPORT PREVIEW")
    print("=" * 60 + "\n")
    with open(text_output, "r") as f:
        print(f.read())

    return 0


if __name__ == "__main__":
    exit(main())
