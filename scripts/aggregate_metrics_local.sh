#!/bin/bash

set -e

# Default configuration file
METRICS_DIR="./metrics"
AGGREGATOR_SCRIPT="./scripts/aggregate_metrics_local.py"
REPORT_JSON="./reports/final_report.json"
LATENCY_PLOT="./reports/final_report.png"
LOG_FILE="./reports/aggregate_metrics.log"

echo "----------------------------------------"
echo "🚀 Starting metrics aggregation (local)"
echo "----------------------------------------"

# Check metrics directory
if [ ! -d "$METRICS_DIR" ]; then
  echo "❌ Metrics directory not found: $METRICS_DIR"
  exit 1
fi

# Check Python script
if [ ! -f "$AGGREGATOR_SCRIPT" ]; then
  echo "❌ Aggregator script not found: $AGGREGATOR_SCRIPT"
  exit 1
fi

# Run the aggregator
echo "📊 Running aggregation using $AGGREGATOR_SCRIPT..."
python3 "$AGGREGATOR_SCRIPT" \
  --metrics-dir "$METRICS_DIR" \
  --output-json "$REPORT_JSON" \
  --output-plot "$LATENCY_PLOT" \
  2>&1 | tee "$LOG_FILE"

# Check for success
if [ $? -eq 0 ]; then
  echo "✅ Aggregation complete!"
  echo "📁 Report: $REPORT_JSON"
  echo "📈 Plot: $LATENCY_PLOT"
else
  echo "❌ Aggregation failed! Check $LOG_FILE for details."
  exit 1
fi
