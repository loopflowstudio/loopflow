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
    private static let faces: [String: URL] = {
        let bundle = Bundle.loopflowResources
        let urls = (bundle.urls(forResourcesWithExtension: "ttf", subdirectory: "Fonts") ?? [])
            + (bundle.urls(forResourcesWithExtension: "ttf", subdirectory: nil) ?? [])
        var faces: [String: URL] = [:]
        for resource in urls {
            // CoreText needs the absolute file URL to select this copy of the face.
            let url = resource.absoluteURL
            guard let descriptors = CTFontManagerCreateFontDescriptorsFromURL(url as CFURL) as? [CTFontDescriptor],
                  let face = descriptors.first,
                  let name = CTFontDescriptorCopyAttribute(face, kCTFontNameAttribute) as? String,
                  faces[name] == nil else { continue }
            CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
            faces[name] = url
        }
        return faces
    }()

    /// Start from the packaged file even when an installed font shares its name.
    static func font(_ name: String, size: CGFloat, commonLigaturesOff: Bool = false) -> CTFont? {
        guard let url = faces[name] else { return nil }
        var face = CTFontDescriptorCreateWithAttributes([kCTFontURLAttribute: url, kCTFontNameAttribute: name] as CFDictionary)
        if commonLigaturesOff {
            face = CTFontDescriptorCreateCopyWithFeature(face, NSNumber(value: kLigaturesType),
                                                         NSNumber(value: kCommonLigaturesOffSelector))
        }
        let font = CTFontCreateWithFontDescriptor(face, size, nil)
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
