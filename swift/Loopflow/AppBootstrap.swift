import SwiftUI
import CoreText

private final class LoopflowBundleToken {}

extension Bundle {
    /// The bundle holding the library's copied resources (Fonts), resolved for
    /// packaged apps, SwiftPM development, and xcodegen framework builds.
    static var loopflowResources: Bundle {
        #if SWIFT_PACKAGE
        if Bundle.main.bundleURL.pathExtension == "app" {
            return packagedLoopflowResources(at: Bundle.main.resourceURL) ?? .main
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

/// The bundled weights are the only ones `Typography` may request; anything
/// else is synthesized by CoreText and renders heavier than the design.
enum AppFontRegistration {
    private static let fontFiles = [
        "OldStandard-Regular.ttf",
        "AtkinsonHyperlegibleNext-Regular.ttf",
        "AtkinsonHyperlegibleNext-Italic.ttf",
        "AtkinsonHyperlegibleNext-SemiBold.ttf",
        "AtkinsonHyperlegibleNext-SemiBoldItalic.ttf",
        "SplineSansMono-Regular.ttf",
        "SplineSansMono-Bold.ttf",
        "SplineSansMono-Italic.ttf",
        "SplineSansMono-BoldItalic.ttf",
    ]

    private static let faces: [String: URL] = {
        var faces: [String: URL] = [:]
        let bundle = Bundle.loopflowResources
        for file in fontFiles {
            let name = (file as NSString).deletingPathExtension
            let ext = (file as NSString).pathExtension
            guard let url = bundle.url(forResource: name, withExtension: ext, subdirectory: "Fonts")
                ?? bundle.url(forResource: name, withExtension: ext),
                  let descriptors = CTFontManagerCreateFontDescriptorsFromURL(url as CFURL) as? [CTFontDescriptor],
                  let face = descriptors.first,
                  let postScriptName = CTFontDescriptorCopyAttribute(face, kCTFontNameAttribute) as? String else { continue }
            CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
            faces[postScriptName] = url
        }
        return faces
    }()

    /// Resolve the packaged face, including when an installed font shares its name.
    static func font(_ name: String, size: CGFloat, descriptor: CTFontDescriptor? = nil) -> CTFont? {
        guard let url = faces[name] else { return nil }
        let base = descriptor ?? CTFontDescriptorCreateWithNameAndSize(name as CFString, size)
        let source = CTFontDescriptorCreateCopyWithAttributes(base, [kCTFontURLAttribute: url] as CFDictionary)
        let font = CTFontCreateWithFontDescriptor(source, size, nil)
        return CTFontCopyPostScriptName(font) as String == name ? font : nil
    }

    static func registerBundledFonts() {
        _ = faces
    }
}

/// Register bundled fonts before the app renders.
public func bootstrapLoopflowApp() {
    AppFontRegistration.registerBundledFonts()
}
