#!/bin/bash
# Build the joymouse command-line tool into build/Joycon2VirtualHID.
# Usage: ./build.sh [FULL] [debug|release]   (FULL is the only mode; kept for compatibility)
set -euo pipefail

cd "$(dirname "$0")"
mkdir -p build

BUILD_MODE=${1:-FULL}
BUILD_TYPE=${2:-debug}

if [ "$BUILD_MODE" != "FULL" ]; then
    echo "Invalid BUILD_MODE: $BUILD_MODE. Use FULL."
    exit 1
fi

DEBUG_FLAG=""
if [ "$BUILD_TYPE" = "debug" ]; then
    DEBUG_FLAG="-DDEBUG"
fi

echo "Building Joycon2VirtualHID ($BUILD_TYPE)..."
clang++ -x objective-c++ -std=c++17 -fobjc-arc $DEBUG_FLAG \
    -framework Foundation -framework CoreBluetooth -framework ApplicationServices \
    -Iinclude \
    src/JMController.mm src/Joycon2BLEReceiver.mm src/Joycon2Engine.cpp \
    src/Joycon2InputMapping.cpp src/Joycon2Packet.cpp src/main_ble.mm \
    -o build/Joycon2VirtualHID
echo "Build successful! Executable: build/Joycon2VirtualHID ($BUILD_TYPE)"
