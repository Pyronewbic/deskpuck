#pragma once

#include "Joycon2InputMapping.h"
#include "Joycon2Packet.h"

#include <vector>

// Raw optical-sensor counts per screen point at pointerSpeed 1.0.
constexpr double kMouseCountsPerPoint = 5.0;

struct EngineSettings {
    std::vector<ButtonKeyMapping> keyMappings = defaultButtonKeyMappings();
    double pointerSpeed = 1.0;
    double repeatDelay = 0.4;
    double repeatInterval = 0.06;
};

// Mouse button bits as used by mouseMoveEventType: 1 left, 2 right, 4 middle.
struct EngineOutput {
    double dx = 0, dy = 0;
    uint8_t mouseButtons = 0;
    int wheel = 0;
    std::vector<KeyEvent> keys;
};

// Turns Joy-Con reports into pointer, click, scroll and key output. No OS calls.
class InputEngine {
public:
    explicit InputEngine(const EngineSettings& settings = EngineSettings());

    // Call when a controller connects; the next report recentres the scroll stick
    // and becomes the baseline for pointer motion.
    void connectionStarted();
    EngineOutput process(const Joycon2Report& report, double now);
    // Releases every held mouse button and key.
    EngineOutput disconnected();

private:
    EngineSettings settings_;
    KeyRepeater keys_;
    int16_t lastMouseX_ = 0;
    int16_t lastMouseY_ = 0;
    bool haveMousePosition_ = false;
    bool stickCentred_ = false;
    int leftStickCentreY_ = 2047;
    int rightStickCentreY_ = 2047;
};

uint8_t mouseButtonsForJoyconButtons(uint32_t buttons);
int wheelForStickDeviation(int deviation);
