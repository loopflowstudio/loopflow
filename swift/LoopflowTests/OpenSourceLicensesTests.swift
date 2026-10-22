import Foundation
import Testing
@testable import LoopflowMac

struct OpenSourceLicensesTests {
    @Test func bundledNoticesAreReadableOffline() throws {
        let text = try OpenSourceLicenses.text(in: OpenSourceLicenses.resourceBundle)
        #expect(text.contains("SIL OPEN FONT LICENSE"))
        #expect(text.contains("GNU LESSER GENERAL PUBLIC LICENSE"))
        #expect(text.contains("GNU GENERAL PUBLIC LICENSE"))
        #expect(text.contains("Mozilla Public License"))
        #expect(text.contains("FreeType"))
        #expect(text.contains("ThirdPartySources"))
    }

    @Test func missingNoticesReportFailure() {
        #expect(throws: CocoaError.self) {
            try OpenSourceLicenses.text(in: Bundle(for: NSObject.self))
        }
    }
}
