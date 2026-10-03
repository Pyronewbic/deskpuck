#import "DPController.h"

#include <cstdio>
#include <cstring>

static int usage(const char* program, int status) {
    FILE* out = status == 0 ? stdout : stderr;
    fprintf(out, "Usage: %s [--config PATH] [--verbose] [--monitor]\n", program);
    fprintf(out, "  --config PATH  settings file (default: %s)\n", [DPConfig defaultFileURL].path.UTF8String);
    fprintf(out, "  --verbose      print Bluetooth connection detail\n");
    fprintf(out, "  --monitor      show a live readout of every Joy-Con report\n");
    return status;
}

int main(int argc, const char* argv[]) {
    @autoreleasepool {
        NSURL* configURL = [DPConfig defaultFileURL];
        BOOL verbose = NO;
        BOOL monitor = NO;
        for (int i = 1; i < argc; ++i) {
            if (strcmp(argv[i], "--config") == 0 && i + 1 < argc) {
                configURL = [NSURL fileURLWithPath:@(argv[++i])];
            } else if (strcmp(argv[i], "--verbose") == 0) {
                verbose = YES;
            } else if (strcmp(argv[i], "--monitor") == 0) {
                monitor = YES;
            } else if (strcmp(argv[i], "--help") == 0 || strcmp(argv[i], "-h") == 0) {
                return usage(argv[0], 0);
            } else if (strcmp(argv[i], "--mouse") != 0) {
                // --mouse is accepted for compatibility; mouse output is the only mode.
                return usage(argv[0], 1);
            }
        }

        NSArray<NSString*>* warnings = nil;
        DPConfig* config = [DPConfig configWithContentsOfURL:configURL warnings:&warnings];
        for (NSString* warning in warnings) {
            fprintf(stderr, "config: %s\n", warning.UTF8String);
        }

        [DPController setVerboseLogging:verbose monitorReports:monitor];
        DPController* controller = [[DPController alloc] initWithConfig:config];
        [controller start];
        CFRunLoopRun();
    }
    return 0;
}
