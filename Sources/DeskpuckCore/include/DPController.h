#import <Foundation/Foundation.h>

#import "DPConfig.h"

NS_ASSUME_NONNULL_BEGIN

typedef NS_ENUM(NSInteger, DPConnectionState) {
    DPConnectionStateBluetoothOff,
    DPConnectionStateBluetoothUnauthorized,
    DPConnectionStateSearching,
    DPConnectionStateConnecting,
    DPConnectionStateConnected,
};

// Connects to a Joy-Con 2 and turns its reports into mouse and keyboard events.
@interface DPController : NSObject

- (instancetype)initWithConfig:(DPConfig*)config NS_DESIGNATED_INITIALIZER;
- (instancetype)init;

// Off by default: only connection status and errors are printed.
+ (void)setVerboseLogging:(BOOL)verbose monitorReports:(BOOL)monitor;

- (void)start;

@property (nonatomic, readonly) DPConnectionState connectionState;
// Name of the Joy-Con being connected or connected to.
@property (nonatomic, readonly, copy, nullable) NSString* deviceName;
// Called on the main queue after connectionState or deviceName changes.
@property (nonatomic, copy, nullable) void (^stateDidChange)(void);

// While paused, held input is released, reports are ignored, and no Joy-Con is
// searched for; a connected one stays connected. Resuming does not move the cursor.
@property (nonatomic, getter=isPaused) BOOL paused;
// Takes effect immediately. Refuses a config with validation problems.
- (BOOL)applyConfig:(DPConfig*)config error:(NSError**)error;

@end

NS_ASSUME_NONNULL_END
