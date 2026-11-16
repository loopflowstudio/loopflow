#import <Foundation/Foundation.h>
#import <ServiceManagement/ServiceManagement.h>
#include <stdio.h>

int lf_server_register(const char *plist_name, const char *legacy_plist, bool replace, char *message, size_t capacity) {
    @autoreleasepool {
        SMAppService *service = [SMAppService agentServiceWithPlistName:[NSString stringWithUTF8String:plist_name]];
        SMAppService *legacy = legacy_plist ? [SMAppService agentServiceWithPlistName:[NSString stringWithUTF8String:legacy_plist]] : nil;
        // Inspect both before changing either. Migration must not turn a disabled
        // background item into an enabled item under another name.
        if (service.status == SMAppServiceStatusRequiresApproval ||
            (legacy && legacy.status == SMAppServiceStatusRequiresApproval)) {
            snprintf(message, capacity, "Loopflow server is disabled; enable Loopflow in System Settings > Login Items & Extensions");
            return 1;
        }
        NSError *error = nil;
        if (legacy && legacy.status == SMAppServiceStatusEnabled && ![legacy unregisterAndReturnError:&error]) {
            snprintf(message, capacity, "Cannot retire previous Loopflow server: %s", error.localizedDescription.UTF8String);
            return 1;
        }
        if (service.status == SMAppServiceStatusEnabled && !replace) return 0;
        if (replace && service.status == SMAppServiceStatusEnabled && ![service unregisterAndReturnError:&error]) {
            snprintf(message, capacity, "Cannot replace bundled Loopflow server: %s", error.localizedDescription.UTF8String);
            return 1;
        }
        if (![service registerAndReturnError:&error]) {
            snprintf(message, capacity, "Cannot register bundled Loopflow server: %s", error.localizedDescription.UTF8String);
            return 1;
        }
        if (service.status != SMAppServiceStatusEnabled) {
            snprintf(message, capacity, "Loopflow server requires approval in System Settings > Login Items & Extensions");
            return 1;
        }
        return 0;
    }
}
