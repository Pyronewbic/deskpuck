#import "../include/Joycon2BLEReceiver.h"
#include "Joycon2Packet.h"
#import <CoreBluetooth/CoreBluetooth.h>
#import <Foundation/Foundation.h>
#include <vector>
#include <map>
#include <string>
#include <iostream>
#include <iomanip>
#include <chrono>
#include <sstream>

// Constants
const uint16_t JOYCON2_MANUFACTURER_ID = 0x0553; // Joy-Con manufacturer ID
NSString* const WRITE_CHARACTERISTIC_UUID = @"649D4AC9-8EB7-4E6C-AF44-1EA54FE5F005";
NSString* const SUBSCRIBE_CHARACTERISTIC_UUID = @"AB7DE9BE-89FE-49AD-828F-118F09DF7FD2";

// Global data counter
int dataReceiveCounter = 0;



std::chrono::time_point<std::chrono::system_clock> connectionStartTime;

@implementation Joycon2BLEReceiver

- (instancetype)init {
    self = [super init];
    if (self) {
        self.centralManager = [[CBCentralManager alloc] initWithDelegate:self queue:nil];
        self.connectingPeripherals = [[NSMutableSet alloc] init];
        self.connectedPeripherals = [[NSMutableSet alloc] init];
        self.deviceType = @"Unknown";

        self.dataTimeoutTimer = nil;

        self.commandTimer = nil;

        if (!sharedInstance) {
            sharedInstance = self;
        }
    }
    return self;
}

- (void)startScan {
    self.shouldScan = YES;
    if (self.centralManager.state == CBManagerStatePoweredOn) {
        [self.centralManager scanForPeripheralsWithServices:nil options:nil];
        log("SECTION", "------ Scanning BLE devices ------");
    } else {
        log("INFO", "Waiting for Bluetooth to be ready...");
    }
}

- (void)stopScan {
    [self.centralManager stopScan];
    std::cout << "Scan stopped." << std::endl;
}

- (void)connectToDevice:(NSString*)address {
    // Find peripheral by address and connect
    NSArray* peripherals = [self.centralManager retrieveConnectedPeripheralsWithServices:@[]];
    for (CBPeripheral* peripheral in peripherals) {
        if ([peripheral.identifier.UUIDString isEqualToString:address]) {
            self.connectedPeripheral = peripheral;
            self.connectedPeripheral.delegate = self;
            [self.centralManager connectPeripheral:self.connectedPeripheral options:nil];
            return;
        }
    }
    // If not connected, scan and connect
    [self startScan];
}

- (void)disconnect {
    if (self.connectedPeripheral) {
        [self.centralManager cancelPeripheralConnection:self.connectedPeripheral];
    }
}

// CBCentralManagerDelegate methods
- (void)centralManagerDidUpdateState:(CBCentralManager*)central {
    switch (central.state) {
        case CBManagerStatePoweredOn:
        std::cout << "Bluetooth is powered on." << std::endl;
        //Auto-start scanning if we were waiting for Bluetooth
        if (self.shouldScan) {
            [self startScan];
        }
        break;
        case CBManagerStatePoweredOff:
        std::cout << "Bluetooth is powered off." << std::endl;
        break;
        default:
        std::cout << "Bluetooth state changed." << std::endl;
        break;
    }
}

- (void)centralManager:(CBCentralManager*)central didDiscoverPeripheral:(CBPeripheral*)peripheral advertisementData:(NSDictionary*)advertisementData RSSI:(NSNumber*)RSSI {
    const char* deviceName = peripheral.name ? [peripheral.name UTF8String] : "Unknown";
    const char* deviceUUID = peripheral.identifier ? [peripheral.identifier.UUIDString UTF8String] : "Unknown";

    id manufacturerData = advertisementData[CBAdvertisementDataManufacturerDataKey];
    if (manufacturerData) {
        uint16_t companyId = 0;
        bool hasValidManufacturerId = false;

        if ([manufacturerData isKindOfClass:[NSDictionary class]]) {
            NSNumber* companyIdNumber = [[manufacturerData allKeys] firstObject];
            if (companyIdNumber) {
                companyId = [companyIdNumber unsignedShortValue];
                companyId = CFSwapInt16LittleToHost(companyId);
                hasValidManufacturerId = true;
            }
        } else if ([manufacturerData isKindOfClass:[NSData class]]) {
            NSData* data = (NSData*)manufacturerData;
            if (data.length >= 2) {
                [data getBytes:&companyId length:sizeof(uint16_t)];
                companyId = CFSwapInt16LittleToHost(companyId);
                hasValidManufacturerId = true;
            }
        }

        if (hasValidManufacturerId && companyId == JOYCON2_MANUFACTURER_ID) {
            log("INFO", "Joy-Con found: " + std::string(deviceName) + " (" + std::string(deviceUUID) + ") RSSI: " + std::to_string([RSSI intValue]));
            if (self.onDeviceFound) {
                self.onDeviceFound(peripheral.name, peripheral.identifier.UUIDString);
            }

            if (![self.connectingPeripherals containsObject:peripheral.identifier] && ![self.connectedPeripherals containsObject:peripheral.identifier]) {
                std::cout << "🔗 Attempting to connect to Joy-Con..." << std::endl;
                [self.connectingPeripherals addObject:peripheral.identifier];
                std::cout << "📊 Connection state updated - Connecting: " << [self.connectingPeripherals count]
                << ", Connected: " << [self.connectedPeripherals count] << std::endl;

                NSDictionary* connectOptions = @{
                    CBConnectPeripheralOptionNotifyOnConnectionKey: @YES,
                    CBConnectPeripheralOptionNotifyOnDisconnectionKey: @YES,
                    CBConnectPeripheralOptionNotifyOnNotificationKey: @YES,
                    CBConnectPeripheralOptionStartDelayKey: @0
                };
                [self.centralManager connectPeripheral:peripheral options:connectOptions];

                dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(60.0 * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
                    if ([self.connectingPeripherals containsObject:peripheral.identifier] && ![self.connectedPeripherals containsObject:peripheral.identifier]) {
                        std::cout << "⏰ Connection timeout for " << deviceName << std::endl;
                        [self.connectingPeripherals removeObject:peripheral.identifier];
                        std::cout << "📊 Connection state updated - Connecting: " << [self.connectingPeripherals count]
                        << ", Connected: " << [self.connectedPeripherals count] << std::endl;
                        [self.centralManager cancelPeripheralConnection:peripheral];
                    }
                });
            } else {
                std::cout << "ℹ️  Already connecting/connected to this Joy-Con" << std::endl;
            }
        }
    }
}

- (void)centralManager:(CBCentralManager*)central didConnectPeripheral:(CBPeripheral*)peripheral {
    log("SECTION", "------ Connection Established ------");
    std::string nameStr = peripheral.name ? [peripheral.name UTF8String] : "Unknown";
    log("SUCCESS", "Connected to: " + nameStr);
    log("INFO", "Discovering services and characteristics...");

    [self.connectingPeripherals removeObject:peripheral.identifier];
    [self.connectedPeripherals addObject:peripheral.identifier];

    std::cout << "📊 Connection state updated - Connecting: " << [self.connectingPeripherals count]
    << ", Connected: " << [self.connectedPeripherals count] << std::endl;

    self.connectedPeripheral = peripheral;
    self.connectedPeripheral.delegate = self;

    self.deviceType = [Joycon2BLEReceiver determineDeviceType:peripheral];
    std::cout << "🎮 Device type detected: " << [self.deviceType UTF8String] << std::endl;

    [self startDataTimeoutTimer];

    connectionStartTime = std::chrono::system_clock::now();

    std::cout << "ℹ️  Initialization will begin after discovery" << std::endl;

    [peripheral discoverServices:nil];
    
    if (self.onConnected) {
        self.onConnected();
    }
}

- (void)centralManager:(CBCentralManager*)central didFailToConnectPeripheral:(CBPeripheral*)peripheral error:(NSError*)error {
    std::cout << "❌ Failed to connect to " << [peripheral.name UTF8String] << ": " << [error.localizedDescription UTF8String] << std::endl;
    std::cout << "❌ Error code: " << [error code] << std::endl;
    std::cout << "❌ Error domain: " << [error.domain UTF8String] << std::endl;

    [self.connectingPeripherals removeObject:peripheral.identifier];
    std::cout << "📊 Connection state updated - Connecting: " << [self.connectingPeripherals count]
              << ", Connected: " << [self.connectedPeripherals count] << std::endl;

    std::cout << "🔄 Retrying connection in 2 seconds..." << std::endl;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(2.0 * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
        std::cout << "🔄 Retrying connection..." << std::endl;
        [self startScan];
    });

    if (self.onError) {
        self.onError(error.localizedDescription);
    }
}

- (void)centralManager:(CBCentralManager*)central didDisconnectPeripheral:(CBPeripheral*)peripheral error:(NSError*)error {
    if (error) {
        std::cout << "🔌 Disconnected from " << [peripheral.name UTF8String] << " with error: " << [error.localizedDescription UTF8String] << std::endl;
        std::cout << "❌ Error code: " << [error code] << std::endl;
    } else {
        std::cout << "🔌 Disconnected from " << [peripheral.name UTF8String] << " (no error)" << std::endl;
    }

    [self invalidateDataTimeoutTimer];

    [self invalidateCommandTimer];

    [self.connectedPeripherals removeObject:peripheral.identifier];
    [self.connectingPeripherals removeObject:peripheral.identifier];
    std::cout << "📊 Connection state updated - Connecting: " << [self.connectingPeripherals count]
              << ", Connected: " << [self.connectedPeripherals count] << std::endl;

    std::cout << "🔄 Attempting to reconnect in 3 seconds..." << std::endl;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(3.0 * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
        std::cout << "🔄 Reconnecting..." << std::endl;
        [self startScan];
    });
}

// CBPeripheralDelegate methods
- (void)peripheral:(CBPeripheral*)peripheral didDiscoverServices:(NSError*)error {
    if (error) {
        std::cout << "Error discovering services: " << [error.localizedDescription UTF8String] << std::endl;
        return;
    }

    std::cout << "Discovered " << [peripheral.services count] << " services" << std::endl;
    for (CBService* service in peripheral.services) {
        std::cout << "Service: " << [service.UUID.UUIDString UTF8String] << std::endl;
        [peripheral discoverCharacteristics:nil forService:service];
    }
}

- (void)peripheral:(CBPeripheral*)peripheral didDiscoverCharacteristicsForService:(CBService*)service error:(NSError*)error {
    if (error) {
        log("ERROR", "Error discovering characteristics: " + std::string([error.localizedDescription UTF8String]));
        return;
    }

    log("SECTION", "------ Service Discovery ------");
    log("INFO", "Discovered " + std::to_string([service.characteristics count]) + " characteristics for service " + std::string([service.UUID.UUIDString UTF8String]));
    for (CBCharacteristic* characteristic in service.characteristics) {
        std::cout << "  Characteristic: " << [characteristic.UUID.UUIDString UTF8String] << " (Properties: " << characteristic.properties << ")" << std::endl;
        if ([characteristic.UUID.UUIDString isEqualToString:WRITE_CHARACTERISTIC_UUID]) {
            std::cout << "    ✓ Found WRITE characteristic" << std::endl;
            self.writeCharacteristic = characteristic;
        } else if ([characteristic.UUID.UUIDString isEqualToString:SUBSCRIBE_CHARACTERISTIC_UUID]) {
            std::cout << "    ✓ Found SUBSCRIBE characteristic" << std::endl;
            self.subscribeCharacteristic = characteristic;
            std::cout << "    📡 Enabling notifications for data stream..." << std::endl;
            [peripheral setNotifyValue:YES forCharacteristic:characteristic];
        } else {
            if (characteristic.properties & CBCharacteristicPropertyWrite) {
                std::cout << "    💡 Found writable characteristic: " << [characteristic.UUID.UUIDString UTF8String] << std::endl;
                if (!self.writeCharacteristic) {
                    std::cout << "    🔧 Using this as WRITE characteristic" << std::endl;
                    self.writeCharacteristic = characteristic;
                }
            }
            if (characteristic.properties & CBCharacteristicPropertyNotify) {
                if ([characteristic.UUID.UUIDString isEqualToString:SUBSCRIBE_CHARACTERISTIC_UUID]) {
                    std::cout << "    📡 Found notifiable characteristic: " << [characteristic.UUID.UUIDString UTF8String] << std::endl;
                    std::cout << "    ✓ Found SUBSCRIBE characteristic" << std::endl;
                    self.subscribeCharacteristic = characteristic;
                }
            }
        }
    }

    if (self.writeCharacteristic && self.subscribeCharacteristic) {
        std::cout << "✓ All required characteristics found, preparing for notification..." << std::endl;

        std::cout << "skipInitCommands: " << (self.skipInitCommands ? "YES" : "NO") << std::endl;
        if (!self.skipInitCommands) {
            dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(0.5 * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
                std::cout << "🚀 Sending initialization commands after characteristics discovery..." << std::endl;
                [self sendInitializationCommandsOnce];
            });
        }

        // Give the peripheral 2 s after discovery before enabling notifications
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(2.0 * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
            std::cout << "📡 Enabling notifications for data stream..." << std::endl;
            [peripheral setNotifyValue:YES forCharacteristic:self.subscribeCharacteristic];
        });
     } else {
        std::cout << "Waiting for all characteristics... (WRITE: " << (self.writeCharacteristic ? "✓" : "✗") << ", SUBSCRIBE: " << (self.subscribeCharacteristic ? "✓" : "✗") << ")" << std::endl;
     }
}

- (void)peripheral:(CBPeripheral*)peripheral didUpdateValueForCharacteristic:(CBCharacteristic*)characteristic error:(NSError*)error {
    if (error) {
        std::cout << "Error receiving data from " << [characteristic.UUID.UUIDString UTF8String] << ": " << [error.localizedDescription UTF8String] << std::endl;
        return;
    }

    NSData* data = characteristic.value;

    if ([characteristic.UUID.UUIDString isEqualToString:SUBSCRIBE_CHARACTERISTIC_UUID]) {
        if (data.length > 0) {
            if (data.length < kJoycon2ReportMinSize) {
                std::cout << "⚠️  Received data packet too small (" << data.length << " bytes, expected >= " << kJoycon2ReportMinSize << ")" << std::endl;
                return;
            }

            dataReceiveCounter++;
            std::string nameStr = peripheral.name ? [peripheral.name UTF8String] : "Unknown";
            log("SECTION", "------ " + nameStr + " Data Packet #" + std::to_string(dataReceiveCounter) + " ------");
            log("INFO", "Received data packet #" + std::to_string(dataReceiveCounter) + " (" + std::to_string(data.length) + " bytes)");



            [self resetDataTimeoutTimer];

            try {
                std::vector<uint8_t> dataVector((uint8_t*)data.bytes, (uint8_t*)data.bytes + data.length);

                Joycon2Report report;
                if (!parseJoycon2Report(dataVector.data(), dataVector.size(), &report)) {
                    return;
                }

                [Joycon2BLEReceiver printReport:report data:dataVector];

                if (self.onReportReceived) {
                    self.onReportReceived(report);
                }
            } catch (const std::exception& e) {
                std::cout << "❌ Data parsing error: " << e.what() << std::endl;
            } catch (...) {
                std::cout << "❌ Unknown data parsing error" << std::endl;
            }
        } else {
            std::cout << "⚠️  Received empty data packet" << std::endl;
        }
    } else {
        // Data from other characteristics is ignored
        // if (data.length > 0) {
        //     std::cout << "📄 Received " << data.length << " bytes from " << [characteristic.UUID.UUIDString UTF8String] << std::endl;
        // }
    }
}

- (void)peripheral:(CBPeripheral*)peripheral didUpdateNotificationStateForCharacteristic:(CBCharacteristic*)characteristic error:(NSError*)error {
    if (error) {
        std::cout << "❌ Failed to enable notifications for " << [characteristic.UUID.UUIDString UTF8String] << ": " << [error.localizedDescription UTF8String] << std::endl;
        std::cout << "❌ Error code: " << [error code] << std::endl;
        std::cout << "❌ Error domain: " << [error.domain UTF8String] << std::endl;
    } else {
        if ([characteristic.UUID.UUIDString isEqualToString:SUBSCRIBE_CHARACTERISTIC_UUID]) {
            std::cout << "✅ Notifications enabled for characteristic: " << [characteristic.UUID.UUIDString UTF8String] << std::endl;
            std::cout << "🎯 Ready to receive Joy-Con data! Move the controller to see sensor data..." << std::endl;
        }
    }
}

- (void)peripheral:(CBPeripheral*)peripheral didWriteValueForCharacteristic:(CBCharacteristic*)characteristic error:(NSError*)error {
    if (error) {
        std::cout << "❌ Failed to write value to characteristic: " << [error.localizedDescription UTF8String] << std::endl;
    } else {
        std::cout << "✅ Successfully wrote value to characteristic: " << [characteristic.UUID.UUIDString UTF8String] << std::endl;
    }
}



- (void)sendInitializationCommandsOnce {
    std::cout << "🚀 sendInitializationCommandsOnce called" << std::endl;
    auto currentTime = std::chrono::system_clock::now();
    auto currentMs = std::chrono::duration_cast<std::chrono::milliseconds>(currentTime.time_since_epoch()).count();
    std::cout << "⏱️  Init commands sent at: " << currentMs << " ms" << std::endl;
    NSArray* commands = @[
        // Feature mask 0xFF: buttons, sticks, IMU, mouse, etc. (ndeadly commands.md, 0x0C)
        [NSData dataWithBytes:(uint8_t[]){0x0c, 0x91, 0x01, 0x02, 0x00, 0x04, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00} length:12],
        // Enable the features selected above
        [NSData dataWithBytes:(uint8_t[]){0x0c, 0x91, 0x01, 0x04, 0x00, 0x04, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00} length:12]
    ];
    for (int i = 0; i < commands.count; i++) {
        std::cout << "📤 Sending command " << (i + 1) << "/" << commands.count << " (length: " << [commands[i] length] << ")" << std::endl;

        const uint8_t* bytes = (const uint8_t*)[commands[i] bytes];
        std::cout << "   Command hex: ";
        for (NSUInteger j = 0; j < [commands[i] length]; j++) {
            std::cout << std::hex << std::uppercase << std::setfill('0') << std::setw(2) << (int)bytes[j];
            if (j < [commands[i] length] - 1) std::cout << " ";
        }
        std::cout << std::dec << std::endl;

        CBCharacteristicWriteType writeType = CBCharacteristicWriteWithoutResponse;
        [self.connectedPeripheral writeValue:commands[i] forCharacteristic:self.writeCharacteristic type:writeType];

        std::cout << "✅ Command " << (i + 1) << " sent" << std::endl;

        if (i < commands.count - 1) {
            [NSThread sleepForTimeInterval:0.5];
        }
    }
}

// Singleton instance
static Joycon2BLEReceiver* sharedInstance = nil;

+ (Joycon2BLEReceiver*)sharedInstance {
    return sharedInstance;
}

+ (NSString*)determineDeviceType:(CBPeripheral*)peripheral {
    if (!peripheral) {
        return @"Unknown";
    }

    NSString* deviceName = peripheral.name;
    if (deviceName) {
        if ([deviceName containsString:@"(L)"] || [deviceName containsString:@"Left"] || [deviceName containsString:@"Joy-Con2 (L)"]) {
            return @"L";
        } else if ([deviceName containsString:@"(R)"] || [deviceName containsString:@"Right"] || [deviceName containsString:@"Joy-Con2 (R)"]) {
            return @"R";
        } else if ([deviceName containsString:@"Pro Controller2"]) {
            return @"Pro";
        }
    }

    return @"Unknown";
}

static int16_t lastMouseX = 0;
static int16_t lastMouseY = 0;
static int dataCounter = 0;

+ (void)printReport:(const Joycon2Report&)report data:(const std::vector<uint8_t>&)data {
    dataCounter++;
    auto currentTime = std::chrono::system_clock::now();
    auto currentMs = std::chrono::duration_cast<std::chrono::milliseconds>(currentTime.time_since_epoch()).count();

    Joycon2BLEReceiver* client = [Joycon2BLEReceiver sharedInstance];
    if (client.displayInterval > 1 && (dataCounter % client.displayInterval) != 0) {
        return;
    }

    #ifdef DEBUG
        auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(currentTime - connectionStartTime).count();
        log("DATA", "Elapsed: " + std::to_string(elapsed) + " ms");

        std::stringstream hexStream;
        hexStream << std::hex << std::uppercase << std::setfill('0') << std::setw(2);
        for (size_t i = 0; i < data.size(); ++i) {
            hexStream << (int)(uint8_t)data[i];
            if (i < data.size() - 1) hexStream << " ";
        }
        log("DATA", "Packet_HEX: " + hexStream.str());

        log("DATA", "PacketID: " + std::to_string((int)report.packetId));

        uint32_t buttons = report.buttons;
        std::stringstream buttonHex;
        buttonHex << std::hex << std::uppercase << std::setfill('0') << std::setw(8) << buttons;
        log("DATA", "Buttons: 0x" + buttonHex.str());

        auto buttonNames = joycon2ButtonNames(buttons);
        std::string pressed = buttonNames.empty() ? "None" : "";
        for (size_t i = 0; i < buttonNames.size(); ++i) {
            pressed += buttonNames[i];
            if (i < buttonNames.size() - 1) pressed += ", ";
        }
        log("DATA", "Pressed: " + pressed);

        log("DATA", "Analog_Triggers: L=" + std::to_string((int)report.triggerL) + ", R=" + std::to_string((int)report.triggerR));

        log("DATA", "LeftStick: X=" + std::to_string((int)report.leftStickX) + ", Y=" + std::to_string((int)report.leftStickY));
        log("DATA", "RightStick: X=" + std::to_string((int)report.rightStickX) + ", Y=" + std::to_string((int)report.rightStickY));

        log("DATA", "Accel: X=" + std::to_string((int)report.accelX) + ", Y=" + std::to_string((int)report.accelY) + ", Z=" + std::to_string((int)report.accelZ));
        log("DATA", "Gyro: X=" + std::to_string((int)report.gyroX) + ", Y=" + std::to_string((int)report.gyroY) + ", Z=" + std::to_string((int)report.gyroZ));
        log("DATA", "Mag: X=" + std::to_string((int)report.magX) + ", Y=" + std::to_string((int)report.magY) + ", Z=" + std::to_string((int)report.magZ));

        int16_t currentMouseX = report.mouseX;
        int16_t currentMouseY = report.mouseY;
        int16_t deltaX = currentMouseX - lastMouseX;
        int16_t deltaY = currentMouseY - lastMouseY;
        log("DATA", "Mouse: X=" + std::to_string(currentMouseX) + ", Y=" + std::to_string(currentMouseY) + ", DeltaX=" + std::to_string(deltaX) + ", DeltaY=" + std::to_string(deltaY));

        lastMouseX = currentMouseX;
        lastMouseY = currentMouseY;

        std::stringstream battery;
        battery << std::fixed << std::setprecision(2) << report.batteryVoltage() << "V, " << report.batteryCurrent() << "mA";
        log("DATA", "Battery: " + battery.str());

        std::stringstream temp;
        temp << std::fixed << std::setprecision(1) << report.temperature() << "°C";
        log("DATA", "Temperature: " + temp.str());

        std::cout << std::flush;
    #else
        std::cout << "\033[2J\033[1;1H"; // clear screen, cursor home

        std::cout << "=================================================" << std::endl;
        Joycon2BLEReceiver* viewer = [Joycon2BLEReceiver sharedInstance];
        NSString* deviceName = viewer.connectedPeripheral.name;
        std::string nameStr = deviceName ? [deviceName UTF8String] : "Unknown Device";
        std::cout << nameStr << " Data:" << std::endl;
        std::cout << "=================================================" << std::endl;

        auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(currentTime - connectionStartTime).count();
        std::cout << "Elapsed: " << elapsed << " ms" << std::endl;

        std::stringstream hexStream;
        hexStream << std::hex << std::uppercase << std::setfill('0') << std::setw(2);
        for (size_t i = 0; i < data.size(); ++i) {
            hexStream << (int)(uint8_t)data[i];
            if (i < data.size() - 1) hexStream << " ";
        }
        std::cout << "Packet_HEX: " << hexStream.str() << std::endl;

        std::cout << "PacketID: " << (int)report.packetId << std::endl;

        uint32_t buttons = report.buttons;
        std::stringstream buttonHex;
        buttonHex << std::hex << std::uppercase << std::setfill('0') << std::setw(8) << buttons;
        std::cout << "Buttons: " << buttonHex.str() << std::endl;

        auto buttonNames = joycon2ButtonNames(buttons);
        std::string pressed = buttonNames.empty() ? "None" : "";
        for (size_t i = 0; i < buttonNames.size(); ++i) {
            pressed += buttonNames[i];
            if (i < buttonNames.size() - 1) pressed += ", ";
        }
        std::cout << "Pressed: " << pressed << std::endl;

        std::cout << "Analog_Triggers: L=" << (int)report.triggerL << ", R=" << (int)report.triggerR << std::endl;

        std::cout << "LeftStick: X=" << (int)report.leftStickX << ", Y=" << (int)report.leftStickY << std::endl;
        std::cout << "RightStick: X=" << (int)report.rightStickX << ", Y=" << (int)report.rightStickY << std::endl;

        std::cout << "Accel: X=" << (int)report.accelX << ", Y=" << (int)report.accelY << ", Z=" << (int)report.accelZ << std::endl;
        std::cout << "Gyro: X=" << (int)report.gyroX << ", Y=" << (int)report.gyroY << ", Z=" << (int)report.gyroZ << std::endl;
        std::cout << "Mag: X=" << (int)report.magX << ", Y=" << (int)report.magY << ", Z=" << (int)report.magZ << std::endl;

        int16_t currentMouseX = report.mouseX;
        int16_t currentMouseY = report.mouseY;
        int16_t deltaX = currentMouseX - lastMouseX;
        int16_t deltaY = currentMouseY - lastMouseY;
        std::cout << "Mouse: X=" << currentMouseX << ", Y=" << currentMouseY << ", DeltaX=" << deltaX << ", DeltaY=" << deltaY << std::endl;

        lastMouseX = currentMouseX;
        lastMouseY = currentMouseY;

        std::stringstream battery;
        battery << std::fixed << std::setprecision(2) << report.batteryVoltage() << "V, " << report.batteryCurrent() << "mA";
        std::cout << "Battery: " << battery.str() << std::endl;

        std::stringstream temp;
        temp << std::fixed << std::setprecision(1) << report.temperature() << "°C";
        std::cout << "Temperature: " << temp.str() << std::endl;

        std::cout << std::flush;
    #endif
}

- (void)startDataTimeoutTimer {
    [self invalidateDataTimeoutTimer];
    self.dataTimeoutTimer = [NSTimer scheduledTimerWithTimeInterval:30.0
                                                             target:self
                                                           selector:@selector(dataTimeoutFired:)
                                                           userInfo:nil
                                                            repeats:NO];
}

- (void)resetDataTimeoutTimer {
    if (self.dataTimeoutTimer) {
        [self.dataTimeoutTimer invalidate];
        self.dataTimeoutTimer = nil;
    }
    [self startDataTimeoutTimer];
}

- (void)invalidateDataTimeoutTimer {
    if (self.dataTimeoutTimer) {
        [self.dataTimeoutTimer invalidate];
        self.dataTimeoutTimer = nil;
        std::cout << "⏰ Data timeout timer invalidated" << std::endl;
    }
}

- (void)invalidateCommandTimer {
 if (self.commandTimer) {
        [self.commandTimer invalidate];
        self.commandTimer = nil;
        std::cout << "⏰ Command timer invalidated" << std::endl;
    }
}



- (void)dataTimeoutFired:(NSTimer*)timer {
    auto currentTime = std::chrono::system_clock::now();
    auto currentMs = std::chrono::duration_cast<std::chrono::milliseconds>(currentTime.time_since_epoch()).count();

    std::cout << "⏰ Data timeout fired! No data received for 30 seconds." << std::endl;
    std::cout << "🔍 Checking connection status..." << std::endl;

    if (self.connectedPeripheral) {
        std::cout << "📡 Connected peripheral: " << [self.connectedPeripheral.name UTF8String] << std::endl;
        std::cout << "🔌 Connection state: " << self.connectedPeripheral.state << std::endl;
    } else {
        std::cout << "❌ No connected peripheral" << std::endl;
    }

    auto connectionDuration = std::chrono::duration_cast<std::chrono::milliseconds>(currentTime - connectionStartTime).count();
    std::cout << "⏱️  Connection duration before packet loss: " << connectionDuration << " ms (" << connectionDuration / 1000 << "s " << connectionDuration % 1000 << "ms)" << std::endl;
    std::cout << "📊 Final data counter: " << dataReceiveCounter << " packets received" << std::endl;

    std::cout << "🛑 Stopping program due to packet loss..." << std::endl;

    exit(0);
}

// Logging functions implementation
std::string getTimestamp() {
    auto now = std::chrono::system_clock::now();
    auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(now.time_since_epoch()) % 1000;
    auto time_t_now = std::chrono::system_clock::to_time_t(now);
    std::tm* tm = std::localtime(&time_t_now);
    if (!tm) {
        return "Invalid time";
    }
    std::stringstream ss;
    ss << std::put_time(tm, "%Y-%m-%d %H:%M:%S") << "." << std::setfill('0') << std::setw(3) << ms.count();
    return ss.str();
}

void log(const std::string& level, const std::string& message) {
#ifdef DEBUG
    std::cout << "[" << getTimestamp() << "] [" << level << "] " << message << std::endl;
#endif
}

@end