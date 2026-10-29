#if os(macOS)
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

/// Jack Heart's running Desktop polish feedback (LOO-673), one test per fix.
@Suite("Desktop polish")
@MainActor
struct DesktopPolishTests {
    @Test("A Wave view part's hover buttons clear the tile's rounded corner in light and dark",
          arguments: [ColorScheme.light, .dark])
    func wavePartActionsInsetFromCorner(scheme: ColorScheme) throws {
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Rendering is passive") }, repoPath: "/src/loopflow")
        let registry = SessionsWorkspaceRegistry(localMachineId: "fixture")
        model.bindWorkspaces(registry, repository: "repo")
        let conversation = registry.workspace(for: .init(repository: "repo", kind: .repository, id: "repo"),
                                              title: "repo", location: nil)
        conversation.stripHover("wave-session").isHovered = true
        let part = HideablePart(part: .session, view: .repository, model: model, workspace: conversation) { Text("Session") }
            .environment(\.colorScheme, scheme)
        let actions = try part.inspect().find(viewWithAccessibilityIdentifier: "wave-part-actions-session")
        #expect(try actions.padding() == EdgeInsets(top: Spacing.sm, leading: Spacing.sm, bottom: Spacing.sm, trailing: Spacing.sm))
    }
}
#endif
