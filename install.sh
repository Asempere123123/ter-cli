#!/bin/bash

URL="https://github.com/Asempere123123/ter-cli/releases/download/v0.1.5/ter-cli"
FILENAME="ter"
TARGET_DIR="/usr/local/bin"

if command -v curl >/dev/null 2>&1; then
    echo "Downloading with curl..."
    curl -fsSL "$URL" -o "/tmp/$FILENAME"
elif command -v wget >/dev/null 2>&1; then
    echo "Downloading with wget..."
    wget -qO "/tmp/$FILENAME" "$URL"
else
    echo "Error: Neither curl nor wget found. Please install one."
    exit 1
fi

echo "Installing $FILENAME to $TARGET_DIR..."
echo "Elevated privileges may be required..."

sudo mkdir -p "$TARGET_DIR"

sudo mv "/tmp/$FILENAME" "$TARGET_DIR/$FILENAME"
sudo chmod +x "$TARGET_DIR/$FILENAME"

if [ -f "$TARGET_DIR/$FILENAME" ]; then
    echo "---------------------------------------"
    echo "Success! $FILENAME is now installed."
    echo "Try running it by typing: $FILENAME"
else
    echo "Installation failed."
    exit 1
fi
