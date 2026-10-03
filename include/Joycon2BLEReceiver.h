 #pragma once

 #import <CoreBluetooth/CoreBluetooth.h>
 #import <Foundation/Foundation.h>

#include <vector>
#include <map>
#include <string>
#include <utility>
#include <iomanip>
#include <chrono>
#include <sstream>
#include <iostream>

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

// Constants
extern const uint16_t JOYCON2_MANUFACTURER_ID;
extern NSString* const WRITE_CHARACTERISTIC_UUID;
extern NSString* const SUBSCRIBE_CHARACTERISTIC_UUID;

// Global data counter
extern int dataReceiveCounter;

// Logging functions
std::string getTimestamp();
void log(const std::string& level, const std::string& message);

// Initialization
- (instancetype)init;

// Public methods
- (void)startScan;
- (void)stopScan;
- (void)connectToDevice:(NSString*)address;
- (void)disconnect;

 // Callbacks (blocks)
 @property (copy, nonatomic) void (^onDeviceFound)(NSString* name, NSString* address);
 @property (copy, nonatomic) void (^onConnected)(void);
 @property (copy, nonatomic) void (^onDataReceived)(NSDictionary* data);
 @property (copy, nonatomic) void (^onError)(NSString* error);

  // Utility methods
   + (Joycon2BLEReceiver*)sharedInstance;
   + (NSString*)determineDeviceType:(CBPeripheral*)peripheral;
  + (std::map<std::string, float>)parseJoycon2Data:(const std::vector<uint8_t>&)data;
  + (void)printParsedData:(const std::map<std::string, float>&)parsed data:(const std::vector<uint8_t>&)data;



 @end