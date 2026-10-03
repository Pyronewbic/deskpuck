#include "Joycon2Packet.h"

#include "check.h"

#include <cstdio>
#include <cstring>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>

static std::string fixed(double value, int decimals) {
    char buf[32];
    std::snprintf(buf, sizeof(buf), "%.*f", decimals, value);
    return buf;
}

// Every captured report must decode to what the original upstream parser printed.
static void testCapturedReports() {
    std::ifstream file("tests/fixtures/joycon2_r_capture.txt");
    CHECK(file.good());

    int records = 0;
    std::string line;
    while (std::getline(file, line)) {
        if (line.empty() || line[0] == '#') continue;
        size_t bar = line.find(" | ");
        CHECK(bar != std::string::npos);
        if (bar == std::string::npos) continue;

        std::string hex = line.substr(0, bar);
        std::vector<uint8_t> bytes;
        for (size_t i = 0; i + 1 < hex.size(); i += 2) {
            bytes.push_back((uint8_t)std::stoul(hex.substr(i, 2), nullptr, 16));
        }
        CHECK(bytes.size() == 63);

        std::istringstream expected(line.substr(bar + 3));
        long packetId, trigL, trigR, lx, ly, rx, ry, ax, ay, az, gx, gy, gz, mgx, mgy, mgz, mouseX, mouseY;
        std::string buttonsHex, volts, amps, temp;
        expected >> packetId >> buttonsHex >> trigL >> trigR >> lx >> ly >> rx >> ry
                 >> ax >> ay >> az >> gx >> gy >> gz >> mgx >> mgy >> mgz >> mouseX >> mouseY
                 >> volts >> amps >> temp;
        CHECK(!expected.fail());

        Joycon2Report r;
        CHECK(parseJoycon2Report(bytes.data(), bytes.size(), &r));
        CHECK(r.packetId == (uint32_t)packetId);
        CHECK(r.buttons == std::stoul(buttonsHex, nullptr, 16));
        CHECK(r.triggerL == trigL && r.triggerR == trigR);
        CHECK(r.leftStickX == lx && r.leftStickY == ly);
        CHECK(r.rightStickX == rx && r.rightStickY == ry);
        CHECK(r.accelX == ax && r.accelY == ay && r.accelZ == az);
        CHECK(r.gyroX == gx && r.gyroY == gy && r.gyroZ == gz);
        CHECK(r.magX == mgx && r.magY == mgy && r.magZ == mgz);
        CHECK(r.mouseX == mouseX && r.mouseY == mouseY);
        CHECK(fixed(r.batteryVoltage(), 2) == volts);
        CHECK(fixed(r.batteryCurrent(), 2) == amps);
        CHECK(fixed(r.temperature(), 1) == temp);
        records++;
    }
    // Positive control: the fixture was actually read, including live IMU reports.
    CHECK(records == 18);
}

static void testLittleEndianReads() {
    const uint8_t minInt[] = {0x00, 0x80};
    const uint8_t maxInt[] = {0xFF, 0x7F};
    const uint8_t minusOne[] = {0xFF, 0xFF};
    CHECK(readInt16LE(minInt, 0) == -32768);
    CHECK(readInt16LE(maxInt, 0) == 32767);
    CHECK(readInt16LE(minusOne, 0) == -1);
    CHECK(readUint16LE(minusOne, 0) == 0xFFFF);

    const uint8_t word[] = {0x01, 0x02, 0x03, 0x04};
    CHECK(readUint16LE(word, 1) == 0x0302);
    CHECK(readUint24LE(word, 0) == 0x030201);
    CHECK(readUint32LE(word, 0) == 0x04030201);

    const uint8_t highBit[] = {0x00, 0x00, 0x00, 0x80};
    CHECK(readUint32LE(highBit, 0) == 0x80000000u);
}

static void testStickNibbles() {
    uint16_t x, y;
    const uint8_t xMax[] = {0xFF, 0x0F, 0x00};
    parseStick(xMax, 0, &x, &y);
    CHECK(x == 0xFFF && y == 0);

    const uint8_t yMax[] = {0x00, 0xF0, 0xFF};
    parseStick(yMax, 0, &x, &y);
    CHECK(x == 0 && y == 0xFFF);

    // The middle byte is split: its low nibble belongs to X, its high nibble to Y.
    const uint8_t mixed[] = {0x21, 0x43, 0x65};
    parseStick(mixed, 0, &x, &y);
    CHECK(x == 0x321 && y == 0x654);
}

static void testSyntheticFields() {
    std::vector<uint8_t> bytes(63, 0);
    // Buttons at 0x03: ZL plus noise in the low byte. Must survive exactly (no float rounding).
    bytes[0x03] = 0xFF; bytes[0x06] = 0x80;
    // Mouse X = -2, Y = 300, distance = 7.
    bytes[0x10] = 0xFE; bytes[0x11] = 0xFF;
    bytes[0x12] = 0x2C; bytes[0x13] = 0x01;
    bytes[0x16] = 0x07;
    bytes[0x3C] = 0x11; bytes[0x3D] = 0x22;

    Joycon2Report r;
    CHECK(parseJoycon2Report(bytes.data(), bytes.size(), &r));
    CHECK(r.buttons == 0x800000FFu);
    CHECK(r.mouseX == -2 && r.mouseY == 300 && r.mouseDistance == 7);
    CHECK(r.triggerL == 0x11 && r.triggerR == 0x22);
}

static void testShortReportsRejected() {
    std::vector<uint8_t> sizes = {0, 1, 59};
    for (uint8_t size : sizes) {
        std::vector<uint8_t> bytes(size, 0xAB);
        Joycon2Report r;
        std::memset(&r, 0x5A, sizeof(r));
        CHECK(!parseJoycon2Report(bytes.data(), bytes.size(), &r));
        CHECK(r.packetId == 0x5A5A5A5Au);
    }
}

static void testButtonNames() {
    CHECK(joycon2ButtonNames(0).empty());
    CHECK(joycon2ButtonNames(0x000000FF).empty());

    std::vector<std::string> a = joycon2ButtonNames(0x00000800);
    CHECK(a.size() == 1 && a[0] == "A");

    std::vector<std::string> xAndStick = joycon2ButtonNames(0x00040200);
    CHECK(xAndStick.size() == 2 && xAndStick[0] == "X" && xAndStick[1] == "RS");

    CHECK(joycon2ButtonNames(0xFFFFFFFF).size() == 23);
}

int main() {
    testCapturedReports();
    testLittleEndianReads();
    testStickNibbles();
    testSyntheticFields();
    testShortReportsRejected();
    testButtonNames();
    return checkSummary("packet");
}
