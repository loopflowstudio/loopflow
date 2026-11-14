// Disposable process only. Compiles the production ExitSignals implementation.
import AppKit
import GhosttyKit
import QuartzCore
import Synchronization

@_silgen_name("diagnostic_fault")
func diagnosticFault()

func mark(_ value: String) {
    try! FileHandle.standardOutput.write(contentsOf: Data((value + "\n").utf8))
}

let arguments = CommandLine.arguments
let journal = arguments[1]
let flag = UnsafeMutablePointer<Atomic<Bool>>.allocate(capacity: 1)
flag.initialize(to: Atomic(false))
ExitSignals.install(path: journal, flag: flag,
                    fields: ["event": "exit", "launch": "diagnostic", "reason": "signal", "pid": String(getpid())])
let app = NSApplication.shared
app.setActivationPolicy(.prohibited)
guard ghostty_init(UInt(CommandLine.argc), CommandLine.unsafeArgv) == GHOSTTY_SUCCESS,
      let config = ghostty_config_new() else { fatalError("Ghostty init failed") }
arguments[2].withCString { ghostty_config_load_file(config, $0) }
ghostty_config_finalize(config)
guard ghostty_config_diagnostics_count(config) == 0 else { fatalError("Config diagnostics") }
var runtime = ghostty_runtime_config_s()
runtime.wakeup_cb = { _ in }
runtime.action_cb = { _, _, _ in false }
runtime.read_clipboard_cb = { _, _, _, _, _, _ in GHOSTTY_CLIPBOARD_READ_UNAVAILABLE }
runtime.confirm_read_clipboard_cb = { _, _, _, _ in }
runtime.write_clipboard_cb = { _, _, _, _, _ in }
runtime.close_surface_cb = { _, _ in }
guard let terminal = ghostty_app_new(&runtime, config) else { fatalError("Ghostty app failed") }
let view = NSView(frame: NSRect(x: 0, y: 0, width: 800, height: 600))
view.wantsLayer = true
view.layer = CAMetalLayer()
var surfaceConfig = ghostty_surface_config_new()
surfaceConfig.platform_tag = GHOSTTY_PLATFORM_MACOS
surfaceConfig.platform.macos.nsview = Unmanaged.passUnretained(view).toOpaque()
surfaceConfig.scale_factor = 1
// External I/O keeps this probe from starting any shell or provider.
surfaceConfig.io_write_cb = { _, _, _ in }
guard let surface = ghostty_surface_new(terminal, &surfaceConfig) else { fatalError("Ghostty surface failed") }
ghostty_surface_set_size(surface, 800, 600)
let text = "\u{1b}[32mreporter diagnostic\u{1b}[0m\r\n"
text.withCString { ghostty_surface_process_output(surface, $0, UInt(text.utf8.count)) }
ghostty_app_tick(terminal)
mark("terminal-ready pid=\(getpid())")
if arguments.contains("--fault") {
    mark("intentional-native-fault")
    diagnosticFault()
}
ghostty_surface_free(surface)
ghostty_app_free(terminal)
ghostty_config_free(config)
mark("terminal-clean-exit")
