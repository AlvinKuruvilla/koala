#!/bin/bash
set -e

cd "$(dirname "$0")"

echo "Building KoalaBrowser..."
swift build

# Create app bundle structure
APP_NAME="KoalaBrowser"
APP_DIR=".build/debug/${APP_NAME}.app"
CONTENTS_DIR="${APP_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"

echo "Creating app bundle..."
rm -rf "${APP_DIR}"
mkdir -p "${MACOS_DIR}"

# Copy executable
cp ".build/debug/${APP_NAME}" "${MACOS_DIR}/"

# Copy Info.plist
cp "Sources/KoalaBrowser/Info.plist" "${CONTENTS_DIR}/"

echo "Launching ${APP_NAME}..."
open "${APP_DIR}"
