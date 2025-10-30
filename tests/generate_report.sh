#!/bin/bash

# Generate a comprehensive test report from logs

set -e

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOGS_DIR="$PROJECT_DIR/test_results/logs"
REPORT_FILE="$PROJECT_DIR/test_results/test_report.md"

if [ ! -d "$LOGS_DIR" ]; then
    echo "Error: No test results found. Run tests first."
    exit 1
fi

echo "Generating test report..."

# Start report
cat > "$REPORT_FILE" << 'EOF'
# CloudP2P Test Report

**Generated**: $(date)

## Executive Summary

EOF

# Count tasks completed per client
TOTAL_TASKS=0
SUCCESSFUL_TASKS=0
FAILED_TASKS=0

for log in "$LOGS_DIR"/*Client*.log; do
    if [ -f "$log" ]; then
        CLIENT_NAME=$(basename "$log" .log)
        COMPLETED=$(grep -c "completed successfully" "$log" 2>/dev/null || true)
        FAILED=$(grep -c "FAILED after" "$log" 2>/dev/null || true)

        # Ensure variables are integers (default to 0 if empty)
        : ${COMPLETED:=0}
        : ${FAILED:=0}

        TOTAL_TASKS=$((TOTAL_TASKS + COMPLETED + FAILED))
        SUCCESSFUL_TASKS=$((SUCCESSFUL_TASKS + COMPLETED))
        FAILED_TASKS=$((FAILED_TASKS + FAILED))

        echo "- **$CLIENT_NAME**: $COMPLETED successful, $FAILED failed" >> "$REPORT_FILE"
    fi
done

cat >> "$REPORT_FILE" << EOF

**Total Tasks**: $TOTAL_TASKS
**Successful**: $SUCCESSFUL_TASKS
**Failed**: $FAILED_TASKS
**Success Rate**: $(if [ "$TOTAL_TASKS" -gt 0 ]; then awk "BEGIN {printf \"%.1f\", ($SUCCESSFUL_TASKS/$TOTAL_TASKS)*100}"; else echo "0"; fi)%

---

## Leader Election Analysis

EOF

# Find leader elections
echo "### Elections Detected" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"

for log in "$LOGS_DIR"/server*.log; do
    if [ -f "$log" ]; then
        SERVER_ID=$(basename "$log" .log | sed 's/server//')

        if grep -q "won election" "$log"; then
            PRIORITY=$(grep "won election" "$log" | tail -1 | sed -n 's/.*priority score: \([0-9.]*\).*/\1/p')
            if [ -z "$PRIORITY" ]; then
                PRIORITY="N/A"
            fi
            echo "- **Server $SERVER_ID** won election (priority: $PRIORITY)" >> "$REPORT_FILE"
        fi
    fi
done

cat >> "$REPORT_FILE" << 'EOF'

### Re-elections

EOF

# Find re-elections (leader failures)
REELECTIONS=$(grep -r "LEADER .* appears to have failed" "$LOGS_DIR" 2>/dev/null | wc -l)
echo "- Total re-elections triggered: $REELECTIONS" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"

cat >> "$REPORT_FILE" << 'EOF'

---

## Server Performance

| Server | Tasks Processed | Failures Detected | Status |
|--------|----------------|-------------------|--------|
EOF

# Server stats
for i in 1 2 3; do
    LOG="$LOGS_DIR/server${i}.log"
    if [ -f "$LOG" ]; then
        PROCESSED=$(grep -c "completed encryption" "$LOG" 2>/dev/null || true)
        FAILURES=$(grep -c "appears to have failed" "$LOG" 2>/dev/null || true)
        : ${PROCESSED:=0}
        : ${FAILURES:=0}
        STATUS="✓ Running"

        echo "| Server $i | $PROCESSED | $FAILURES | $STATUS |" >> "$REPORT_FILE"
    else
        echo "| Server $i | N/A | N/A | ✗ No logs |" >> "$REPORT_FILE"
    fi
done

cat >> "$REPORT_FILE" << 'EOF'

---

## Client Behavior

EOF

# Client retry analysis
for log in "$LOGS_DIR"/*Client*.log; do
    if [ -f "$log" ]; then
        CLIENT_NAME=$(basename "$log" .log)
        RETRIES=$(grep -c "Retry attempt" "$log" 2>/dev/null || true)
        TIMEOUTS=$(grep -c "timed out" "$log" 2>/dev/null || true)
        COMPLETED_COUNT=$(grep -c "completed successfully" "$log" 2>/dev/null || true)

        : ${RETRIES:=0}
        : ${TIMEOUTS:=0}
        : ${COMPLETED_COUNT:=0}

        cat >> "$REPORT_FILE" << EOF
### $CLIENT_NAME

- Retry attempts: $RETRIES
- Timeouts: $TIMEOUTS
- Tasks completed: $COMPLETED_COUNT

EOF
    fi
done

cat >> "$REPORT_FILE" << 'EOF'

---

## Fault Tolerance Events

### Server Failures Detected

EOF

# Find all failure detections
grep -r "appears to have failed" "$LOGS_DIR" 2>/dev/null | while read -r line; do
    echo "- $line" | sed 's/.*logs\///' | sed 's/:/ - /' >> "$REPORT_FILE"
done || echo "- No failures detected" >> "$REPORT_FILE"

cat >> "$REPORT_FILE" << 'EOF'

### Orphaned Tasks Cleaned

EOF

# Find orphaned task cleanup
ORPHANED=$(grep -r "found .* orphaned task" "$LOGS_DIR" 2>/dev/null | wc -l)
echo "- Total orphaned task cleanup events: $ORPHANED" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"

cat >> "$REPORT_FILE" << 'EOF'

---

## Error Analysis

### Critical Errors

EOF

# Find critical errors
if grep -r "ERROR\|FAILED\|Failed" "$LOGS_DIR" 2>/dev/null > /tmp/errors.txt; then
    head -20 /tmp/errors.txt | while read -r line; do
        echo "- $(echo "$line" | sed 's/.*logs\///')" >> "$REPORT_FILE"
    done
    rm /tmp/errors.txt
else
    echo "- No critical errors detected ✓" >> "$REPORT_FILE"
fi

cat >> "$REPORT_FILE" << 'EOF'

---

## Output Files

EOF

# List generated files
OUTPUT_DIR="$PROJECT_DIR/user-data/outputs"
if [ -d "$OUTPUT_DIR" ]; then
    FILE_COUNT=$(ls -1 "$OUTPUT_DIR"/encrypted_* 2>/dev/null | wc -l)
    echo "- Total encrypted files generated: $FILE_COUNT" >> "$REPORT_FILE"
    echo "" >> "$REPORT_FILE"

    if [ "$FILE_COUNT" -gt 0 ]; then
        echo "### Files" >> "$REPORT_FILE"
        echo '```' >> "$REPORT_FILE"
        ls -lh "$OUTPUT_DIR"/encrypted_* 2>/dev/null | tail -10 >> "$REPORT_FILE"
        echo '```' >> "$REPORT_FILE"
    fi
else
    echo "- No output directory found" >> "$REPORT_FILE"
fi

cat >> "$REPORT_FILE" << 'EOF'

---

## Timeline Analysis

### Key Events Timeline

EOF

# Create timeline from all logs
echo '```' >> "$REPORT_FILE"
grep -h "won election\|LEADER.*failed\|initiating election\|completed successfully" "$LOGS_DIR"/*.log 2>/dev/null | \
    sed -n 's/.*\(\[[0-9][0-9]:[0-9][0-9]:[0-9][0-9]\].*\)/\1/p' | \
    sort -u | \
    head -20 >> "$REPORT_FILE" || echo "No timeline data available" >> "$REPORT_FILE"
echo '```' >> "$REPORT_FILE"

cat >> "$REPORT_FILE" << 'EOF'

---

## Recommendations

EOF

# Generate recommendations based on findings
if [ "$FAILED_TASKS" -gt 0 ]; then
    echo "- ⚠️ **$FAILED_TASKS tasks failed** - Review client retry logic and timeout settings" >> "$REPORT_FILE"
fi

if [ "$REELECTIONS" -gt 5 ]; then
    echo "- ⚠️ **Frequent re-elections detected** - May indicate unstable system or aggressive timeout settings" >> "$REPORT_FILE"
fi

if [ "$SUCCESSFUL_TASKS" -eq "$TOTAL_TASKS" ] && [ "$TOTAL_TASKS" -gt 0 ]; then
    echo "- ✓ **All tasks completed successfully** - System is functioning optimally" >> "$REPORT_FILE"
fi

cat >> "$REPORT_FILE" << 'EOF'

---

## Logs Location

All detailed logs available at: `test_results/logs/`

To view:
```bash
# Server logs
tail -f test_results/logs/server1.log

# Client logs
tail -f test_results/logs/TestClient1.log

# Search for specific events
grep -r "pattern" test_results/logs/
```

---

*End of Report*
EOF

# Replace $(date) with actual date
sed -i '' "s/\$(date)/$(date)/" "$REPORT_FILE" 2>/dev/null || \
    sed -i "s/\$(date)/$(date)/" "$REPORT_FILE" 2>/dev/null

echo ""
echo "✓ Report generated: $REPORT_FILE"
echo ""
echo "View report:"
echo "  cat $REPORT_FILE"
echo ""

# Also display summary to terminal
echo "=== Quick Summary ==="
echo "Total Tasks:      $TOTAL_TASKS"
echo "Successful:       $SUCCESSFUL_TASKS"
echo "Failed:           $FAILED_TASKS"
echo "Re-elections:     $REELECTIONS"
echo "Output Files:     $(ls -1 "$OUTPUT_DIR"/encrypted_* 2>/dev/null | wc -l)"
echo ""
