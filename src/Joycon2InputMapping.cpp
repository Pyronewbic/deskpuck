#include "Joycon2InputMapping.h"

#include <cmath>

DisplayLookup systemDisplayLookup() {
    return [](CGPoint point, CGRect* bounds) {
        CGDirectDisplayID display;
        uint32_t displayCount = 0;
        if (CGGetDisplaysWithPoint(point, 1, &display, &displayCount) != kCGErrorSuccess || displayCount == 0) {
            return false;
        }
        *bounds = CGDisplayBounds(display);
        return true;
    };
}

CGPoint clampToDisplays(CGPoint target, CGPoint from, const DisplayLookup& displayAt, CGRect fallbackBounds) {
    CGRect bounds;
    if (displayAt(target, &bounds)) {
        return target;
    }
    if (!displayAt(from, &bounds)) {
        bounds = fallbackBounds;
    }
    CGPoint clamped;
    clamped.x = fmax(CGRectGetMinX(bounds), fmin(target.x, CGRectGetMaxX(bounds) - 1));
    clamped.y = fmax(CGRectGetMinY(bounds), fmin(target.y, CGRectGetMaxY(bounds) - 1));
    return clamped;
}

CGEventType mouseMoveEventType(uint8_t mouseBtnState, CGMouseButton* button) {
    if (mouseBtnState & 1) {
        *button = kCGMouseButtonLeft;
        return kCGEventLeftMouseDragged;
    }
    if (mouseBtnState & 2) {
        *button = kCGMouseButtonRight;
        return kCGEventRightMouseDragged;
    }
    if (mouseBtnState & 4) {
        *button = kCGMouseButtonCenter;
        return kCGEventOtherMouseDragged;
    }
    *button = kCGMouseButtonLeft;
    return kCGEventMouseMoved;
}
