#include "Joycon2Engine.h"

#include "check.h"

#include <cstring>

static Joycon2Report report(uint32_t buttons = 0, int16_t mouseX = 0, int16_t mouseY = 0,
                            uint16_t leftStickY = 2047, uint16_t rightStickY = 2047) {
    Joycon2Report r;
    std::memset(&r, 0, sizeof(r));
    r.buttons = buttons;
    r.mouseX = mouseX;
    r.mouseY = mouseY;
    r.leftStickX = r.rightStickX = 2047;
    r.leftStickY = leftStickY;
    r.rightStickY = rightStickY;
    return r;
}

static const uint32_t R = 0x00004000, ZR = 0x00008000, RS = 0x00040000, LS = 0x00080000;
static const uint32_t L = 0x40000000, ZL = 0x80000000;

static void testMouseButtonMapping() {
    CHECK(mouseButtonsForJoyconButtons(0) == 0);
    CHECK(mouseButtonsForJoyconButtons(R) == 1);
    CHECK(mouseButtonsForJoyconButtons(ZL) == 1);
    CHECK(mouseButtonsForJoyconButtons(ZR) == 2);
    CHECK(mouseButtonsForJoyconButtons(L) == 2);
    CHECK(mouseButtonsForJoyconButtons(LS) == 4);
    CHECK(mouseButtonsForJoyconButtons(R | ZR | LS) == 7);
    // The right stick click is a key (Return), not a mouse button.
    CHECK(mouseButtonsForJoyconButtons(RS) == 0);
}

static void testWheelLevels() {
    CHECK(wheelForStickDeviation(0) == 0);
    CHECK(wheelForStickDeviation(30) == 0);
    CHECK(wheelForStickDeviation(-30) == 0);
    // Past the deadzone but under one 60-unit level: still zero.
    CHECK(wheelForStickDeviation(59) == 0);
    CHECK(wheelForStickDeviation(60) == -5);
    CHECK(wheelForStickDeviation(-60) == 5);
    CHECK(wheelForStickDeviation(1200) == -100);
    CHECK(wheelForStickDeviation(4000) == -100);
}

static void testScrollCentresOnFirstReport() {
    InputEngine engine;
    // The first report defines the resting position, whatever it is.
    CHECK(engine.process(report(0, 0, 0, 2047, 2100), 0.0).wheel == 0);
    CHECK(engine.process(report(0, 0, 0, 2047, 2220), 0.1).wheel == -10);
    CHECK(engine.process(report(0, 0, 0, 2047, 1980), 0.2).wheel == 10);

    // A new connection recentres on its first report.
    engine.connectionStarted();
    CHECK(engine.process(report(0, 0, 0, 2047, 2300), 0.3).wheel == 0);
    CHECK(engine.process(report(0, 0, 0, 2047, 2300), 0.4).wheel == 0);
}

static void testWheelClamped() {
    InputEngine engine;
    engine.process(report(0, 0, 0, 2047, 2047), 0.0);
    // Both sticks at full deflection would sum to 200.
    CHECK(engine.process(report(0, 0, 0, 747, 747), 0.1).wheel == 127);
    CHECK(engine.process(report(0, 0, 0, 3347, 3347), 0.2).wheel == -127);
}

static void testPointerDeltas() {
    InputEngine engine;
    engine.process(report(0, 100, -40), 0.0);
    EngineOutput out = engine.process(report(0, 150, -60), 0.1);
    CHECK(out.dx == 10.0 && out.dy == -4.0);

    // Counters are 16-bit and wrap: 32760 -> -32766 is a step of +10.
    engine.process(report(0, 32760, 0), 0.2);
    out = engine.process(report(0, -32766, 0), 0.3);
    CHECK(out.dx == 2.0 && out.dy == 0.0);

    // No sensor movement, no pointer movement.
    out = engine.process(report(0, -32766, 0), 0.4);
    CHECK(out.dx == 0.0 && out.dy == 0.0);
}

static void testPointerSpeed() {
    EngineSettings settings;
    settings.pointerSpeed = 2.0;
    InputEngine engine(settings);
    engine.process(report(0, 0, 0), 0.0);
    CHECK(engine.process(report(0, 50, 25), 0.1).dx == 20.0);
}

static void testKeysPassThrough() {
    InputEngine engine;
    EngineOutput out = engine.process(report(RS), 0.0);
    CHECK(out.keys.size() == 1 && out.keys[0].keyCode == 36 && out.keys[0].isDown);
    CHECK(out.mouseButtons == 0);
}

static void testButtonBitsExact() {
    // ZL plus noise in the unmapped low byte. Rounding through float would turn
    // 0x800000FF into 0x80000100 and press Y (Left arrow).
    InputEngine engine;
    EngineOutput out = engine.process(report(0x800000FF), 0.0);
    CHECK(out.keys.empty());
    CHECK(out.mouseButtons == 1);
}

static void testNoJumpOnConnect() {
    // The sensor counter is absolute; its first value says nothing about motion.
    InputEngine engine;
    EngineOutput out = engine.process(report(0, 12000, -9000), 0.0);
    CHECK(out.dx == 0.0 && out.dy == 0.0);

    // Same after a reconnect, where the counter may have moved while disconnected.
    engine.connectionStarted();
    out = engine.process(report(0, 5000, 300), 0.1);
    CHECK(out.dx == 0.0 && out.dy == 0.0);

    // Positive control: the next report moves from the new baseline.
    out = engine.process(report(0, 5050, 300), 0.2);
    CHECK(out.dx == 10.0 && out.dy == 0.0);
}

static void testDisconnectReleasesEverything() {
    InputEngine engine;
    EngineOutput held = engine.process(report(R | RS), 0.0);
    CHECK(held.mouseButtons == 1 && held.keys.size() == 1);

    EngineOutput out = engine.disconnected();
    CHECK(out.mouseButtons == 0);
    CHECK(out.keys.size() == 1 && out.keys[0].keyCode == 36 && !out.keys[0].isDown);
    CHECK(out.dx == 0.0 && out.dy == 0.0 && out.wheel == 0);
}

static void testNoScrollWhileStickClicked() {
    InputEngine engine;
    engine.process(report(0, 0, 0, 2047, 2047), 0.0);
    // Pressing the right stick in tilts it slightly; that must not scroll.
    CHECK(engine.process(report(RS, 0, 0, 2047, 2167), 0.1).wheel == 0);
    // Same for the left stick on a left Joy-Con.
    CHECK(engine.process(report(LS, 0, 0, 2167, 2047), 0.2).wheel == 0);
    // Positive control: the same tilts scroll once the stick is released.
    CHECK(engine.process(report(0, 0, 0, 2047, 2167), 0.3).wheel == -10);
    CHECK(engine.process(report(0, 0, 0, 2167, 2047), 0.4).wheel == -10);
    // Clicking one stick does not silence the other.
    CHECK(engine.process(report(RS, 0, 0, 2167, 2167), 0.5).wheel == -10);
}

int main() {
    testMouseButtonMapping();
    testWheelLevels();
    testScrollCentresOnFirstReport();
    testWheelClamped();
    testPointerDeltas();
    testPointerSpeed();
    testKeysPassThrough();
    testButtonBitsExact();
    testNoJumpOnConnect();
    testDisconnectReleasesEverything();
    testNoScrollWhileStickClicked();
    return checkSummary("engine");
}
