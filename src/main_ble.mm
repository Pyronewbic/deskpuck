#import "JMController.h"

#include <cstdio>
#include <cstring>

static int usage(const char* program) {
    fprintf(stderr, "Usage: %s [--config PATH]\n", program);
    fprintf(stderr, "  --config PATH  settings file (default: %s)\n", [JMConfig defaultFileURL].path.UTF8String);
    return 1;
}

int main(int argc, const char* argv[]) {
    @autoreleasepool {
        NSURL* configURL = [JMConfig defaultFileURL];
        for (int i = 1; i < argc; ++i) {
            if (strcmp(argv[i], "--config") == 0 && i + 1 < argc) {
                configURL = [NSURL fileURLWithPath:@(argv[++i])];
            } else if (strcmp(argv[i], "--mouse") != 0) {
                // --mouse is accepted for compatibility; mouse output is the only mode.
                return usage(argv[0]);
            }
        }

        NSArray<NSString*>* warnings = nil;
        JMConfig* config = [JMConfig configWithContentsOfURL:configURL warnings:&warnings];
        for (NSString* warning in warnings) {
            fprintf(stderr, "config: %s\n", warning.UTF8String);
        }

        JMController* controller = [[JMController alloc] initWithConfig:config];
        [controller start];
        CFRunLoopRun();
    }
    return 0;
}
