#pragma once

#import <CoreBluetooth/CoreBluetooth.h>
#import <Foundation/Foundation.h>

#include "Joycon2Packet.h"

#include <string>
#include <vector>

extern const uint16_t JOYCON2_MANUFACTURER_ID;
extern NSString* const WRITE_CHARACTERISTIC_UUID;
extern NSString* const SUBSCRIBE_CHARACTERISTIC_UUID;

extern int dataReceiveCounter;

std::string getTimestamp();
void log(const std::string& level, const std::string& message);

@interface Joycon2BLEReceiver : NSObject<CBCentralManagerDelegate, CBPeripheralDelegate>

@property (strong, nonatomic) CBCentralManager* centralManager;
@property (strong, nonatomic) CBPeripheral* connectedPeripheral;
@property (strong, nonatomic) CBCharacteristic* writeCharacteristic;
@property (strong, nonatomic) CBCharacteristic* subscribeCharacteristic;
@property (assign, nonatomic) BOOL shouldScan;
@property (strong, nonatomic) NSMutableSet* connectingPeripherals;
@property (strong, nonatomic) NSMutableSet* connectedPeripherals;
@property (strong, nonatomic) NSString* deviceType;
@property (strong, nonatomic) NSTimer* dataTimeoutTimer;
@property (strong, nonatomic) NSTimer* commandTimer;
@property (assign, nonatomic) int displayInterval;
@property (assign, nonatomic) BOOL skipInitCommands;

@property (copy, nonatomic) void (^onDeviceFound)(NSString* name, NSString* address);
@property (copy, nonatomic) void (^onConnected)(void);
@property (copy, nonatomic) void (^onReportReceived)(const Joycon2Report& report);
@property (copy, nonatomic) void (^onError)(NSString* error);

- (instancetype)init;
- (void)startScan;
- (void)stopScan;
- (void)connectToDevice:(NSString*)address;
- (void)disconnect;

+ (Joycon2BLEReceiver*)sharedInstance;
+ (NSString*)determineDeviceType:(CBPeripheral*)peripheral;
+ (void)printReport:(const Joycon2Report&)report data:(const std::vector<uint8_t>&)data;

@end
