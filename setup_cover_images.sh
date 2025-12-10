#!/bin/bash

# Setup script for client images directory structure
# This creates the client_images folder structure

echo "Setting up client_images directory structure..."

# Create cover_images directory
mkdir -p client_images/cover_images

# Create directory structure for each client
for client in client1 client2 client3; do
    mkdir -p client_images/$client/original_images
    mkdir -p client_images/$client/encrypted_images
    mkdir -p client_images/$client/received_images
done

echo "Created directory structure:"
echo "  - client_images/cover_images/ (place your carrier images here)"
echo "  - client_images/client1/original_images/"
echo "  - client_images/client1/encrypted_images/"
echo "  - client_images/client1/received_images/"
echo "  - client_images/client2/original_images/"
echo "  - client_images/client2/encrypted_images/"
echo "  - client_images/client2/received_images/"
echo "  - client_images/client3/original_images/"
echo "  - client_images/client3/encrypted_images/"
echo "  - client_images/client3/received_images/"
echo ""
echo "Next steps:"
echo "1. Add carrier images (e.g., JPG/PNG) to client_images/cover_images/ folder"
echo "2. Add client images to client_images/{client_id}/original_images/ folders"
echo "3. Recommended: Use large carrier images (800x600 or higher) for better capacity"
