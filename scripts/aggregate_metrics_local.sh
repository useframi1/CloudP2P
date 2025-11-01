#!/bin/bash

set -e

# Default configuration
METRICS_DIR="./metrics"
AGGREGATOR_SCRIPT="./scripts/aggregate_metrics_local.py"
OUTPUT_DIR="./reports"
REPORT_JSON="$OUTPUT_DIR/final_report.json"
REPORT_TXT="$OUTPUT_DIR/final_report.txt"
LATENCY_PLOT="$OUTPUT_DIR/final_report.png"
LOG_FILE="$OUTPUT_DIR/aggregate_metrics.log"

echo "============================================"
echo "CloudP2P Metrics Aggregation (Local)"
echo "============================================"
echo "Metrics Dir:   $METRICS_DIR"
echo "Output Dir:    $OUTPUT_DIR"
echo "============================================"
echo ""

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

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Run the aggregator
echo "📊 Running aggregation..."
echo ""
python3 "$AGGREGATOR_SCRIPT" \
  --metrics-dir "$METRICS_DIR" \
  --output-json "$REPORT_JSON" \
  --output-plot "$LATENCY_PLOT" \
  2>&1 | tee "$LOG_FILE"

# Check for success
if [ $? -eq 0 ]; then
  echo ""
  echo "============================================"
  echo "Metrics Aggregation Complete!"
  echo "============================================"
  echo "Output Directory: $OUTPUT_DIR"
  echo ""
  echo "Reports generated:"
  echo "  - JSON: $REPORT_JSON"
  echo "  - Text: $REPORT_TXT"
  echo "  - Plot: $LATENCY_PLOT"
  echo ""
  echo "To view the text report:"
  echo "  cat $REPORT_TXT"
  echo ""
  echo "To view metrics JSON:"
  echo "  cat $REPORT_JSON | jq"
  echo ""
else
  echo ""
  echo "❌ Aggregation failed! Check $LOG_FILE for details."
  exit 1
fi
