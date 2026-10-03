#import "JMController.h"

#import <ApplicationServices/ApplicationServices.h>

#import "Joycon2BLEReceiver.h"
#include "Joycon2Engine.h"
#include "Joycon2InputMapping.h"

#include <cmath>

@implementation JMController {
    Joycon2BLEReceiver* _receiver;
    InputEngine _engine;
    uint8_t _postedMouseButtons;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _receiver = [[Joycon2BLEReceiver alloc] init];
        __weak JMController* weakSelf = self;
        _receiver.onConnected = ^{
            JMController* strongSelf = weakSelf;
            if (strongSelf) strongSelf->_engine.connectionStarted();
        };
        _receiver.onDisconnected = ^{
            [weakSelf handleDisconnect];
        };
        _receiver.onReportReceived = ^(const Joycon2Report& report) {
            [weakSelf handleReport:report];
        };
    }
    return self;
}

- (void)start {
    [_receiver startScan];
}

- (void)handleReport:(const Joycon2Report&)report {
    [self postOutput:_engine.process(report, CFAbsoluteTimeGetCurrent())];
}

- (void)handleDisconnect {
    [self postOutput:_engine.disconnected()];
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
