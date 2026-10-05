#!/bin/bash
# Build and run every tests/*_test.{cpp,mm} against the core,
# with AddressSanitizer and UBSan so out-of-bounds reads fail the run.
set -uo pipefail

cd "$(dirname "$0")/.."
mkdir -p build/tests

CORE=Sources/DeskpuckCore
CORE_SOURCES="$CORE/DPConfig.mm $CORE/Joycon2BLEReceiver.mm $CORE/Joycon2Engine.cpp $CORE/Joycon2InputMapping.cpp $CORE/Joycon2Packet.cpp"
CXXFLAGS="-std=c++17 -fobjc-arc -Wall -Wextra -Werror -g -fsanitize=address,undefined -fno-sanitize-recover=all"

total=0
ran=0
failed=0
for test_source in tests/*_test.cpp tests/*_test.mm; do
    [ -e "$test_source" ] || continue
    name=$(basename "${test_source%.*}")
    binary="build/tests/$name"
    total=$((total + 1))
    # shellcheck disable=SC2086
    if ! clang++ $CXXFLAGS -framework CoreBluetooth -framework CoreGraphics -framework Foundation -I"$CORE" -I"$CORE/include" $CORE_SOURCES "$test_source" -o "$binary"; then
        echo "FAIL $name: did not compile"
        failed=$((failed + 1))
        continue
    fi
    ran=$((ran + 1))
    output=$("$binary" 2>&1)
    status=$?
    echo "$output"
    if [ "$status" -ne 0 ]; then
        failed=$((failed + 1))
    elif ! echo "$output" | grep -qE '^[a-z_]+: [1-9][0-9]* checks, 0 failures$'; then
        # An exit 0 without a summary means the suite never reached its checks.
        echo "FAIL $name: exited 0 without a passing check summary"
        failed=$((failed + 1))
    fi
done

if [ "$ran" -eq 0 ]; then
    echo "FAIL: no test binaries ran"
    exit 1
fi
if [ "$failed" -ne 0 ]; then
    echo "FAILED: $failed of $total suites"
    exit 1
fi
echo "OK: $ran suites passed"
