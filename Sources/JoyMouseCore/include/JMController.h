#import <Foundation/Foundation.h>

#import "JMConfig.h"

NS_ASSUME_NONNULL_BEGIN

typedef NS_ENUM(NSInteger, JMConnectionState) {
    JMConnectionStateBluetoothOff,
    JMConnectionStateBluetoothUnauthorized,
    JMConnectionStateSearching,
    JMConnectionStateConnecting,
    JMConnectionStateConnected,
};

// Connects to a Joy-Con 2 and turns its reports into mouse and keyboard events.
@interface JMController : NSObject

- (instancetype)initWithConfig:(JMConfig*)config NS_DESIGNATED_INITIALIZER;
- (instancetype)init;

// Off by default: only connection status and errors are printed.
+ (void)setVerboseLogging:(BOOL)verbose monitorReports:(BOOL)monitor;

- (void)start;

@property (nonatomic, readonly) JMConnectionState connectionState;
// Name of the Joy-Con being connected or connected to.
@property (nonatomic, readonly, copy, nullable) NSString* deviceName;
// Called on the main queue after connectionState or deviceName changes.
@property (nonatomic, copy, nullable) void (^stateDidChange)(void);

// While paused, held input is released and reports are ignored. Resuming
// does not move the cursor.
@property (nonatomic, getter=isPaused) BOOL paused;
// Takes effect immediately. Refuses a config with validation problems.
- (BOOL)applyConfig:(JMConfig*)config error:(NSError**)error;

@end

NS_ASSUME_NONNULL_END
