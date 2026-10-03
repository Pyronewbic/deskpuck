#pragma once

#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

// Smallest input report that contains every field below (triggers end at 0x3D).
constexpr size_t kJoycon2ReportMinSize = 0x3E;

struct Joycon2Report {
    uint32_t packetId;
    uint32_t buttons;
    uint16_t leftStickX, leftStickY;
    uint16_t rightStickX, rightStickY;
    int16_t mouseX, mouseY, mouseUnknown, mouseDistance;
    int16_t magX, magY, magZ;
    uint16_t batteryVoltageRaw;
    int16_t batteryCurrentRaw;
    int16_t temperatureRaw;
    int16_t accelX, accelY, accelZ;
    int16_t gyroX, gyroY, gyroZ;
    uint8_t triggerL, triggerR;

    float batteryVoltage() const { return batteryVoltageRaw / 1000.0f; }
    float batteryCurrent() const { return batteryCurrentRaw / 100.0f; }
    float temperature() const { return 25.0f + temperatureRaw / 127.0f; }
};

// Little-endian reads; callers guarantee offset + width <= size.
int16_t readInt16LE(const uint8_t* data, size_t offset);
uint16_t readUint16LE(const uint8_t* data, size_t offset);
uint32_t readUint24LE(const uint8_t* data, size_t offset);
uint32_t readUint32LE(const uint8_t* data, size_t offset);

// Two 12-bit values packed into 3 bytes: X in the low 12 bits, Y in the high 12.
void parseStick(const uint8_t* data, size_t offset, uint16_t* x, uint16_t* y);

// Returns false (leaving *report untouched) if the report is too short.
bool parseJoycon2Report(const uint8_t* data, size_t size, Joycon2Report* report);

std::vector<std::string> joycon2ButtonNames(uint32_t buttons);
// Single-button lookups by the names above; false if unknown.
bool joycon2ButtonMask(const std::string& name, uint32_t* mask);
bool joycon2ButtonName(uint32_t mask, std::string* name);
