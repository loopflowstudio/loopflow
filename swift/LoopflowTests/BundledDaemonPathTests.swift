// GUI-launched tools inherit the user-installed binary search paths.

import Foundation
import Testing
@testable import LoopflowMac

@Suite("GUI Process Environment")
struct BundledDaemonPathTests {
    // macOS GUI apps inherit this when launched from the Dock or Finder.
    private static let guiPath = "/usr/bin:/bin:/usr/sbin:/sbin"

    @Test("enrichment is idempotent and preserves existing PATH entries")
    func enrichmentIsIdempotent() {
        let once = GUIProcessEnvironment.enrichedPath(from: Self.guiPath)
        let twice = GUIProcessEnvironment.enrichedPath(from: once)

        #expect(once == twice)
        #expect(once.contains("/usr/bin"))
        #expect(once.contains("/opt/homebrew/bin"))
    }

    @Test("enriched env preserves non-PATH keys and upgrades PATH")
    func enrichedPreservesOtherKeys() {
        let input = ["PATH": Self.guiPath, "HOME": "/Users/example", "CUSTOM": "value"]
        let result = GUIProcessEnvironment.enriched(input)

        #expect(result["HOME"] == "/Users/example")
        #expect(result["CUSTOM"] == "value")
        #expect(result["PATH"]?.contains("/opt/homebrew/bin") == true)
    }

}
