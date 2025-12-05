#!/bin/bash
# Setup test images for all clients

echo "=========================================="
echo "  Setting up test images"
echo "=========================================="

# Check if cover image exists
if [ ! -f "test_images/cover_image.jpg" ]; then
    echo "❌ Error: test_images/cover_image.jpg not found"
    echo "Please add a cover_image.jpg to the test_images directory first"
    exit 1
fi

# Create directories
mkdir -p test_images/client1
mkdir -p test_images/client2
mkdir -p test_images/client3
mkdir -p test_images/encrypted
mkdir -p test_images/decrypted

# Copy cover image as test images for each client
echo "Creating test images for Client 1..."
cp test_images/cover_image.jpg test_images/client1/image1.jpg
cp test_images/cover_image.jpg test_images/client1/image2.jpg
cp test_images/cover_image.jpg test_images/client1/image3.jpg
echo "✅ Client 1: 3 images created"

echo "Creating test images for Client 2..."
cp test_images/cover_image.jpg test_images/client2/image1.jpg
cp test_images/cover_image.jpg test_images/client2/image2.jpg
cp test_images/cover_image.jpg test_images/client2/image3.jpg
echo "✅ Client 2: 3 images created"

echo "Creating test images for Client 3..."
cp test_images/cover_image.jpg test_images/client3/image1.jpg
cp test_images/cover_image.jpg test_images/client3/image2.jpg
cp test_images/cover_image.jpg test_images/client3/image3.jpg
echo "✅ Client 3: 3 images created"

echo ""
echo "=========================================="
echo "  ✅ Setup Complete!"
echo "=========================================="
echo ""
echo "Image directories:"
ls -lh test_images/client1/
echo ""
ls -lh test_images/client2/
echo ""
ls -lh test_images/client3/
echo ""
echo "Note: All clients currently have identical images (copies of cover_image.jpg)"
echo "You can replace them with your own images if desired."
echo ""
