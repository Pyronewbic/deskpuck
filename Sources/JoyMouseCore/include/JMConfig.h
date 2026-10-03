#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

// Joy-Con button names that can be mapped to keys. R, ZR, ZL, L and LS are
// mouse buttons and cannot be mapped.
FOUNDATION_EXPORT NSArray<NSString*>* JMMappableButtons(void);

FOUNDATION_EXPORT const double JMPointerSpeedMin;     // 0.1
FOUNDATION_EXPORT const double JMPointerSpeedMax;     // 10
FOUNDATION_EXPORT const double JMRepeatDelayMax;      // 5 s
FOUNDATION_EXPORT const double JMRepeatIntervalMax;   // 2 s; 0 turns repeat off

// User settings, stored as JSON. Loading never fails: anything unusable falls
// back to its default and is described in `warnings`.
@interface JMConfig : NSObject <NSCopying>

// Button name -> macOS virtual key code (0-127).
@property (nonatomic, copy) NSDictionary<NSString*, NSNumber*>* keyMappings;
@property (nonatomic) double pointerSpeed;
@property (nonatomic) double repeatDelay;
@property (nonatomic) double repeatInterval;
@property (nonatomic) BOOL scrollEnabled;

+ (instancetype)defaultConfig;
+ (NSURL*)defaultFileURL;

+ (instancetype)configWithJSONData:(nullable NSData*)data
                          warnings:(NSArray<NSString*>* _Nullable * _Nullable)warnings;
// A missing file gives the defaults without a warning.
+ (instancetype)configWithContentsOfURL:(NSURL*)url
                               warnings:(NSArray<NSString*>* _Nullable * _Nullable)warnings;

// Empty when every value is in range.
- (NSArray<NSString*>*)validationProblems;
// nil if validationProblems is not empty.
- (nullable NSData*)JSONData;
// Refuses (returns NO) if validationProblems is not empty. Replaces the file
// atomically and never writes through a symlink at `url`.
- (BOOL)writeToURL:(NSURL*)url error:(NSError**)error;

@end

NS_ASSUME_NONNULL_END
