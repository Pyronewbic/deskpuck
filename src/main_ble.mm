#import "JMController.h"

#include <cstdio>
#include <cstring>

static int usage(const char* program) {
    fprintf(stderr, "Usage: %s [--config PATH] [--verbose] [--monitor]\n", program);
    fprintf(stderr, "  --config PATH  settings file (default: %s)\n", [JMConfig defaultFileURL].path.UTF8String);
    fprintf(stderr, "  --verbose      print Bluetooth connection detail\n");
    fprintf(stderr, "  --monitor      show a live readout of every Joy-Con report\n");
    return 1;
}

int main(int argc, const char* argv[]) {
    @autoreleasepool {
        NSURL* configURL = [JMConfig defaultFileURL];
        BOOL verbose = NO;
        BOOL monitor = NO;
        for (int i = 1; i < argc; ++i) {
            if (strcmp(argv[i], "--config") == 0 && i + 1 < argc) {
                configURL = [NSURL fileURLWithPath:@(argv[++i])];
            } else if (strcmp(argv[i], "--verbose") == 0) {
                verbose = YES;
            } else if (strcmp(argv[i], "--monitor") == 0) {
                monitor = YES;
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

        [JMController setVerboseLogging:verbose monitorReports:monitor];
        JMController* controller = [[JMController alloc] initWithConfig:config];
        [controller start];
        CFRunLoopRun();
    }
    return 0;
}
