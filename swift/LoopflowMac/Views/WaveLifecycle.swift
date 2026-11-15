// Rename and Remove for one Wave, offered wherever a Wave is named: the sidebar
// rows, the titlebar crumb and the Waves window. The sheets run `lf wave
// rename|remove`, so every entry point behaves the same.

import SwiftUI
import Loopflow

/// The context-menu items.
struct WaveLifecycleMenu: View {
    let onRename: () -> Void
    let onRemove: () -> Void

    var body: some View {
        Button(action: onRename) { Label("Rename Wave…", systemImage: "pencil") }
        Button(role: .destructive, action: onRemove) { Label("Remove Wave…", systemImage: "trash") }
    }
}

/// Presents the sheets for the Wave each binding names; a removed Wave that was
/// selected gives way to the repository.
private struct WaveLifecycleSheets: ViewModifier {
    let model: WorkModel
    @Binding var removing: WaveSnapshot?
    @Binding var renaming: WaveSnapshot?

    func body(content: Content) -> some View {
        content
            .sheet(item: $removing) { wave in
                RemoveWaveSheet(
                    model: model, name: wave.id, displayName: wave.displayName, repo: model.repoPath ?? wave.repo,
                    onRemoved: {
                        if model.navigation.sidebarScope == .wave(wave.id) { model.select(nil) }
                        removing = nil
                    },
                    onCancel: { removing = nil }
                )
            }
            .sheet(item: $renaming) { wave in
                RenameWaveSheet(
                    model: model, id: wave.id, currentName: wave.name, repo: model.repoPath ?? wave.repo,
                    onDone: { renaming = nil }
                )
            }
    }
}

extension View {
    func waveLifecycleSheets(
        model: WorkModel, removing: Binding<WaveSnapshot?>, renaming: Binding<WaveSnapshot?>
    ) -> some View {
        modifier(WaveLifecycleSheets(model: model, removing: removing, renaming: renaming))
    }
}
