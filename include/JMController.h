#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

// Connects to a Joy-Con 2 and turns its reports into mouse and keyboard events.
@interface JMController : NSObject

- (void)start;

@end

NS_ASSUME_NONNULL_END
