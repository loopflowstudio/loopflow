import CoreText
import Foundation
import Testing
@testable import Loopflow

@Suite("App Bootstrap Resources")
struct AppBootstrapResourceTests {
    @Test("The New Task title and workflow labels have glyphs in their bundled faces")
    func newTaskGlyphs() throws {
        for (face, text) in [(Typography.serifFace, "What do you want?"),
                             (Typography.sansFace, "implement · ship-review"),
                             (Typography.sansStrongFace, "verify")] {
            let font = try #require(AppFontRegistration.font(face, size: 18))
            let characters = Array(text.utf16)
            var glyphs = [CGGlyph](repeating: 0, count: characters.count)
            #expect(CTFontGetGlyphsForCharacters(font, characters, &glyphs, characters.count))
            #expect(!glyphs.contains(0))
        }
    }

    @Test("packaged app resolves the SwiftPM bundle from Contents/Resources")
    func packagedBundleResolvesFromResources() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let resources = root.appendingPathComponent("Contents/Resources", isDirectory: true)
        let resourceBundle = resources.appendingPathComponent(
            "LoopflowSwift_Loopflow.bundle",
            isDirectory: true
        )
        try FileManager.default.createDirectory(
            at: resourceBundle.appendingPathComponent("Fonts", isDirectory: true),
            withIntermediateDirectories: true
        )
        defer { try? FileManager.default.removeItem(at: root) }

        let resolved = Bundle.packagedLoopflowResources(at: resources)

        #expect(resolved?.bundleURL.standardizedFileURL == resourceBundle.standardizedFileURL)
    }

    @Test("every ramp face is a bundled font, and the mono has real bold and italic faces")
    func rampFacesAreBundled() {
        bootstrapLoopflowApp()
        for face in [Typography.serifFace, Typography.sansFace, Typography.sansStrongFace, Typography.monoFace] {
            let font = CTFontCreateWithName(face as CFString, 13, nil)
            #expect(CTFontCopyPostScriptName(font) as String == face)
        }
        // Ghostty asks CoreText for the family's bold and italic; a synthesized
        // face would come back as the regular one.
        let mono = CTFontCreateWithName(Typography.monoFace as CFString, 13, nil)
        let styles: [(CTFontSymbolicTraits, String)] = [
            (.traitBold, "SplineSansMono-Bold"),
            (.traitItalic, "SplineSansMono-Italic"),
            ([.traitBold, .traitItalic], "SplineSansMono-BoldItalic"),
        ]
        for (traits, name) in styles {
            let styled = CTFontCreateCopyWithSymbolicTraits(mono, 13, nil, traits, traits)
            #expect(styled.map { CTFontCopyPostScriptName($0) as String } == name)
        }
        #expect(CTFontGetSymbolicTraits(mono).contains(.traitMonoSpace))
    }
}
