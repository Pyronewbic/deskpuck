#include "Joycon2InputMapping.h"

#include <cstdio>
#include <vector>

static int checks = 0;
static int failures = 0;

#define CHECK(cond) do { \
    checks++; \
    if (!(cond)) { failures++; std::printf("FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond); } \
} while (0)

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

int main() {
    testClampToDisplays();
    testMouseMoveEventType();

    std::printf("%d checks, %d failures\n", checks, failures);
    if (checks == 0) {
        std::printf("FAIL: no checks ran\n");
        return 1;
    }
    return failures == 0 ? 0 : 1;
}
