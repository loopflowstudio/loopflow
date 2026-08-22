import Foundation
import Testing

/// The W2-178 contract says the primary Wave hierarchy must not surface
/// registered-wave vocabulary or raw sync-timestamp prominence. That vocabulary
/// shipped once (RoadmapView's "Every registered Wave · as of 10:32"), so this
/// guards the removal at the source: the user-facing panes may reference the
/// `isRegistered` model property, but never the retired user-facing phrases.
@Suite("Wave surface primary hierarchy vocabulary")
struct WaveSurfacePrimaryHierarchyTests {
    /// The default center pane (no selection), the list rows, and the selected
    /// Wave detail — every pane a user lands on before touching disclosure.
    private static let primaryPanes = [
        "RoadmapView.swift",
        "WaveRow.swift",
        "WaveDetailPane.swift",
        "WavesView.swift",
        "WorkSurfaceView.swift",
    ]

    private static let retiredVocabulary = [
        "registered Wave",
        "Registered Wave",
        "in the registry",
        "registry read",
        "· as of ",
    ]

    @Test("no primary pane surfaces registered-wave vocabulary or a raw sync timestamp")
    func primaryPanesAreCleanOfRetiredVocabulary() throws {
        for pane in Self.primaryPanes {
            let source = try Self.paneSource(pane)
            for phrase in Self.retiredVocabulary {
                #expect(
                    !source.contains(phrase),
                    "\(pane) still surfaces retired vocabulary: \"\(phrase)\""
                )
            }
        }
    }

    @Test("the primary Podium Wave detail renders the shared metric portfolio")
    func podiumWaveDetailRendersMetricPortfolio() throws {
        let source = try Self.paneSource("WorkSurfaceView.swift")

        #expect(source.contains("WaveMetricPortfolioView("))
        #expect(source.contains("portfolio: roadmap.metricPortfolio"))
    }

    private static func paneSource(_ name: String, sourceFile: String = #filePath) throws -> String {
        let viewsDir = URL(fileURLWithPath: sourceFile)
            .deletingLastPathComponent()      // LoopflowTests
            .deletingLastPathComponent()      // swift
            .appendingPathComponent("LoopflowMac/Views")
            .appendingPathComponent(name)
        return try String(contentsOf: viewsDir, encoding: .utf8)
    }
}
