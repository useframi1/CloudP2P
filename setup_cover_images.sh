#!/bin/bash

# Setup script for cover images
# This creates the cover_images folder and provides sample carrier images

echo "Setting up cover_images folder..."
mkdir -p cover_images

# Create encrypted_images directories for clients
mkdir -p encrypted_images/client1
mkdir -p encrypted_images/client2
mkdir -p encrypted_images/client3

echo "Created directory structure:"
echo "  - cover_images/ (place your carrier images here)"
echo "  - encrypted_images/client1/"
echo "  - encrypted_images/client2/"
echo "  - encrypted_images/client3/"
echo ""
echo "Next steps:"
echo "1. Add carrier images (e.g., JPG/PNG) to cover_images/ folder"
echo "2. These will be used to hide secret images via steganography"
echo "3. Recommended: Use large images (800x600 or higher) for better capacity"
