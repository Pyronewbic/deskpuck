#import <Foundation/Foundation.h>

#import "JMConfig.h"

NS_ASSUME_NONNULL_BEGIN

// Connects to a Joy-Con 2 and turns its reports into mouse and keyboard events.
@interface JMController : NSObject

- (instancetype)initWithConfig:(JMConfig*)config NS_DESIGNATED_INITIALIZER;
- (instancetype)init;

- (void)start;
// Takes effect immediately. Refuses a config with validation problems.
- (BOOL)applyConfig:(JMConfig*)config error:(NSError**)error;

@end

NS_ASSUME_NONNULL_END
