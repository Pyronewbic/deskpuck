#import "Joycon2BLEReceiver.h"

#include "check.h"

// Stands in for CBCentralManager: records what the receiver asks of it.
@interface FakeCentral : NSObject
@property (nonatomic) CBManagerState state;
@property (nonatomic) int scans;
@property (nonatomic) int stops;
@property (nonatomic) int connects;
@end

@implementation FakeCentral
- (void)scanForPeripheralsWithServices:(NSArray*)services options:(NSDictionary*)options { self.scans++; }
- (void)stopScan { self.stops++; }
- (void)connectPeripheral:(id)peripheral options:(NSDictionary*)options { self.connects++; }
- (void)cancelPeripheralConnection:(id)peripheral {}
@end

// Stands in for CBPeripheral where the receiver only reads its name and identifier.
@interface FakePeripheral : NSObject
@property (nonatomic, copy) NSString* name;
@property (nonatomic, strong) NSUUID* identifier;
@property (nonatomic, weak) id delegate;
@end

@implementation FakePeripheral
- (void)discoverServices:(NSArray*)services {}
@end

static FakeCentral* poweredOnCentral() {
    FakeCentral* central = [[FakeCentral alloc] init];
    central.state = CBManagerStatePoweredOn;
    return central;
}

static Joycon2BLEReceiver* receiverWith(FakeCentral* central, BOOL suspended) {
    Joycon2BLEReceiver* receiver = [[Joycon2BLEReceiver alloc] initWithCentralManager:(CBCentralManager*)central];
    receiver.scanSuspended = suspended;
    return receiver;
}

static FakePeripheral* joycon() {
    FakePeripheral* peripheral = [[FakePeripheral alloc] init];
    peripheral.name = @"Joy-Con 2 (R)";
    peripheral.identifier = [NSUUID UUID];
    return peripheral;
}

static NSDictionary* nintendoAdvertisement() {
    const uint8_t bytes[] = {0x53, 0x05, 0x01};
    return @{CBAdvertisementDataManufacturerDataKey: [NSData dataWithBytes:bytes length:sizeof(bytes)]};
}

static void spin(double seconds) {
    [[NSRunLoop mainRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:seconds]];
}

static void testDirectScan() {
    FakeCentral* running = poweredOnCentral();
    [receiverWith(running, NO) startScan];
    CHECK(running.scans == 1);

    FakeCentral* paused = poweredOnCentral();
    [receiverWith(paused, YES) startScan];
    CHECK(paused.scans == 0);
}

static void testBluetoothPowerOn() {
    // Scan wanted while Bluetooth was off; it then turns on.
    FakeCentral* running = [[FakeCentral alloc] init];
    Joycon2BLEReceiver* runningReceiver = receiverWith(running, NO);
    [runningReceiver startScan];
    FakeCentral* paused = [[FakeCentral alloc] init];
    Joycon2BLEReceiver* pausedReceiver = receiverWith(paused, NO);
    [pausedReceiver startScan];
    pausedReceiver.scanSuspended = YES;

    running.state = paused.state = CBManagerStatePoweredOn;
    [runningReceiver centralManagerDidUpdateState:(CBCentralManager*)running];
    [pausedReceiver centralManagerDidUpdateState:(CBCentralManager*)paused];
    CHECK(running.scans == 1);
    CHECK(paused.scans == 0);
}

static void testDelayedRetries() {
    // A failed connect retries after 2 s, a disconnect after 3 s; both rescan.
    FakeCentral* running = poweredOnCentral();
    FakeCentral* paused = poweredOnCentral();
    Joycon2BLEReceiver* runningReceiver = receiverWith(running, NO);
    Joycon2BLEReceiver* pausedReceiver = receiverWith(paused, YES);
    NSError* error = [NSError errorWithDomain:CBErrorDomain code:CBErrorConnectionTimeout userInfo:nil];
    for (Joycon2BLEReceiver* receiver in @[runningReceiver, pausedReceiver]) {
        CBCentralManager* central = receiver.centralManager;
        [receiver centralManager:central didFailToConnectPeripheral:(CBPeripheral*)joycon() error:error];
        [receiver centralManager:central didDisconnectPeripheral:(CBPeripheral*)joycon() error:nil];
    }
    spin(3.3);
    CHECK(running.scans == 2);
    CHECK(paused.scans == 0);
}

static void testDiscoveryWhilePaused() {
    FakeCentral* running = poweredOnCentral();
    FakeCentral* paused = poweredOnCentral();
    Joycon2BLEReceiver* runningReceiver = receiverWith(running, NO);
    Joycon2BLEReceiver* pausedReceiver = receiverWith(paused, YES);
    [runningReceiver centralManager:(CBCentralManager*)running didDiscoverPeripheral:(CBPeripheral*)joycon()
                  advertisementData:nintendoAdvertisement() RSSI:@-50];
    [pausedReceiver centralManager:(CBCentralManager*)paused didDiscoverPeripheral:(CBPeripheral*)joycon()
                 advertisementData:nintendoAdvertisement() RSSI:@-50];
    CHECK(running.connects == 1);
    CHECK(paused.connects == 0);
    CHECK(pausedReceiver.connectingPeripherals.count == 0);
}

static void testPauseAndResume() {
    FakeCentral* central = poweredOnCentral();
    Joycon2BLEReceiver* receiver = receiverWith(central, NO);
    [receiver startScan];
    CHECK(central.scans == 1);

    // Pausing stops the running scan.
    receiver.scanSuspended = YES;
    CHECK(central.stops == 1);

    // Resuming with nothing connected scans again.
    receiver.scanSuspended = NO;
    CHECK(central.scans == 2);

    // Resuming while a Joy-Con is connected leaves the link alone.
    [receiver.connectedPeripherals addObject:[NSUUID UUID]];
    receiver.scanSuspended = YES;
    receiver.scanSuspended = NO;
    CHECK(central.scans == 2);

    // A receiver that never wanted a scan does not start one on resume.
    FakeCentral* idle = poweredOnCentral();
    Joycon2BLEReceiver* idleReceiver = receiverWith(idle, YES);
    idleReceiver.scanSuspended = NO;
    CHECK(idle.scans == 0);
}

int main() {
    @autoreleasepool {
        testDirectScan();
        testBluetoothPowerOn();
        testDelayedRetries();
        testDiscoveryWhilePaused();
        testPauseAndResume();
    }
    return checkSummary("receiver");
}
