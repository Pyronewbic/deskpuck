#pragma once

#include <cstdio>

static int checks = 0;
static int failures = 0;

#define CHECK(cond) do { \
    checks++; \
    if (!(cond)) { failures++; std::printf("FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond); } \
} while (0)

// Exit status for main: fails if any check failed or if no checks ran at all.
static int checkSummary(const char* suite) {
    std::printf("%s: %d checks, %d failures\n", suite, checks, failures);
    if (checks == 0) {
        std::printf("FAIL %s: no checks ran\n", suite);
        return 1;
    }
    return failures == 0 ? 0 : 1;
}
