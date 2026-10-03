#!/bin/bash
# Build and run every tests/*_test.cpp against the platform-independent core,
# with AddressSanitizer and UBSan so out-of-bounds reads fail the run.
set -uo pipefail

cd "$(dirname "$0")/.."
mkdir -p build/tests

CORE_SOURCES="src/Joycon2InputMapping.cpp src/Joycon2Packet.cpp"
CXXFLAGS="-std=c++17 -Wall -Wextra -Werror -g -fsanitize=address,undefined -fno-sanitize-recover=all"

ran=0
failed=0
for test_source in tests/*_test.cpp; do
    name=$(basename "$test_source" .cpp)
    binary="build/tests/$name"
    # shellcheck disable=SC2086
    if ! clang++ $CXXFLAGS -framework CoreGraphics -Iinclude $CORE_SOURCES "$test_source" -o "$binary"; then
        echo "FAIL $name: did not compile"
        failed=$((failed + 1))
        continue
    fi
    ran=$((ran + 1))
    if ! "$binary"; then
        failed=$((failed + 1))
    fi
done

if [ "$ran" -eq 0 ]; then
    echo "FAIL: no test binaries ran"
    exit 1
fi
if [ "$failed" -ne 0 ]; then
    echo "FAILED: $failed of $((ran + failed)) suites"
    exit 1
fi
echo "OK: $ran suites passed"
