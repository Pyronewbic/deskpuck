#pragma once

#include <CoreGraphics/CoreGraphics.h>
#include <functional>
#include <vector>

// Bounds of the display containing a point; false if no display contains it.
typedef std::function<bool(CGPoint point, CGRect* bounds)> DisplayLookup;

DisplayLookup systemDisplayLookup();

// The cursor may cross onto any display; off all displays it is clamped to the
// display it came from, stopping on the last pixel so edge triggers (Dock) fire.
CGPoint clampToDisplays(CGPoint target, CGPoint from, const DisplayLookup& displayAt, CGRect fallbackBounds);

// Mouse button bits: 1 left, 2 right, 4 middle. Held buttons turn a move into a drag.
CGEventType mouseMoveEventType(uint8_t mouseBtnState, CGMouseButton* button);

// Joy-Con button mask (see parseButtons) -> macOS virtual key code
struct ButtonKeyMapping { uint32_t buttonMask; CGKeyCode keyCode; };
struct KeyEvent { CGKeyCode keyCode; bool isDown; bool isRepeat; };

const std::vector<ButtonKeyMapping>& defaultButtonKeyMappings();

// Turns button states into key down/up events, repeating held keys like a keyboard.
// Repeat is off unless repeatDelay >= 0 and repeatInterval > 0. At most one repeat
// per key per update, so a stalled packet stream never bursts.
class KeyRepeater {
public:
    KeyRepeater(std::vector<ButtonKeyMapping> mappings, double repeatDelay, double repeatInterval);
    std::vector<KeyEvent> update(uint32_t buttons, double now);
    // Key-up for every held key, e.g. when the controller disconnects mid-press.
    std::vector<KeyEvent> releaseAll();

private:
    std::vector<ButtonKeyMapping> mappings_;
    std::vector<double> nextRepeatAt_;
    double repeatDelay_;
    double repeatInterval_;
    bool repeatEnabled_;
    uint32_t lastButtons_ = 0;
};
