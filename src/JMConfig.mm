#import "JMConfig.h"
#import "JMConfigEngine.h"

#include "Joycon2InputMapping.h"
#include "Joycon2Packet.h"

#include <cerrno>
#include <cmath>
#include <fcntl.h>
#include <string>
#include <sys/stat.h>
#include <unistd.h>
#include <vector>

const double JMPointerSpeedMin = 0.1;
const double JMPointerSpeedMax = 10.0;
const double JMRepeatDelayMax = 5.0;
const double JMRepeatIntervalMax = 2.0;

static const double kConfigVersion = 1;
static const size_t kMaxConfigBytes = 64 * 1024;
static NSString* const kErrorDomain = @"JMConfig";

static NSString* const kVersionKey = @"version";
static NSString* const kKeyMappingsKey = @"keyMappings";
static NSString* const kPointerSpeedKey = @"pointerSpeed";
static NSString* const kRepeatDelayKey = @"repeatDelay";
static NSString* const kRepeatIntervalKey = @"repeatInterval";
static NSString* const kScrollEnabledKey = @"scrollEnabled";

NSArray<NSString*>* JMMappableButtons(void) {
    static NSArray<NSString*>* buttons;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        NSMutableArray* names = [NSMutableArray array];
        for (const std::string& name : joycon2ButtonNames(0xFFFFFFFF)) {
            uint32_t mask = 0;
            joycon2ButtonMask(name, &mask);
            if (mouseButtonsForJoyconButtons(mask) == 0) {
                [names addObject:@(name.c_str())];
            }
        }
        buttons = [names copy];
    });
    return buttons;
}

static BOOL isMouseButtonName(NSString* name) {
    uint32_t mask = 0;
    return joycon2ButtonMask(name.UTF8String, &mask) && mouseButtonsForJoyconButtons(mask) != 0;
}

static BOOL isBoolean(id value) {
    return [value isKindOfClass:[NSNumber class]] && CFGetTypeID((__bridge CFTypeRef)value) == CFBooleanGetTypeID();
}

static BOOL readNumber(id value, double* out) {
    if (![value isKindOfClass:[NSNumber class]] || isBoolean(value)) {
        return NO;
    }
    double number = [value doubleValue];
    if (!std::isfinite(number)) {
        return NO;
    }
    *out = number;
    return YES;
}

static void readSetting(NSDictionary* dict, NSString* key, double min, double max, double* value, NSMutableArray* warnings) {
    id raw = dict[key];
    if (!raw) {
        return;
    }
    double number;
    if (!readNumber(raw, &number) || number < min || number > max) {
        [warnings addObject:[NSString stringWithFormat:@"%@ must be a number from %g to %g; using %g.", key, min, max, *value]];
        return;
    }
    *value = number;
}

@implementation JMConfig

+ (instancetype)defaultConfig {
    JMConfig* config = [[JMConfig alloc] init];
    EngineSettings defaults;
    NSMutableDictionary* mappings = [NSMutableDictionary dictionary];
    for (const ButtonKeyMapping& mapping : defaults.keyMappings) {
        std::string name;
        if (joycon2ButtonName(mapping.buttonMask, &name)) {
            mappings[@(name.c_str())] = @(mapping.keyCode);
        }
    }
    config.keyMappings = mappings;
    config.pointerSpeed = defaults.pointerSpeed;
    config.repeatDelay = defaults.repeatDelay;
    config.repeatInterval = defaults.repeatInterval;
    config.scrollEnabled = defaults.scrollEnabled;
    return config;
}

+ (NSURL*)defaultFileURL {
    NSURL* support = [[NSFileManager defaultManager] URLsForDirectory:NSApplicationSupportDirectory inDomains:NSUserDomainMask].firstObject;
    return [[support URLByAppendingPathComponent:@"JoyMouse" isDirectory:YES] URLByAppendingPathComponent:@"config.json"];
}

- (id)copyWithZone:(NSZone*)zone {
    JMConfig* copy = [[JMConfig allocWithZone:zone] init];
    copy.keyMappings = self.keyMappings;
    copy.pointerSpeed = self.pointerSpeed;
    copy.repeatDelay = self.repeatDelay;
    copy.repeatInterval = self.repeatInterval;
    copy.scrollEnabled = self.scrollEnabled;
    return copy;
}

+ (instancetype)configWithJSONObject:(id)object warnings:(NSMutableArray*)warnings {
    JMConfig* config = [self defaultConfig];
    if (![object isKindOfClass:[NSDictionary class]]) {
        [warnings addObject:@"Config is not a JSON object; using defaults."];
        return config;
    }
    NSDictionary* dict = object;

    double version = 0;
    if (!readNumber(dict[kVersionKey], &version) || version != kConfigVersion) {
        [warnings addObject:@"Config version is missing or unsupported (expected 1); using defaults."];
        return config;
    }

    NSSet* known = [NSSet setWithObjects:kVersionKey, kKeyMappingsKey, kPointerSpeedKey, kRepeatDelayKey, kRepeatIntervalKey, kScrollEnabledKey, nil];
    for (NSString* key in [dict.allKeys sortedArrayUsingSelector:@selector(compare:)]) {
        if (![known containsObject:key]) {
            [warnings addObject:[NSString stringWithFormat:@"Unknown setting \"%@\" ignored.", key]];
        }
    }

    id mappings = dict[kKeyMappingsKey];
    if (mappings && ![mappings isKindOfClass:[NSDictionary class]]) {
        [warnings addObject:@"keyMappings must be an object; using the default mappings."];
    } else if (mappings) {
        NSSet* mappable = [NSSet setWithArray:JMMappableButtons()];
        NSMutableDictionary* parsed = [NSMutableDictionary dictionary];
        for (NSString* button in [[mappings allKeys] sortedArrayUsingSelector:@selector(compare:)]) {
            double code;
            if (isMouseButtonName(button)) {
                [warnings addObject:[NSString stringWithFormat:@"%@ is a mouse button and cannot be mapped to a key.", button]];
            } else if (![mappable containsObject:button]) {
                [warnings addObject:[NSString stringWithFormat:@"Unknown button \"%@\" in keyMappings ignored.", button]];
            } else if (!readNumber(mappings[button], &code) || code != std::floor(code) || code < 0 || code > 127) {
                [warnings addObject:[NSString stringWithFormat:@"Key code for %@ must be a whole number from 0 to 127; mapping ignored.", button]];
            } else {
                parsed[button] = @((int)code);
            }
        }
        config.keyMappings = parsed;
    }

    double pointerSpeed = config.pointerSpeed;
    double repeatDelay = config.repeatDelay;
    double repeatInterval = config.repeatInterval;
    readSetting(dict, kPointerSpeedKey, JMPointerSpeedMin, JMPointerSpeedMax, &pointerSpeed, warnings);
    readSetting(dict, kRepeatDelayKey, 0, JMRepeatDelayMax, &repeatDelay, warnings);
    readSetting(dict, kRepeatIntervalKey, 0, JMRepeatIntervalMax, &repeatInterval, warnings);
    config.pointerSpeed = pointerSpeed;
    config.repeatDelay = repeatDelay;
    config.repeatInterval = repeatInterval;

    id scroll = dict[kScrollEnabledKey];
    if (scroll && !isBoolean(scroll)) {
        [warnings addObject:@"scrollEnabled must be true or false; using true."];
    } else if (scroll) {
        config.scrollEnabled = [scroll boolValue];
    }
    return config;
}

+ (instancetype)configWithJSONData:(NSData*)data warnings:(NSArray<NSString*>**)warnings {
    NSMutableArray* found = [NSMutableArray array];
    JMConfig* config;
    NSError* error = nil;
    id object = data ? [NSJSONSerialization JSONObjectWithData:data options:0 error:&error] : nil;
    if (!object) {
        [found addObject:[NSString stringWithFormat:@"Config is not valid JSON (%@); using defaults.",
                          error.localizedDescription ?: @"no data"]];
        config = [self defaultConfig];
    } else {
        config = [self configWithJSONObject:object warnings:found];
    }
    if (warnings) *warnings = found;
    return config;
}

+ (instancetype)configWithContentsOfURL:(NSURL*)url warnings:(NSArray<NSString*>**)warnings {
    if (warnings) *warnings = @[];
    // O_NONBLOCK: opening a FIFO must not hang; it is rejected below.
    int fd = open(url.fileSystemRepresentation, O_RDONLY | O_NONBLOCK | O_CLOEXEC);
    if (fd < 0) {
        if (errno != ENOENT && warnings) {
            *warnings = @[[NSString stringWithFormat:@"Could not open %@ (%s); using defaults.", url.path, strerror(errno)]];
        }
        return [self defaultConfig];
    }

    NSString* problem = nil;
    struct stat info;
    std::vector<uint8_t> bytes;
    if (fstat(fd, &info) != 0 || !S_ISREG(info.st_mode)) {
        problem = @"is not a regular file";
    } else {
        // Read at most one byte past the cap, even if the file grows meanwhile.
        bytes.resize(kMaxConfigBytes + 1);
        size_t total = 0;
        while (total < bytes.size()) {
            ssize_t n = read(fd, bytes.data() + total, bytes.size() - total);
            if (n < 0 && errno == EINTR) continue;
            if (n < 0) { problem = @"could not be read"; break; }
            if (n == 0) break;
            total += (size_t)n;
        }
        bytes.resize(total);
        if (!problem && total > kMaxConfigBytes) {
            problem = @"is larger than 64 KB";
        }
    }
    close(fd);

    if (problem) {
        if (warnings) *warnings = @[[NSString stringWithFormat:@"%@ %@; using defaults.", url.path, problem]];
        return [self defaultConfig];
    }
    return [self configWithJSONData:[NSData dataWithBytes:bytes.data() length:bytes.size()] warnings:warnings];
}

- (NSDictionary*)dictionaryRepresentation {
    return @{
        kVersionKey: @(kConfigVersion),
        kKeyMappingsKey: self.keyMappings ?: @{},
        kPointerSpeedKey: @(self.pointerSpeed),
        kRepeatDelayKey: @(self.repeatDelay),
        kRepeatIntervalKey: @(self.repeatInterval),
        kScrollEnabledKey: @(self.scrollEnabled),
    };
}

- (NSArray<NSString*>*)validationProblems {
    // Same rules as loading, so what the app can save is exactly what it can load.
    NSMutableArray* problems = [NSMutableArray array];
    [JMConfig configWithJSONObject:[self dictionaryRepresentation] warnings:problems];
    return problems;
}

- (NSData*)JSONData {
    if ([self validationProblems].count > 0) {
        return nil;
    }
    return [NSJSONSerialization dataWithJSONObject:[self dictionaryRepresentation]
                                           options:NSJSONWritingPrettyPrinted | NSJSONWritingSortedKeys
                                             error:nil];
}

static BOOL failWithErrno(NSError** error, NSString* what) {
    int code = errno;
    if (error) {
        *error = [NSError errorWithDomain:NSPOSIXErrorDomain code:code
                                 userInfo:@{NSLocalizedDescriptionKey: [NSString stringWithFormat:@"%@: %s", what, strerror(code)]}];
    }
    return NO;
}

- (BOOL)writeToURL:(NSURL*)url error:(NSError**)error {
    NSArray* problems = [self validationProblems];
    if (problems.count > 0) {
        if (error) {
            *error = [NSError errorWithDomain:kErrorDomain code:1
                                     userInfo:@{NSLocalizedDescriptionKey: [problems componentsJoinedByString:@" "]}];
        }
        return NO;
    }
    NSData* data = [self JSONData];

    NSString* directory = url.path.stringByDeletingLastPathComponent;
    if (![[NSFileManager defaultManager] createDirectoryAtPath:directory withIntermediateDirectories:YES
                                                    attributes:@{NSFilePosixPermissions: @0700} error:error]) {
        return NO;
    }

    // Write a temp file in the same directory, then rename over the target:
    // rename replaces a symlink at the target instead of writing through it.
    std::string pattern = std::string(directory.fileSystemRepresentation) + "/.config.json.XXXXXX";
    std::vector<char> tempPath(pattern.begin(), pattern.end());
    tempPath.push_back('\0');
    int fd = mkstemp(tempPath.data());
    if (fd < 0) {
        return failWithErrno(error, @"Could not create a temporary config file");
    }

    const uint8_t* bytes = (const uint8_t*)data.bytes;
    size_t written = 0;
    BOOL ok = YES;
    while (ok && written < data.length) {
        ssize_t n = write(fd, bytes + written, data.length - written);
        if (n < 0 && errno == EINTR) continue;
        if (n <= 0) ok = NO;
        else written += (size_t)n;
    }
    ok = ok && fchmod(fd, 0600) == 0 && fsync(fd) == 0;
    if (close(fd) != 0) ok = NO;
    if (ok && rename(tempPath.data(), url.fileSystemRepresentation) != 0) ok = NO;
    if (!ok) {
        int code = errno;
        unlink(tempPath.data());
        errno = code;
        return failWithErrno(error, @"Could not save the config");
    }
    return YES;
}

@end

EngineSettings JMEngineSettingsFromConfig(JMConfig* config) {
    EngineSettings settings;
    settings.keyMappings.clear();
    // Fixed button order keeps simultaneous key events in a stable order.
    for (NSString* button in JMMappableButtons()) {
        NSNumber* code = config.keyMappings[button];
        uint32_t mask = 0;
        if (code && joycon2ButtonMask(button.UTF8String, &mask)) {
            settings.keyMappings.push_back({mask, (CGKeyCode)code.intValue});
        }
    }
    settings.pointerSpeed = config.pointerSpeed;
    settings.repeatDelay = config.repeatDelay;
    settings.repeatInterval = config.repeatInterval;
    settings.scrollEnabled = config.scrollEnabled;
    return settings;
}
