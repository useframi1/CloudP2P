#!/bin/bash

# Simple script to convert Markdown to PDF using Chrome
# No external dependencies needed (uses Python's markdown which is already installed)

REPORT_MD="TECHNICAL_REPORT.md"
OUTPUT_PDF="TECHNICAL_REPORT.pdf"
TEMP_HTML="temp_report.html"

echo "🔄 Converting $REPORT_MD to PDF..."

# Create HTML with styling and markdown content
python3 << 'PYTHON_SCRIPT'
import markdown
import sys

# Read markdown file
with open('TECHNICAL_REPORT.md', 'r', encoding='utf-8') as f:
    md_content = f.read()

# Convert to HTML
html_body = markdown.markdown(
    md_content,
    extensions=['tables', 'fenced_code', 'codehilite', 'toc']
)

# Create full HTML document with nice styling
html_template = """<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>CloudP2P Technical Report</title>
    <style>
        @page {
            margin: 2cm;
        }
        body {
            font-family: 'Georgia', 'Times New Roman', serif;
            line-height: 1.6;
            color: #333;
            max-width: 1000px;
            margin: 0 auto;
        }
        h1 {
            color: #2c3e50;
            border-bottom: 3px solid #3498db;
            padding-bottom: 10px;
            margin-top: 40px;
            page-break-before: always;
        }
        h1:first-of-type {
            page-break-before: avoid;
        }
        h2 {
            color: #2980b9;
            border-bottom: 2px solid #bdc3c7;
            padding-bottom: 8px;
            margin-top: 30px;
            page-break-after: avoid;
        }
        h3 {
            color: #34495e;
            margin-top: 25px;
            page-break-after: avoid;
        }
        h4 {
            color: #555;
            margin-top: 20px;
        }
        code {
            background-color: #f4f4f4;
            padding: 2px 6px;
            border-radius: 3px;
            font-family: 'Monaco', 'Courier New', monospace;
            font-size: 0.85em;
            color: #c7254e;
        }
        pre {
            background-color: #f8f8f8;
            border: 1px solid #ddd;
            border-left: 4px solid #3498db;
            border-radius: 5px;
            padding: 15px;
            overflow-x: auto;
            margin: 20px 0;
            page-break-inside: avoid;
        }
        pre code {
            background-color: transparent;
            padding: 0;
            color: #333;
        }
        table {
            border-collapse: collapse;
            width: 100%;
            margin: 20px 0;
            font-size: 0.9em;
            page-break-inside: avoid;
        }
        th, td {
            border: 1px solid #ddd;
            padding: 10px;
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
            margin: 20px 0;
            color: #555;
            font-style: italic;
            background-color: #f0f8ff;
            padding: 15px;
            border-radius: 5px;
        }
        a {
            color: #3498db;
            text-decoration: none;
        }
        strong {
            color: #2c3e50;
        }
        ul, ol {
            margin: 15px 0;
        }
        li {
            margin: 8px 0;
        }
        hr {
            border: none;
            border-top: 2px solid #bdc3c7;
            margin: 40px 0;
        }
    </style>
</head>
<body>
""" + html_body + """
</body>
</html>"""

# Write to file
with open('temp_report.html', 'w', encoding='utf-8') as f:
    f.write(html_template)

print("✅ HTML generated")
PYTHON_SCRIPT

if [ ! -f "$TEMP_HTML" ]; then
    echo "❌ Failed to generate HTML"
    exit 1
fi

# Convert HTML to PDF using Chrome
echo "📄 Generating PDF..."
google-chrome --headless --disable-gpu \
  --print-to-pdf="$OUTPUT_PDF" \
  --print-to-pdf-no-header \
  --no-pdf-header-footer \
  "$TEMP_HTML" 2>/dev/null

# Wait a moment for file to be written
sleep 1

# Clean up
rm -f "$TEMP_HTML"

# Check result
if [ -f "$OUTPUT_PDF" ]; then
    echo ""
    echo "✅ SUCCESS! PDF created:"
    ls -lh "$OUTPUT_PDF"
    echo ""
    echo "📖 Open with: xdg-open $OUTPUT_PDF"
else
    echo "❌ PDF creation failed"
    exit 1
fi
