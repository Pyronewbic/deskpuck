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

const std::vector<ButtonKeyMapping>& defaultButtonKeyMappings() {
    static const std::vector<ButtonKeyMapping> mappings = {
        {0x00040000, 36},  // RS (right stick click) -> Return
        {0x00000200, 126}, // X -> Up
        {0x00000400, 125}, // B -> Down
        {0x00000100, 123}, // Y -> Left
        {0x00000800, 124}, // A -> Right
    };
    return mappings;
}

KeyRepeater::KeyRepeater(std::vector<ButtonKeyMapping> mappings, double repeatDelay, double repeatInterval)
    : mappings_(std::move(mappings)),
      nextRepeatAt_(mappings_.size(), 0),
      repeatDelay_(repeatDelay),
      repeatInterval_(repeatInterval),
      repeatEnabled_(repeatDelay >= 0 && repeatInterval > 0) {}

std::vector<KeyEvent> KeyRepeater::update(uint32_t buttons, double now) {
    std::vector<KeyEvent> events;
    for (size_t i = 0; i < mappings_.size(); i++) {
        const ButtonKeyMapping& mapping = mappings_[i];
        bool isDown = buttons & mapping.buttonMask;
        bool wasDown = lastButtons_ & mapping.buttonMask;
        if (isDown != wasDown) {
            events.push_back({mapping.keyCode, isDown, false});
            nextRepeatAt_[i] = now + repeatDelay_;
        } else if (isDown && repeatEnabled_ && now >= nextRepeatAt_[i]) {
            events.push_back({mapping.keyCode, true, true});
            nextRepeatAt_[i] = now + repeatInterval_;
        }
    }
    lastButtons_ = buttons;
    return events;
}
