import SwiftUI
import CoreText

extension AppearanceMode {
    public static func resolvedTheme(
        rawValue: String,
        systemScheme: ColorScheme
    ) -> (preferredScheme: ColorScheme?, palette: LoopflowPalette) {
        let mode = AppearanceMode(rawValue: rawValue) ?? .system
        let palette: LoopflowPalette

        switch mode {
        case .light:
            palette = .light
        case .dark:
            palette = .dark
        case .system:
            palette = systemScheme == .dark ? .dark : .light
        }

        return (mode.colorScheme, palette)
    }
}

public enum AppRuntime {
    public static var isAutomatedTest: Bool {
        let environment = ProcessInfo.processInfo.environment
        if environment["XCTestConfigurationFilePath"] != nil ||
            environment["LOOPFLOW_UI_TEST_MODE"] != nil {
            return true
        }

        let arguments = ProcessInfo.processInfo.arguments
        return arguments.contains("-ui-test-mode") || arguments.contains("--snapshot")
    }
}

public enum LaunchArguments {
    public static func repoURL() -> URL? {
        let args = ProcessInfo.processInfo.arguments
        guard let index = args.firstIndex(of: "--repo"), args.count > index + 1 else {
            return nil
        }
        return URL(fileURLWithPath: args[index + 1])
    }
}

private final class LoopflowBundleToken {}

extension Bundle {
    /// The bundle holding the library's copied resources (Fonts), resolved for
    /// packaged apps, SwiftPM development, and xcodegen framework builds.
    static var loopflowResources: Bundle {
        #if SWIFT_PACKAGE
        if let packaged = packagedLoopflowResources(at: Bundle.main.resourceURL) {
            return packaged
        }
        return .module
        #else
        return Bundle(for: LoopflowBundleToken.self)
        #endif
    }

    static func packagedLoopflowResources(at resourcesURL: URL?) -> Bundle? {
        guard let bundleURL = resourcesURL?.appendingPathComponent(
            "LoopflowSwift_Loopflow.bundle",
            isDirectory: true
        ) else {
            return nil
        }
        return Bundle(url: bundleURL)
    }
}

enum AppFontRegistration {
    private static let fontFiles = [
        "CormorantGaramond-Regular.otf",
        "CormorantGaramond-Medium.otf",
        "CormorantGaramond-SemiBold.otf",
        "Lato-Regular.ttf",
        "Lato-Bold.ttf",
        "JetBrainsMono-Regular.ttf",
    ]

    static func registerBundledFonts() {
        let bundle = Bundle.loopflowResources
        for file in fontFiles {
            let name = (file as NSString).deletingPathExtension
            let ext = (file as NSString).pathExtension
            guard let url = bundle.url(forResource: name, withExtension: ext, subdirectory: "Fonts")
                ?? bundle.url(forResource: name, withExtension: ext) else { continue }
            CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
        }
    }
}

/// Register bundled fonts before the app renders.
public func bootstrapLoopflowApp() {
    AppFontRegistration.registerBundledFonts()
}
