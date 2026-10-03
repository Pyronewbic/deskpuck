#!/bin/bash
# Build and run the unit tests for the platform-independent input mapping logic.
set -euo pipefail

cd "$(dirname "$0")/.."
mkdir -p build

clang++ -std=c++17 -Wall -Wextra -Werror -framework CoreGraphics -Iinclude \
    src/Joycon2InputMapping.cpp tests/input_mapping_test.cpp -o build/input_mapping_test

./build/input_mapping_test
