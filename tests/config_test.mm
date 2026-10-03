#import "JMConfig.h"
#import "JMConfigEngine.h"

#include "check.h"

#include <cstdlib>
#include <string>
#include <sys/stat.h>
#include <unistd.h>

static JMConfig* parse(NSString* json, NSArray<NSString*>** warnings) {
    return [JMConfig configWithJSONData:[json dataUsingEncoding:NSUTF8StringEncoding] warnings:warnings];
}

static BOOL warned(NSArray<NSString*>* warnings, NSString* fragment) {
    for (NSString* warning in warnings) {
        if ([warning containsString:fragment]) return YES;
    }
    return NO;
}

static BOOL isDefault(JMConfig* config) {
    JMConfig* d = [JMConfig defaultConfig];
    return [config.keyMappings isEqualToDictionary:d.keyMappings] && config.pointerSpeed == d.pointerSpeed &&
           config.repeatDelay == d.repeatDelay && config.repeatInterval == d.repeatInterval &&
           config.scrollEnabled == d.scrollEnabled;
}

static NSString* tempDirectory() {
    std::string pattern = std::string(NSTemporaryDirectory().fileSystemRepresentation) + "/jmconfig.XXXXXX";
    char* path = mkdtemp(pattern.data());
    return path ? @(path) : nil;
}

static void testDefaultsMatchEngine() {
    JMConfig* config = [JMConfig defaultConfig];
    NSDictionary* expected = @{@"RS": @36, @"X": @126, @"B": @125, @"Y": @123, @"A": @124};
    CHECK([config.keyMappings isEqualToDictionary:expected]);
    CHECK([config validationProblems].count == 0);

    EngineSettings fromConfig = JMEngineSettingsFromConfig(config);
    EngineSettings engineDefaults;
    CHECK(fromConfig.keyMappings.size() == engineDefaults.keyMappings.size());
    for (const ButtonKeyMapping& mapping : engineDefaults.keyMappings) {
        bool found = false;
        for (const ButtonKeyMapping& m : fromConfig.keyMappings) {
            if (m.buttonMask == mapping.buttonMask && m.keyCode == mapping.keyCode) found = true;
        }
        CHECK(found);
    }
    CHECK(fromConfig.pointerSpeed == engineDefaults.pointerSpeed);
    CHECK(fromConfig.repeatDelay == engineDefaults.repeatDelay);
    CHECK(fromConfig.repeatInterval == engineDefaults.repeatInterval);
    CHECK(fromConfig.scrollEnabled == engineDefaults.scrollEnabled);
}

static void testMappableButtons() {
    NSArray* buttons = JMMappableButtons();
    CHECK([buttons containsObject:@"RS"] && [buttons containsObject:@"A"] && [buttons containsObject:@"HOME"]);
    for (NSString* mouse in @[@"R", @"ZR", @"ZL", @"L", @"LS"]) {
        CHECK(![buttons containsObject:mouse]);
    }
    CHECK(buttons.count == 18);
}

static void testRoundTrip() {
    JMConfig* config = [JMConfig defaultConfig];
    config.keyMappings = @{@"HOME": @53, @"PLUS": @49};
    config.pointerSpeed = 2.5;
    config.repeatDelay = 0.25;
    config.repeatInterval = 0;
    config.scrollEnabled = NO;
    NSArray* warnings = nil;
    JMConfig* back = [JMConfig configWithJSONData:[config JSONData] warnings:&warnings];
    CHECK(warnings.count == 0);
    CHECK([back.keyMappings isEqualToDictionary:config.keyMappings]);
    CHECK(back.pointerSpeed == 2.5 && back.repeatDelay == 0.25 && back.repeatInterval == 0 && !back.scrollEnabled);
}

static void testWholeFileFallbacks() {
    NSArray* warnings = nil;
    CHECK(isDefault(parse(@"{not json", &warnings)) && warned(warnings, @"not valid JSON"));
    CHECK(isDefault(parse(@"[1, 2]", &warnings)) && warned(warnings, @"not a JSON object"));
    CHECK(isDefault([JMConfig configWithJSONData:nil warnings:&warnings]) && warnings.count == 1);

    // Missing or future versions ignore otherwise valid settings.
    CHECK(isDefault(parse(@"{\"pointerSpeed\": 3}", &warnings)) && warned(warnings, @"version"));
    CHECK(isDefault(parse(@"{\"version\": 2, \"pointerSpeed\": 3}", &warnings)) && warned(warnings, @"version"));
    CHECK(isDefault(parse(@"{\"version\": true, \"pointerSpeed\": 3}", &warnings)) && warned(warnings, @"version"));

    // Positive control: version 1 applies the same setting.
    JMConfig* config = parse(@"{\"version\": 1, \"pointerSpeed\": 3}", &warnings);
    CHECK(config.pointerSpeed == 3 && warnings.count == 0);
}

static void testNumberBounds() {
    NSArray* warnings = nil;
    for (NSString* bad in @[@"0", @"0.05", @"10.5", @"-1", @"\"fast\"", @"true", @"null", @"[]"]) {
        NSString* json = [NSString stringWithFormat:@"{\"version\": 1, \"pointerSpeed\": %@}", bad];
        JMConfig* config = parse(json, &warnings);
        CHECK(config.pointerSpeed == 1.0);
        CHECK(warned(warnings, @"pointerSpeed"));
    }
    CHECK(parse(@"{\"version\": 1, \"pointerSpeed\": 0.1}", &warnings).pointerSpeed == 0.1 && warnings.count == 0);
    CHECK(parse(@"{\"version\": 1, \"pointerSpeed\": 10}", &warnings).pointerSpeed == 10 && warnings.count == 0);

    CHECK(parse(@"{\"version\": 1, \"repeatDelay\": 5.01}", &warnings).repeatDelay == 0.4 && warned(warnings, @"repeatDelay"));
    CHECK(parse(@"{\"version\": 1, \"repeatDelay\": 0}", &warnings).repeatDelay == 0 && warnings.count == 0);
    CHECK(parse(@"{\"version\": 1, \"repeatInterval\": -0.1}", &warnings).repeatInterval == 0.06 && warned(warnings, @"repeatInterval"));
    CHECK(parse(@"{\"version\": 1, \"repeatInterval\": 3}", &warnings).repeatInterval == 0.06 && warned(warnings, @"repeatInterval"));
}

static void testRepeatOffSentinel() {
    // 0 is the documented "repeat off" value: it must pass through unclamped and
    // switch repeat off, not fall back to the default (which repeats).
    NSArray* warnings = nil;
    JMConfig* config = parse(@"{\"version\": 1, \"repeatInterval\": 0}", &warnings);
    CHECK(config.repeatInterval == 0 && warnings.count == 0);

    InputEngine engine(JMEngineSettingsFromConfig(config));
    Joycon2Report held;
    memset(&held, 0, sizeof(held));
    held.buttons = 0x00040000;
    size_t events = 0;
    for (double t = 0; t < 3.0; t += 0.03) events += engine.process(held, t).keys.size();
    CHECK(events == 1);

    // Positive control: the default interval repeats under the same hold.
    InputEngine repeating(JMEngineSettingsFromConfig([JMConfig defaultConfig]));
    events = 0;
    for (double t = 0; t < 3.0; t += 0.03) events += repeating.process(held, t).keys.size();
    CHECK(events > 1);
}

static void testKeyMappings() {
    NSArray* warnings = nil;
    // An explicit empty object clears every mapping; that is a choice, not an error.
    JMConfig* cleared = parse(@"{\"version\": 1, \"keyMappings\": {}}", &warnings);
    CHECK(cleared.keyMappings.count == 0 && warnings.count == 0);

    // A malformed section never means "no mappings".
    for (NSString* bad in @[@"\"RS\"", @"[36]", @"36", @"null"]) {
        NSString* json = [NSString stringWithFormat:@"{\"version\": 1, \"keyMappings\": %@}", bad];
        JMConfig* config = parse(json, &warnings);
        CHECK([config.keyMappings isEqualToDictionary:[JMConfig defaultConfig].keyMappings]);
        CHECK(warned(warnings, @"keyMappings must be an object"));
    }

    // Bad entries are dropped one by one; good ones survive.
    JMConfig* mixed = parse(@"{\"version\": 1, \"keyMappings\": {"
                             "\"A\": 49, \"Q\": 36, \"rs\": 36, \"R\": 36, \"LS\": 36,"
                             "\"B\": 128, \"X\": -1, \"Y\": 36.5, \"HOME\": true, \"PLUS\": \"36\", \"CHAT\": 0}}", &warnings);
    CHECK([mixed.keyMappings isEqualToDictionary:(@{@"A": @49, @"CHAT": @0})]);
    CHECK(warned(warnings, @"Unknown button \"Q\""));
    CHECK(warned(warnings, @"Unknown button \"rs\""));
    CHECK(warned(warnings, @"R is a mouse button"));
    CHECK(warned(warnings, @"LS is a mouse button"));
    for (NSString* button in @[@"B", @"X", @"Y", @"HOME", @"PLUS"]) {
        CHECK(warned(warnings, [NSString stringWithFormat:@"Key code for %@", button]));
    }
    CHECK(warnings.count == 9);
}

static void testOtherFields() {
    NSArray* warnings = nil;
    CHECK(!parse(@"{\"version\": 1, \"scrollEnabled\": false}", &warnings).scrollEnabled && warnings.count == 0);
    CHECK(parse(@"{\"version\": 1, \"scrollEnabled\": 0}", &warnings).scrollEnabled && warned(warnings, @"scrollEnabled"));

    JMConfig* config = parse(@"{\"version\": 1, \"pointerSpeed\": 2, \"pointerSpeeed\": 9}", &warnings);
    CHECK(config.pointerSpeed == 2 && warned(warnings, @"Unknown setting \"pointerSpeeed\""));
}

static void testValidationProblems() {
    JMConfig* config = [JMConfig defaultConfig];
    CHECK([config validationProblems].count == 0);
    config.pointerSpeed = NAN;
    CHECK([config validationProblems].count == 1);
    CHECK([config JSONData] == nil);
    config.pointerSpeed = 1;
    config.keyMappings = @{@"ZR": @36};
    CHECK([config validationProblems].count == 1);
}

static void testFileLoading() {
    NSString* dir = tempDirectory();
    CHECK(dir != nil);
    if (!dir) return;
    NSArray* warnings = nil;

    // Missing file: defaults, silently.
    NSURL* missing = [NSURL fileURLWithPath:[dir stringByAppendingPathComponent:@"none.json"]];
    CHECK(isDefault([JMConfig configWithContentsOfURL:missing warnings:&warnings]) && warnings.count == 0);

    // A FIFO must be refused without blocking on open or read.
    NSString* fifo = [dir stringByAppendingPathComponent:@"fifo.json"];
    CHECK(mkfifo(fifo.fileSystemRepresentation, 0600) == 0);
    CHECK(isDefault([JMConfig configWithContentsOfURL:[NSURL fileURLWithPath:fifo] warnings:&warnings]));
    CHECK(warned(warnings, @"not a regular file"));

    // A directory is refused too.
    CHECK(isDefault([JMConfig configWithContentsOfURL:[NSURL fileURLWithPath:dir] warnings:&warnings]));
    CHECK(warned(warnings, @"not a regular file"));

    // Oversized, even though it would parse.
    NSMutableString* big = [NSMutableString stringWithString:@"{\"version\": 1, \"pointerSpeed\": 3"];
    while (big.length <= 64 * 1024) [big appendString:@"                "];
    [big appendString:@"}"];
    NSString* bigPath = [dir stringByAppendingPathComponent:@"big.json"];
    CHECK([big writeToFile:bigPath atomically:NO encoding:NSUTF8StringEncoding error:nil]);
    CHECK(isDefault([JMConfig configWithContentsOfURL:[NSURL fileURLWithPath:bigPath] warnings:&warnings]));
    CHECK(warned(warnings, @"larger than 64 KB"));

    // Positive control: a small valid file loads.
    NSString* goodPath = [dir stringByAppendingPathComponent:@"good.json"];
    CHECK([@"{\"version\": 1, \"pointerSpeed\": 3}" writeToFile:goodPath atomically:NO encoding:NSUTF8StringEncoding error:nil]);
    JMConfig* good = [JMConfig configWithContentsOfURL:[NSURL fileURLWithPath:goodPath] warnings:&warnings];
    CHECK(good.pointerSpeed == 3 && warnings.count == 0);

    [[NSFileManager defaultManager] removeItemAtPath:dir error:nil];
}

static void testFileWriting() {
    NSString* dir = tempDirectory();
    CHECK(dir != nil);
    if (!dir) return;

    // Creates missing parent directories; owner-only permissions.
    NSURL* url = [NSURL fileURLWithPath:[dir stringByAppendingPathComponent:@"JoyMouse/config.json"]];
    JMConfig* config = [JMConfig defaultConfig];
    config.pointerSpeed = 4;
    NSError* error = nil;
    CHECK([config writeToURL:url error:&error]);
    struct stat info;
    CHECK(stat(url.fileSystemRepresentation, &info) == 0 && (info.st_mode & 0777) == 0600);
    NSArray* warnings = nil;
    CHECK([JMConfig configWithContentsOfURL:url warnings:&warnings].pointerSpeed == 4 && warnings.count == 0);

    // A symlink at the target is replaced, not written through.
    NSString* victim = [dir stringByAppendingPathComponent:@"victim.txt"];
    CHECK([@"untouched" writeToFile:victim atomically:NO encoding:NSUTF8StringEncoding error:nil]);
    NSString* linkPath = [dir stringByAppendingPathComponent:@"link.json"];
    CHECK(symlink(victim.fileSystemRepresentation, linkPath.fileSystemRepresentation) == 0);
    CHECK([config writeToURL:[NSURL fileURLWithPath:linkPath] error:&error]);
    CHECK([[NSString stringWithContentsOfFile:victim encoding:NSUTF8StringEncoding error:nil] isEqualToString:@"untouched"]);
    CHECK(lstat(linkPath.fileSystemRepresentation, &info) == 0 && S_ISREG(info.st_mode));

    // Invalid settings are refused and leave the existing file alone.
    JMConfig* invalid = [JMConfig defaultConfig];
    invalid.pointerSpeed = 50;
    error = nil;
    CHECK(![invalid writeToURL:url error:&error] && error != nil);
    CHECK([JMConfig configWithContentsOfURL:url warnings:&warnings].pointerSpeed == 4);

    // No temp files left behind.
    NSArray* leftovers = [[NSFileManager defaultManager] contentsOfDirectoryAtPath:url.path.stringByDeletingLastPathComponent error:nil];
    CHECK(leftovers.count == 1);

    [[NSFileManager defaultManager] removeItemAtPath:dir error:nil];
}

int main() {
    @autoreleasepool {
        testDefaultsMatchEngine();
        testMappableButtons();
        testRoundTrip();
        testWholeFileFallbacks();
        testNumberBounds();
        testRepeatOffSentinel();
        testKeyMappings();
        testOtherFields();
        testValidationProblems();
        testFileLoading();
        testFileWriting();
    }
    return checkSummary("config");
}
