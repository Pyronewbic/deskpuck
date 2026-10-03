#include "Joycon2InputMapping.h"

#include "check.h"

#include <cmath>
#include <vector>

// Two displays: main 1920x1080 at origin, second 1280x1024 to its right, offset 200px down.
static const CGRect mainDisplay = CGRectMake(0, 0, 1920, 1080);
static const CGRect sideDisplay = CGRectMake(1920, 200, 1280, 1024);

static int lookups = 0;
static bool fakeDisplayAt(CGPoint point, CGRect* bounds) {
    lookups++;
    for (const CGRect& display : {mainDisplay, sideDisplay}) {
        if (CGRectContainsPoint(display, point)) {
            *bounds = display;
            return true;
        }
    }
    return false;
}

static bool samePoint(CGPoint a, CGPoint b) {
    return a.x == b.x && a.y == b.y;
}

static void testClampToDisplays() {
    // Control: a move within one display is unchanged (passes with or without the multi-display fix).
    CGPoint inside = clampToDisplays(CGPointMake(600, 500), CGPointMake(500, 500), fakeDisplayAt, mainDisplay);
    CHECK(samePoint(inside, CGPointMake(600, 500)));

    // Crossing onto the second display is allowed.
    lookups = 0;
    CGPoint crossed = clampToDisplays(CGPointMake(2000, 500), CGPointMake(1900, 500), fakeDisplayAt, mainDisplay);
    CHECK(samePoint(crossed, CGPointMake(2000, 500)));
    CHECK(lookups > 0);

    // Pushing past the bottom of the main display stops on its last pixel row (Dock trigger), on-screen.
    CGPoint bottom = clampToDisplays(CGPointMake(500, 1200), CGPointMake(500, 1079), fakeDisplayAt, mainDisplay);
    CHECK(samePoint(bottom, CGPointMake(500, 1079)));
    CGRect landed;
    CHECK(fakeDisplayAt(bottom, &landed));

    // Off all displays from the second display clamps to the second display, not main.
    CGPoint offSide = clampToDisplays(CGPointMake(3500, 100), CGPointMake(3000, 300), fakeDisplayAt, mainDisplay);
    CHECK(samePoint(offSide, CGPointMake(3199, 200)));

    // Gap above the offset second display: clamps back onto the display we came from.
    CGPoint gap = clampToDisplays(CGPointMake(1950, 150), CGPointMake(1950, 250), fakeDisplayAt, mainDisplay);
    CHECK(samePoint(gap, CGPointMake(1950, 200)));

    // Origin off every display (e.g. display unplugged) falls back to the given bounds.
    CGPoint lost = clampToDisplays(CGPointMake(-50, 5000), CGPointMake(-40, 4000), fakeDisplayAt, mainDisplay);
    CHECK(samePoint(lost, CGPointMake(0, 1079)));
}

static void testMouseMoveEventType() {
    CGMouseButton button = kCGMouseButtonCenter;
    CHECK(mouseMoveEventType(0, &button) == kCGEventMouseMoved);
    CHECK(button == kCGMouseButtonLeft);

    CHECK(mouseMoveEventType(1, &button) == kCGEventLeftMouseDragged);
    CHECK(button == kCGMouseButtonLeft);

    CHECK(mouseMoveEventType(2, &button) == kCGEventRightMouseDragged);
    CHECK(button == kCGMouseButtonRight);

    CHECK(mouseMoveEventType(4, &button) == kCGEventOtherMouseDragged);
    CHECK(button == kCGMouseButtonCenter);

    // Left wins when several buttons are held.
    CHECK(mouseMoveEventType(1 | 2 | 4, &button) == kCGEventLeftMouseDragged);
    CHECK(button == kCGMouseButtonLeft);
}

static const uint32_t A = 0x00000800;
static const uint32_t X = 0x00000200;
static const uint32_t UNMAPPED = 0x00100000; // HOME
static const std::vector<ButtonKeyMapping> testMappings = {{A, 124}, {X, 126}};

static bool isEvent(const KeyEvent& e, CGKeyCode keyCode, bool isDown, bool isRepeat) {
    return e.keyCode == keyCode && e.isDown == isDown && e.isRepeat == isRepeat;
}

static void testKeyPressAndRelease() {
    // Control: a tap sends exactly one down then one up (passes with or without repeat support).
    KeyRepeater keys(testMappings, 0.4, 0.06);
    std::vector<KeyEvent> down = keys.update(A, 0.0);
    CHECK(down.size() == 1 && isEvent(down[0], 124, true, false));
    std::vector<KeyEvent> up = keys.update(0, 0.1);
    CHECK(up.size() == 1 && isEvent(up[0], 124, false, false));
    CHECK(keys.update(0, 0.2).empty());
}

static void testKeyRepeat() {
    KeyRepeater keys(testMappings, 0.4, 0.06);
    CHECK(keys.update(A, 0.0).size() == 1);

    // Held, before the delay: nothing.
    CHECK(keys.update(A, 0.39).empty());

    // At the delay: one repeat, then one per interval.
    std::vector<KeyEvent> first = keys.update(A, 0.40);
    CHECK(first.size() == 1 && isEvent(first[0], 124, true, true));
    CHECK(keys.update(A, 0.45).empty());
    std::vector<KeyEvent> second = keys.update(A, 0.46);
    CHECK(second.size() == 1 && isEvent(second[0], 124, true, true));

    // A long packet gap yields one repeat, not a burst.
    CHECK(keys.update(A, 5.0).size() == 1);

    // Release sends a plain key up and stops repeating; the positive control is the
    // repeat just above, proving the same repeater would otherwise have fired.
    std::vector<KeyEvent> up = keys.update(0, 5.1);
    CHECK(up.size() == 1 && isEvent(up[0], 124, false, false));
    CHECK(keys.update(0, 10.0).empty());
}

static void testIndependentKeys() {
    KeyRepeater keys(testMappings, 0.4, 0.06);
    CHECK(keys.update(A, 0.0).size() == 1);
    std::vector<KeyEvent> both = keys.update(A | X, 0.2);
    CHECK(both.size() == 1 && isEvent(both[0], 126, true, false));

    // At 0.4 only A has been held long enough to repeat.
    std::vector<KeyEvent> onlyA = keys.update(A | X, 0.4);
    CHECK(onlyA.size() == 1 && isEvent(onlyA[0], 124, true, true));
    // Just past X's own delay (0.2 + 0.4; not exactly 0.6 in floating point) both repeat.
    std::vector<KeyEvent> bothRepeat = keys.update(A | X, 0.61);
    CHECK(bothRepeat.size() == 2);
    CHECK(bothRepeat.size() == 2 && isEvent(bothRepeat[0], 124, true, true) && isEvent(bothRepeat[1], 126, true, true));
}

static void testUnmappedButtonsIgnored() {
    KeyRepeater keys(testMappings, 0.4, 0.06);
    CHECK(keys.update(UNMAPPED, 0.0).empty());
    CHECK(keys.update(UNMAPPED, 1.0).empty());
    // Positive control: a mapped button on the same repeater still produces events.
    CHECK(keys.update(UNMAPPED | A, 1.1).size() == 1);
}

static void testRepeatDisableSentinels() {
    const double disabled[][2] = {
        {0.4, 0.0},          // interval 0 must disable, not repeat every packet
        {0.4, -1.0},         // negative interval
        {-1.0, 0.06},        // negative delay
        {0.4, NAN},          // malformed interval
        {NAN, 0.06},         // malformed delay
    };
    for (const auto& knobs : disabled) {
        KeyRepeater keys(testMappings, knobs[0], knobs[1]);
        CHECK(keys.update(A, 0.0).size() == 1);
        size_t repeats = 0;
        for (double t = 0.03; t < 3.0; t += 0.03) {
            repeats += keys.update(A, t).size();
        }
        CHECK(repeats == 0);
        // Press and release still work with repeat off.
        std::vector<KeyEvent> up = keys.update(0, 3.0);
        CHECK(up.size() == 1 && isEvent(up[0], 124, false, false));
    }

    // Positive control: the same loop with valid knobs does repeat.
    KeyRepeater keys(testMappings, 0.4, 0.06);
    keys.update(A, 0.0);
    size_t repeats = 0;
    for (double t = 0.03; t < 3.0; t += 0.03) {
        repeats += keys.update(A, t).size();
    }
    CHECK(repeats > 0);

    // Delay 0 is valid: repeat starts on the next packet.
    KeyRepeater immediate(testMappings, 0.0, 0.06);
    immediate.update(A, 0.0);
    CHECK(immediate.update(A, 0.03).size() == 1);
}

static void testDefaultMappings() {
    const std::vector<ButtonKeyMapping>& mappings = defaultButtonKeyMappings();
    auto keyFor = [&](uint32_t mask) -> int {
        for (const ButtonKeyMapping& m : mappings) {
            if (m.buttonMask == mask) return m.keyCode;
        }
        return -1;
    };
    CHECK(keyFor(0x00040000) == 36);  // RS -> Return
    CHECK(keyFor(0x00000200) == 126); // X -> Up
    CHECK(keyFor(0x00000400) == 125); // B -> Down
    CHECK(keyFor(0x00000100) == 123); // Y -> Left
    CHECK(keyFor(0x00000800) == 124); // A -> Right
    // R and ZR stay mouse buttons.
    CHECK(keyFor(0x00004000) == -1);
    CHECK(keyFor(0x00008000) == -1);
}

int main() {
    testClampToDisplays();
    testMouseMoveEventType();
    testKeyPressAndRelease();
    testKeyRepeat();
    testIndependentKeys();
    testUnmappedButtonsIgnored();
    testRepeatDisableSentinels();
    testDefaultMappings();

    return checkSummary("input_mapping");
}
