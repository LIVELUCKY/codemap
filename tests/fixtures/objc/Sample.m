#import "BookStore.h"

@protocol BookPort <NSObject>
- (void)load;
@end

@interface BookStore : NSObject <BookPort>
@property (nonatomic) NSString *name;
- (void)load;
+ (instancetype)shared;
@end

@implementation BookStore
- (void)load {
    [self refresh];
    NSLog(@"x");
}
+ (instancetype)shared {
    return nil;
}
@end

void helper(int x) {
    helper(x);
}
