#import <AppKit/AppKit.h>
#import <Metal/Metal.h>
#include "ghostty.h"
static void wake(void *p) {}
static bool action(ghostty_app_t a, ghostty_target_s t, ghostty_action_s s) { return false; }
static void readClip(void *p, ghostty_clipboard_e c, void *s) {}
static void confirmClip(void *p, const char *c, void *s, ghostty_clipboard_request_e r) {}
static void writeClip(void *p, ghostty_clipboard_e c, const ghostty_clipboard_content_s *s, size_t n, bool b) {}
int main(int argc, char **argv) { @autoreleasepool {
    [NSApplication sharedApplication];
    printf("screens=%lu metal=%s\n", (unsigned long)NSScreen.screens.count,
        MTLCreateSystemDefaultDevice() ? "true" : "false");
    if (ghostty_init(argc, argv) != GHOSTTY_SUCCESS) return 2;
    ghostty_config_t cfg = ghostty_config_new();
    if (argc > 1) ghostty_config_load_default_files(cfg);
    ghostty_config_finalize(cfg);
    ghostty_runtime_config_s rt = {.wakeup_cb=wake, .action_cb=action, .read_clipboard_cb=readClip, .confirm_read_clipboard_cb=confirmClip, .write_clipboard_cb=writeClip};
    ghostty_app_t app = ghostty_app_new(&rt, cfg);
    if (!app) return 3;
    NSView *view = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, 500, 300)];
    view.wantsLayer = YES;
    NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 500, 300)
        styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:NO];
    window.contentView = view;
    [window layoutIfNeeded];
    ghostty_surface_config_s opts = ghostty_surface_config_new();
    opts.platform_tag = GHOSTTY_PLATFORM_MACOS;
    opts.platform.macos.nsview = (__bridge void *)view;
    opts.scale_factor = 2;
    opts.working_directory = "/tmp";
    opts.command = "/bin/cat";
    ghostty_surface_t surface = ghostty_surface_new(app, &opts);
    printf("surface=%s defaults=%s\n", surface ? "created" : "nil", argc > 1 ? "loaded" : "omitted");
    if (surface) ghostty_surface_free(surface);
    ghostty_app_free(app);
    ghostty_config_free(cfg);
    return surface ? 0 : 1;
}}
