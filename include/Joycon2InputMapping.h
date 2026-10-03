#pragma once

#include <CoreGraphics/CoreGraphics.h>
#include <functional>

// Bounds of the display containing a point; false if no display contains it.
typedef std::function<bool(CGPoint point, CGRect* bounds)> DisplayLookup;

DisplayLookup systemDisplayLookup();

// The cursor may cross onto any display; off all displays it is clamped to the
// display it came from, stopping on the last pixel so edge triggers (Dock) fire.
CGPoint clampToDisplays(CGPoint target, CGPoint from, const DisplayLookup& displayAt, CGRect fallbackBounds);

// Mouse button bits: 1 left, 2 right, 4 middle. Held buttons turn a move into a drag.
CGEventType mouseMoveEventType(uint8_t mouseBtnState, CGMouseButton* button);
