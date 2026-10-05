#include "Joycon2Engine.h"

#include <cstdlib>

static const uint32_t kButtonR = 0x00004000;
static const uint32_t kButtonZR = 0x00008000;
static const uint32_t kButtonRS = 0x00040000;
static const uint32_t kButtonLS = 0x00080000;
static const uint32_t kButtonL = 0x40000000;
static const uint32_t kButtonZL = 0x80000000;

uint8_t mouseButtonsForJoyconButtons(uint32_t buttons) {
    uint8_t mouse = 0;
    if (buttons & (kButtonR | kButtonZL)) mouse |= 1;
    if (buttons & (kButtonZR | kButtonL)) mouse |= 2;
    if (buttons & kButtonLS) mouse |= 4;
    return mouse;
}

// 20 speed levels of 60 stick units each; the first 60 units are the deadzone.
// Stick up scrolls up.
int wheelForStickDeviation(int deviation) {
    int level = std::abs(deviation) / 60;
    if (level > 20) level = 20;
    int speed = level * 5;
    return deviation > 0 ? -speed : speed;
}

InputEngine::InputEngine(const EngineSettings& settings)
    : settings_(settings),
      keys_(settings.keyMappings, settings.repeatDelay, settings.repeatInterval) {}

void InputEngine::connectionStarted() {
    stickCentred_ = false;
    haveMousePosition_ = false;
}

EngineOutput InputEngine::process(const Joycon2Report& report, double now) {
    EngineOutput out;

    if (!haveMousePosition_) {
        lastMouseX_ = report.mouseX;
        lastMouseY_ = report.mouseY;
        haveMousePosition_ = true;
    }
    int16_t rawDx = (int16_t)(report.mouseX - lastMouseX_);
    int16_t rawDy = (int16_t)(report.mouseY - lastMouseY_);
    lastMouseX_ = report.mouseX;
    lastMouseY_ = report.mouseY;
    out.dx = rawDx * settings_.pointerSpeed / kMouseCountsPerPoint;
    out.dy = rawDy * settings_.pointerSpeed / kMouseCountsPerPoint;

    out.mouseButtons = mouseButtonsForJoyconButtons(report.buttons);

    if (!stickCentred_) {
        leftStickCentreY_ = report.leftStickY;
        rightStickCentreY_ = report.rightStickY;
        stickCentred_ = true;
    }
    if (settings_.scrollEnabled) {
        // A stick being clicked also tilts a little; don't let that scroll.
        int wheel = 0;
        if (!(report.buttons & kButtonLS)) {
            wheel += wheelForStickDeviation(report.leftStickY - leftStickCentreY_);
        }
        if (!(report.buttons & kButtonRS)) {
            wheel += wheelForStickDeviation(report.rightStickY - rightStickCentreY_);
        }
        if (wheel > 127) wheel = 127;
        if (wheel < -127) wheel = -127;
        out.wheel = wheel;
    }

    out.keys = keys_.update(report.buttons, now);
    return out;
}

std::vector<KeyEvent> InputEngine::applySettings(const EngineSettings& settings) {
    std::vector<KeyEvent> released = keys_.releaseAll();
    settings_ = settings;
    keys_ = KeyRepeater(settings.keyMappings, settings.repeatDelay, settings.repeatInterval);
    return released;
}

EngineOutput InputEngine::disconnected() {
    EngineOutput out;
    out.keys = keys_.releaseAll();
    return out;
}
