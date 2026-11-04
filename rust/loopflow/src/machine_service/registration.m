#import <Foundation/Foundation.h>
#import <ServiceManagement/ServiceManagement.h>
#include <stdio.h>

int lf_machine_service_register(const char *plist_name, bool replace, char *message, size_t capacity) {
    @autoreleasepool {
        SMAppService *service = [SMAppService agentServiceWithPlistName:[NSString stringWithUTF8String:plist_name]];
        if (service.status == SMAppServiceStatusEnabled && !replace) return 0;
        if (service.status == SMAppServiceStatusRequiresApproval) {
            snprintf(message, capacity, "Machine service is disabled; enable Loopflow in System Settings > Login Items & Extensions");
            return 1;
        }
        NSError *error = nil;
        if (replace && ![service unregisterAndReturnError:&error]) {
            snprintf(message, capacity, "Cannot replace bundled Machine service: %s", error.localizedDescription.UTF8String);
            return 1;
        }
        if (![service registerAndReturnError:&error]) {
            snprintf(message, capacity, "Cannot register bundled Machine service: %s", error.localizedDescription.UTF8String);
            return 1;
        }
        if (service.status != SMAppServiceStatusEnabled) {
            snprintf(message, capacity, "Machine service requires approval in System Settings > Login Items & Extensions");
            return 1;
        }
        return 0;
    }
}
