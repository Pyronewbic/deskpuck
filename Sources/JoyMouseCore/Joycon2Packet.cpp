#include "Joycon2Packet.h"

int16_t readInt16LE(const uint8_t* data, size_t offset) {
    return (int16_t)readUint16LE(data, offset);
}

uint16_t readUint16LE(const uint8_t* data, size_t offset) {
    return (uint16_t)(data[offset] | (data[offset + 1] << 8));
}

uint32_t readUint24LE(const uint8_t* data, size_t offset) {
    return (uint32_t)data[offset] | ((uint32_t)data[offset + 1] << 8) | ((uint32_t)data[offset + 2] << 16);
}

uint32_t readUint32LE(const uint8_t* data, size_t offset) {
    return readUint24LE(data, offset) | ((uint32_t)data[offset + 3] << 24);
}

void parseStick(const uint8_t* data, size_t offset, uint16_t* x, uint16_t* y) {
    uint32_t packed = readUint24LE(data, offset);
    *x = packed & 0xFFF;
    *y = (packed >> 12) & 0xFFF;
}

bool parseJoycon2Report(const uint8_t* data, size_t size, Joycon2Report* report) {
    if (size < kJoycon2ReportMinSize) {
        return false;
    }
    Joycon2Report r;
    r.packetId = readUint24LE(data, 0x00);
    r.buttons = readUint32LE(data, 0x03);
    parseStick(data, 0x0A, &r.leftStickX, &r.leftStickY);
    parseStick(data, 0x0D, &r.rightStickX, &r.rightStickY);
    r.mouseX = readInt16LE(data, 0x10);
    r.mouseY = readInt16LE(data, 0x12);
    r.mouseUnknown = readInt16LE(data, 0x14);
    r.mouseDistance = readInt16LE(data, 0x16);
    r.magX = readInt16LE(data, 0x18);
    r.magY = readInt16LE(data, 0x1A);
    r.magZ = readInt16LE(data, 0x1C);
    r.batteryVoltageRaw = readUint16LE(data, 0x1F);
    r.batteryCurrentRaw = readInt16LE(data, 0x28);
    r.temperatureRaw = readInt16LE(data, 0x2E);
    r.accelX = readInt16LE(data, 0x30);
    r.accelY = readInt16LE(data, 0x32);
    r.accelZ = readInt16LE(data, 0x34);
    r.gyroX = readInt16LE(data, 0x36);
    r.gyroY = readInt16LE(data, 0x38);
    r.gyroZ = readInt16LE(data, 0x3A);
    r.triggerL = data[0x3C];
    r.triggerR = data[0x3D];
    *report = r;
    return true;
}

namespace {
const struct { uint32_t mask; const char* name; } kButtons[] = {
    {0x00000100, "Y"}, {0x00000200, "X"}, {0x00000400, "B"}, {0x00000800, "A"},
    {0x00001000, "SR"}, {0x00002000, "SL"}, {0x00004000, "R"}, {0x00008000, "ZR"},
    {0x00010000, "MINUS"}, {0x00020000, "PLUS"}, {0x00040000, "RS"}, {0x00080000, "LS"},
    {0x00100000, "HOME"}, {0x00200000, "CAPTURE"}, {0x00400000, "CHAT"},
    {0x01000000, "DOWN"}, {0x02000000, "UP"}, {0x04000000, "RIGHT"}, {0x08000000, "LEFT"},
    {0x10000000, "SR_L"}, {0x20000000, "SL_L"}, {0x40000000, "L"}, {0x80000000, "ZL"},
};
}

std::vector<std::string> joycon2ButtonNames(uint32_t buttons) {
    std::vector<std::string> names;
    for (const auto& button : kButtons) {
        if (buttons & button.mask) {
            names.push_back(button.name);
        }
    }
    return names;
}

bool joycon2ButtonMask(const std::string& name, uint32_t* mask) {
    for (const auto& button : kButtons) {
        if (name == button.name) {
            *mask = button.mask;
            return true;
        }
    }
    return false;
}

bool joycon2ButtonName(uint32_t mask, std::string* name) {
    for (const auto& button : kButtons) {
        if (mask == button.mask) {
            *name = button.name;
            return true;
        }
    }
    return false;
}
