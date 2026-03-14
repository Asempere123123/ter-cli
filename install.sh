#!/bin/bash

URL="https://github.com/Asempere123123/ter-cli/releases/download/v0.0.1/ter-cli"
FILENAME="ter-cli"
TARGET_DIR="/usr/local/bin"

if [ "$EUID" -ne 0 ]; then
    echo "Elevated privileges required. Asking for sudo..."
    exec sudo bash "$0" "$@"
    exit $?
fi

if command -v curl >/dev/null 2>&1; then
    echo "Downloading with curl..."
    curl -L "$URL" -o "/tmp/$FILENAME"
elif command -v wget >/dev/null 2>&1; then
    echo "Downloading with wget..."
    wget -O "/tmp/$FILENAME" "$URL"
else
    echo "Error: Neither curl nor wget found. Please install one."
    exit 1
fi

mv "/tmp/$FILENAME" "$TARGET_DIR/$FILENAME"
chmod +x "$TARGET_DIR/$FILENAME"

echo "Success!"
