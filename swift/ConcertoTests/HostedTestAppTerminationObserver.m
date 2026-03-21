#import <AppKit/AppKit.h>
#import <XCTest/XCTest.h>

@interface HostedTestAppTerminationObserver : NSObject <XCTestObservation>
@end

@implementation HostedTestAppTerminationObserver

- (void)testBundleDidFinish:(NSBundle *)testBundle
{
    dispatch_async(dispatch_get_main_queue(), ^{
        if (NSApp == nil) {
            return;
        }

        fprintf(stderr, "ConcertoTests: terminating hosted app after test bundle finished\n");
        fflush(stderr);
        [NSApp terminate:nil];
    });
}

@end

static HostedTestAppTerminationObserver *hostedTestAppTerminationObserver = nil;

__attribute__((constructor))
static void InstallHostedTestAppTerminationObserver(void)
{
    hostedTestAppTerminationObserver = [HostedTestAppTerminationObserver new];
    [[XCTestObservationCenter sharedTestObservationCenter] addTestObserver:hostedTestAppTerminationObserver];
}
