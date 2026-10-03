#import "JMController.h"

#include <cstdio>
#include <cstring>

int main(int argc, const char* argv[]) {
    for (int i = 1; i < argc; ++i) {
        // --mouse is accepted for compatibility; mouse output is the only mode.
        if (strcmp(argv[i], "--mouse") != 0) {
            fprintf(stderr, "Usage: %s [--mouse]\n", argv[0]);
            return 1;
        }
    }

    @autoreleasepool {
        JMController* controller = [[JMController alloc] init];
        [controller start];
        CFRunLoopRun();
    }
    return 0;
}
