import argparse
import os
import json
import statistics
import matplotlib.pyplot as plt


def load_all_metrics(metrics_dir):
    data = []
    for filename in os.listdir(metrics_dir):
        if filename.endswith(".json"):
            with open(os.path.join(metrics_dir, filename), "r") as f:
                data.append(json.load(f))
    return data


def aggregate_metrics(metrics):
    all_latencies = []
    total_failures = 0
    total_requests = 0
    total_success = 0

    for m in metrics:
        all_latencies.append(m["aggregated_stats"]["latency_avg_ms"])
        total_failures += m["aggregated_stats"]["failed_requests"]
        total_requests += m["aggregated_stats"]["total_requests"]
        total_success += m["aggregated_stats"]["successful_requests"]

    summary = {
        "total_clients": len(metrics),
        "avg_latency": statistics.mean(all_latencies) if all_latencies else 0,
        "p95_latency": (
            sorted(all_latencies)[int(0.95 * len(all_latencies))]
            if all_latencies
            else 0
        ),
        "failure_rate": total_failures / total_requests if total_requests else 0,
        "success_rate": total_success / total_requests if total_requests else 0,
    }
    return summary, all_latencies


def generate_plots(all_latencies, output_plot):
    plt.hist(all_latencies, bins=20)
    plt.title("Latency Distribution")
    plt.xlabel("Latency (ms)")
    plt.ylabel("Frequency")
    plt.savefig(output_plot)
    plt.close()


def save_report(summary, output_json):
    with open(output_json, "w") as f:
        json.dump(summary, f, indent=4)


def main():
    parser = argparse.ArgumentParser(
        description="Aggregate client metrics into a single report"
    )
    parser.add_argument("--metrics-dir", required=True)
    parser.add_argument("--output-json", required=True)
    parser.add_argument("--output-plot", required=True)
    args = parser.parse_args()

    metrics = load_all_metrics(args.metrics_dir)
    summary, all_latencies = aggregate_metrics(metrics)
    generate_plots(all_latencies, args.output_plot)
    save_report(summary, args.output_json)
    print(f"✅ Report saved to {args.output_json}")
    print(f"✅ Plot saved to {args.output_plot}")


if __name__ == "__main__":
    main()
