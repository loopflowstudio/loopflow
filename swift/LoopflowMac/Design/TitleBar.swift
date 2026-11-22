import Loopflow
import SwiftUI

/// The window's one title bar: sidebar toggle, back and forward at 24 pt
/// targets, then the path as a jump bar.
struct TitleBar: View {
    @Bindable var model: WorkModel
    @Environment(\.palette) private var palette
    @State private var removingWave: WaveSnapshot?
    @State private var renamingWave: WaveSnapshot?

    var body: some View {
        ViewThatFits(in: .horizontal) {
            trail(showWave: true).fixedSize(horizontal: true, vertical: false)
            trail(showWave: false)
        }
        .buttonStyle(.plain)
        .font(Typography.textStrong)
        .foregroundStyle(palette.titlebarText, palette.titlebarSecondaryText)
        .accessibilityIdentifier("work-breadcrumbs")
        .waveLifecycleSheets(model: model, removing: $removingWave, renaming: $renamingWave)
    }

    private func trail(showWave: Bool) -> some View {
        HStack(alignment: .center, spacing: Spacing.xs) {
            target("sidebar.left", help: "Toggle sidebar") { model.navigation.toggleNavigator() }
            target("chevron.left", help: "Back") { model.goBack() }
                .disabled(model.navigation.backStack.isEmpty)
            target("chevron.right", help: "Forward") { model.goForward() }
                .disabled(model.navigation.forwardStack.isEmpty)
            Button(model.repoPath.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "Work") {
                // The parent crumb of a zoomed part zooms out.
                if model.navigation.waveView == .repository { model.setZoomed(nil, in: .repository) }
                model.select(nil)
                model.navigation.content = .terminals
            }.lineLimit(1).fixedSize()
            WorkJumpSeparator(model: model, scope: .repository, label: "Waves")
            // A Wave view in the trail, or the Wave of Work outside any board, heads
            // the crumbs after the repository; the repository page adds none.
            let board = model.navigation.trailBoard
            if case .wave(let id) = board {
                waveCrumb(id, name: model.wave(id: id)?.wave.displayName ?? id)
            } else if showWave, board == nil || board == .repository, let wave = model.breadcrumb?.wave {
                waveCrumb(wave.roadmap.wave.id, name: wave.roadmap.wave.displayName)
            }
            switch board {
            // A Dashboard opened through `lf desktop` names itself; Back returns to it.
            case .saved(let id):
                Text(model.dashboard(id)?.name ?? id).lineLimit(1).fixedSize()
                if model.breadcrumb?.task != nil { Text("/").foregroundStyle(palette.titlebarSecondaryText) }
            case .workingTasks:
                Text(WorkNavigation.workingTasksTitle).lineLimit(1).fixedSize()
                if model.breadcrumb?.task != nil { Text("/").foregroundStyle(palette.titlebarSecondaryText) }
            case .wave, .repository, nil: EmptyView()
            }
            if let view = model.navigation.waveView, let zoomed = model.zoomedPart(in: view) {
                ZoomCrumb(title: zoomed.title) { model.setZoomed(nil, in: view) }
            }
            if let task = model.breadcrumb?.task {
                let presentation = model.taskPresentation(id: task.task.id)
                WorkTaskIdentity(presentation: presentation)
                // The word its sidebar row shows.
                if presentation.context == nil,
                   let word = TaskRowStatus(task: task.task, status: model.programStatus(task: task.task.id)).word {
                    Text(verbatim: word).foregroundStyle(palette.titlebarSecondaryText).fixedSize()
                        .accessibilityIdentifier("task-heading-state")
                }
                if let workspace = model.shownWorkspace, workspace.key.kind == .task, workspace.key.id == task.task.id {
                    ShowHiddenMenu(workspace: workspace).font(Typography.meta)
                    let store = workspace.multiplexer
                    if let zoomed = store.zoomedPaneId, let pane = store.layout.pane(for: zoomed) {
                        ZoomCrumb(title: pane.kind.rawValue.capitalized) { store.setZoom(zoomed, enabled: false) }
                    }
                }
            }
            if model.breadcrumb?.task == nil, let selection = model.selection, selection.kind == .task {
                WorkTaskIdentity(presentation: model.taskPresentation(id: selection.id))
            }
            if let target = model.machineTarget {
                WorkMachineControl(model: model, target: target).padding(.leading, Spacing.xs)
            }
        }
    }

    private func target(_ symbol: String, help: String, action: @escaping () -> Void) -> some View {
        Button(action: action) { Image(systemName: symbol) }
            .frame(width: HitTarget.minimum, height: HitTarget.minimum).contentShape(Rectangle())
            .help(help)
    }

    private func waveCrumb(_ id: String, name: String) -> some View {
        HStack(alignment: .center, spacing: Spacing.xs) {
            Button(name) {
                if model.navigation.waveView == .wave(id) { model.setZoomed(nil, in: .wave(id)) }
                model.select(.wave(id: id))
                model.navigation.content = .terminals
            }.lineLimit(1).fixedSize()
            .contextMenu {
                if let wave = model.wave(id: id)?.wave {
                    WaveLifecycleMenu(onRename: { renamingWave = wave }, onRemove: { removingWave = wave })
                }
            }
            WorkJumpSeparator(model: model, scope: .wave(id), label: "Tasks in \(name)")
        }
    }
}
