#!/bin/bash

# Script to convert Markdown to PDF using Chrome

REPORT_FILE="TECHNICAL_REPORT.md"
OUTPUT_PDF="TECHNICAL_REPORT.pdf"

echo "Converting $REPORT_FILE to PDF..."

# Create temporary HTML file with styling
cat > temp_report.html <<'EOF'
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>CloudP2P Technical Report</title>
    <style>
        body {
            font-family: 'Georgia', 'Times New Roman', serif;
            line-height: 1.6;
            max-width: 900px;
            margin: 40px auto;
            padding: 20px;
            color: #333;
        }
        h1 {
            color: #2c3e50;
            border-bottom: 3px solid #3498db;
            padding-bottom: 10px;
            font-size: 2.5em;
            margin-top: 1em;
        }
        h2 {
            color: #2980b9;
            border-bottom: 2px solid #bdc3c7;
            padding-bottom: 8px;
            margin-top: 1.5em;
            font-size: 2em;
        }
        h3 {
            color: #34495e;
            margin-top: 1.2em;
            font-size: 1.5em;
        }
        h4 {
            color: #555;
            margin-top: 1em;
        }
        code {
            background-color: #f4f4f4;
            padding: 2px 6px;
            border-radius: 3px;
            font-family: 'Monaco', 'Courier New', monospace;
            font-size: 0.9em;
        }
        pre {
            background-color: #f8f8f8;
            border: 1px solid #ddd;
            border-radius: 5px;
            padding: 15px;
            overflow-x: auto;
            font-size: 0.85em;
        }
        pre code {
            background-color: transparent;
            padding: 0;
        }
        table {
            border-collapse: collapse;
            width: 100%;
            margin: 20px 0;
            font-size: 0.9em;
        }
        th, td {
            border: 1px solid #ddd;
            padding: 12px;
            text-align: left;
        }
        th {
            background-color: #3498db;
            color: white;
            font-weight: bold;
        }
        tr:nth-child(even) {
            background-color: #f9f9f9;
        }
        blockquote {
            border-left: 4px solid #3498db;
            padding-left: 20px;
            margin-left: 0;
            color: #555;
            font-style: italic;
        }
        a {
            color: #3498db;
            text-decoration: none;
        }
        a:hover {
            text-decoration: underline;
        }
        .page-break {
            page-break-after: always;
        }
        @media print {
            body {
                margin: 0;
                padding: 20px;
            }
            h1, h2 {
                page-break-after: avoid;
            }
        }
    </style>
</head>
<body>
EOF

# Convert markdown to HTML using Python (markdown module)
python3 -c "
import markdown
import sys

with open('$REPORT_FILE', 'r', encoding='utf-8') as f:
    text = f.read()

html = markdown.markdown(text, extensions=['tables', 'fenced_code', 'codehilite'])
print(html)
" >> temp_report.html

cat >> temp_report.html <<'EOF'
</body>
</html>
EOF

# Convert HTML to PDF using Chrome
google-chrome --headless --disable-gpu --print-to-pdf="$OUTPUT_PDF" \
  --no-margins \
  --print-to-pdf-no-header \
  temp_report.html 2>/dev/null

# Clean up
rm temp_report.html

if [ -f "$OUTPUT_PDF" ]; then
    echo "✅ PDF created successfully: $OUTPUT_PDF"
    ls -lh "$OUTPUT_PDF"
else
    echo "❌ PDF creation failed"
    exit 1
fi
