#import "JMController.h"

#if !__has_feature(objc_arc)
#error "JoyMouseCore must be built with ARC"
#endif

#import <ApplicationServices/ApplicationServices.h>

#import "JMConfigEngine.h"
#import "Joycon2BLEReceiver.h"
#include "Joycon2Engine.h"
#include "Joycon2InputMapping.h"

#include <cmath>

@implementation JMController {
    Joycon2BLEReceiver* _receiver;
    InputEngine _engine;
    uint8_t _postedMouseButtons;
}

@synthesize paused = _paused;

- (instancetype)init {
    return [self initWithConfig:[JMConfig defaultConfig]];
}

- (instancetype)initWithConfig:(JMConfig*)config {
    self = [super init];
    if (self) {
        if ([config validationProblems].count == 0) {
            _engine.applySettings(JMEngineSettingsFromConfig(config));
        }
        _connectionState = JMConnectionStateBluetoothOff;
        _receiver = [[Joycon2BLEReceiver alloc] init];
        __weak JMController* weakSelf = self;
        _receiver.onBluetoothStateChanged = ^(CBManagerState state) {
            JMConnectionState next = JMConnectionStateBluetoothOff;
            if (state == CBManagerStatePoweredOn) next = JMConnectionStateSearching;
            if (state == CBManagerStateUnauthorized) next = JMConnectionStateBluetoothUnauthorized;
            [weakSelf setState:next name:nil];
        };
        _receiver.onConnecting = ^(NSString* name) {
            [weakSelf setState:JMConnectionStateConnecting name:name];
        };
        _receiver.onConnectionFailed = ^{
            [weakSelf setState:JMConnectionStateSearching name:nil];
        };
        _receiver.onConnected = ^{
            JMController* strongSelf = weakSelf;
            if (!strongSelf) return;
            strongSelf->_engine.connectionStarted();
            [strongSelf setState:JMConnectionStateConnected name:strongSelf->_receiver.connectedPeripheral.name];
        };
        _receiver.onDisconnected = ^{
            [weakSelf handleDisconnect];
            [weakSelf setState:JMConnectionStateSearching name:nil];
        };
        _receiver.onReportReceived = ^(const Joycon2Report& report) {
            [weakSelf handleReport:report];
        };
    }
    return self;
}

+ (void)setVerboseLogging:(BOOL)verbose monitorReports:(BOOL)monitor {
    Joycon2SetLogging(verbose, monitor);
}

- (void)start {
    [_receiver startScan];
}

- (void)setState:(JMConnectionState)state name:(NSString*)name {
    _connectionState = state;
    _deviceName = [name copy];
    if (self.stateDidChange) {
        self.stateDidChange();
    }
}

- (void)setPaused:(BOOL)paused {
    if (paused == _paused) {
        return;
    }
    _paused = paused;
    if (paused) {
        [self postOutput:_engine.disconnected()];
    } else {
        // Re-baseline so motion while paused does not jump the cursor.
        _engine.connectionStarted();
    }
}

- (BOOL)applyConfig:(JMConfig*)config error:(NSError**)error {
    NSArray* problems = [config validationProblems];
    if (problems.count > 0) {
        if (error) {
            *error = [NSError errorWithDomain:@"JMConfig" code:1
                                     userInfo:@{NSLocalizedDescriptionKey: [problems componentsJoinedByString:@" "]}];
        }
        return NO;
    }
    [self postKeys:_engine.applySettings(JMEngineSettingsFromConfig(config))];
    return YES;
}

- (void)handleReport:(const Joycon2Report&)report {
    if (_paused) {
        return;
    }
    [self postOutput:_engine.process(report, CFAbsoluteTimeGetCurrent())];
}

- (void)handleDisconnect {
    if (!_paused) {
        [self postOutput:_engine.disconnected()];
    }
}

- (void)postOutput:(const EngineOutput&)out {
    [self postMoveDx:out.dx dy:out.dy];
    [self postMouseButtons:out.mouseButtons];
    [self postWheel:out.wheel];
    [self postKeys:out.keys];
}

static CGPoint currentCursorPosition() {
    CGEventRef event = CGEventCreate(NULL);
    CGPoint position = CGEventGetLocation(event);
    CFRelease(event);
    return position;
}

- (void)postMoveDx:(double)dx dy:(double)dy {
    if (dx == 0 && dy == 0) {
        return;
    }
    CGPoint previous = currentCursorPosition();
    CGPoint target = CGPointMake(previous.x + dx, previous.y + dy);
    target = clampToDisplays(target, previous, systemDisplayLookup(), CGDisplayBounds(CGMainDisplayID()));

    // A real move/drag event (not a warp) so the Dock, hot corners and drags see it
    CGMouseButton button;
    CGEventType type = mouseMoveEventType(_postedMouseButtons, &button);
    CGEventRef event = CGEventCreateMouseEvent(NULL, type, target, button);
    CGEventSetIntegerValueField(event, kCGMouseEventDeltaX, lround(target.x - previous.x));
    CGEventSetIntegerValueField(event, kCGMouseEventDeltaY, lround(target.y - previous.y));
    CGEventPost(kCGHIDEventTap, event);
    CFRelease(event);
}

- (void)postMouseButtons:(uint8_t)buttons {
    static const struct { uint8_t bit; CGEventType down; CGEventType up; CGMouseButton button; } kMouseButtons[] = {
        {1, kCGEventLeftMouseDown, kCGEventLeftMouseUp, kCGMouseButtonLeft},
        {2, kCGEventRightMouseDown, kCGEventRightMouseUp, kCGMouseButtonRight},
        {4, kCGEventOtherMouseDown, kCGEventOtherMouseUp, kCGMouseButtonCenter},
    };
    CGPoint position = currentCursorPosition();
    for (const auto& b : kMouseButtons) {
        bool isDown = buttons & b.bit;
        bool wasDown = _postedMouseButtons & b.bit;
        if (isDown != wasDown) {
            CGEventRef event = CGEventCreateMouseEvent(NULL, isDown ? b.down : b.up, position, b.button);
            CGEventPost(kCGHIDEventTap, event);
            CFRelease(event);
        }
    }
    _postedMouseButtons = buttons;
}

- (void)postWheel:(int)wheel {
    if (wheel == 0) {
        return;
    }
    CGEventRef event = CGEventCreateScrollWheelEvent(NULL, kCGScrollEventUnitPixel, 1, -wheel);
    CGEventPost(kCGHIDEventTap, event);
    CFRelease(event);
}

- (void)postKeys:(const std::vector<KeyEvent>&)keys {
    for (const KeyEvent& key : keys) {
        CGEventRef event = CGEventCreateKeyboardEvent(NULL, key.keyCode, key.isDown);
        if (key.isRepeat) {
            CGEventSetIntegerValueField(event, kCGKeyboardEventAutorepeat, 1);
        }
        CGEventPost(kCGHIDEventTap, event);
        CFRelease(event);
    }
}

@end
