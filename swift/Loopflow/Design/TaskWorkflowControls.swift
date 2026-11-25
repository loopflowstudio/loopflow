import SwiftUI

/// The workflow track and Task controls both Task workspaces share: a fixed
/// ▶/⏸ button, the Graph's steps with the current one grown into a chip that
/// opens the workflow menu, then Auto and Complete at the far end.
public struct TaskWorkflowControls<Items: View>: View {
    let task: RoadmapTask
    let actions: TaskActions
    let pending: String?
    let run: (String?) -> Void
    let pause: (TaskActions.Pause) -> Void
    let interrupt: () -> Void
    let toggleAuto: () -> Void
    let complete: () -> Void
    @ViewBuilder let items: () -> Items
    @Environment(\.palette) private var palette

    public init(task: RoadmapTask, actions: TaskActions, pending: String?, run: @escaping (String?) -> Void,
                pause: @escaping (TaskActions.Pause) -> Void, interrupt: @escaping () -> Void,
                toggleAuto: @escaping () -> Void, complete: @escaping () -> Void, @ViewBuilder items: @escaping () -> Items) {
        self.task = task
        self.actions = actions
        self.pending = pending
        self.run = run
        self.pause = pause
        self.interrupt = interrupt
        self.toggleAuto = toggleAuto
        self.complete = complete
        self.items = items
    }

    public var body: some View {
        HStack(spacing: Spacing.sm) {
            primary
            ViewThatFits(in: .horizontal) {
                HStack(spacing: Spacing.xs) { track(folded: false) }
                HStack(spacing: Spacing.xs) { track(folded: true) }
            }
            .layoutPriority(-1)
            .help(task.setupPosition ?? actions.sentence ?? "")
            .accessibilityElement(children: .contain)
            .accessibilityHint(actions.sentence ?? "")
            .accessibilityIdentifier("task-workflow-track")
            Spacer(minLength: Spacing.md)
            HStack(spacing: Spacing.md) {
                auto
                completion
            }
        }
    }

    // MARK: The track

    /// Each top-level step, the one the Task stands at grown into the chip.
    /// Folded, the other steps go and the chip stays.
    @ViewBuilder private func track(folded: Bool) -> some View {
        ForEach(Array(actions.track.enumerated()), id: \.offset) { index, step in
            if index == actions.chip {
                chip
            } else if !folded {
                stepButton(step, index: index)
            }
        }
        if actions.chip >= actions.track.count { chip }
    }

    private var jumpDisabled: Bool {
        task.isCompleted || pending != nil || actions.runRefusal != nil
            || actions.control?.action == .pause || actions.control?.action == .pausing
    }

    private func stepButton(_ step: TaskGraph.Step, index: Int) -> some View {
        Button { run(step.name) } label: {
            HStack(spacing: Spacing.xxs) {
                if step.interactive {
                    Image(systemName: "person.fill").font(Typography.meta)
                        .accessibilityLabel("you take part")
                }
                Text(step.name)
            }
            .foregroundStyle(index < actions.chip ? palette.textSecondary : palette.textTertiary)
            .padding(.horizontal, 4)
            .frame(height: transport)
            .contentShape(Rectangle())
        }
        .buttonStyle(StepButtonStyle())
        .fixedSize()
        .disabled(jumpDisabled)
        .help("Start from \(step.name)")
        .accessibilityIdentifier("task-workflow-step-\(index)")
    }

    private var chip: some View {
        Menu {
            if !task.isCompleted {
                if !actions.steps.isEmpty {
                    Section("Start from") {
                        ForEach(actions.steps, id: \.self) { step in
                            Button(step) { run(step) }
                                .disabled(jumpDisabled)
                                .accessibilityIdentifier("task-workflow-move-\(step)")
                        }
                    }
                }
                if actions.control?.action == .pause {
                    Button("Interrupt step") { interrupt() }.disabled(pending != nil)
                } else if actions.pause == .pause {
                    Button("Pause") { pause(.pause) }.disabled(pending != nil)
                }
            }
            items()
        } label: {
            HStack(spacing: 4) {
                // Folded on a narrow pane, the chip truncates before Auto and ✓ move.
                Text(pending ?? actions.control?.label ?? "Reading workflow…").lineLimit(1).truncationMode(.tail)
                    .font(Typography.textStrong)
                Image(systemName: "chevron.down").imageScale(.small)
            }
            .foregroundStyle(ink)
            .padding(.horizontal, 7)
            .frame(height: transport)
            .background(RoundedRectangle(cornerRadius: 6).fill(ink.opacity(0.12)))
            .contentShape(Rectangle())
        }
        .menuIndicator(.hidden)
        .buttonStyle(.plain)
        .fixedSize(horizontal: false, vertical: true)
        .accessibilityIdentifier("task-workflow-move")
    }

    // MARK: ▶/⏸

    /// The fill says what the Task is doing: running (green with Auto on),
    /// held, done, or waiting for a click.
    private var ink: Color {
        switch actions.control?.action {
        case .pause: actions.auto == .on ? palette.success : palette.working
        case .pausing, .resume, .unpause: palette.warning
        case .done: palette.success
        default: palette.text
        }
    }

    private var filled: Bool {
        switch actions.control?.action {
        case .pause, .pausing, .resume, .unpause, .done: true
        default: false
        }
    }

    private var transport: CGFloat {
        #if os(iOS)
        32
        #else
        24
        #endif
    }

    private var primary: some View {
        let control = actions.control
        let title: String = switch control?.action {
        case .pause: "Pause"
        case .pausing: "Pausing"
        case .unpause: "Unpause"
        case .resume: "Unpause and run"
        case .wait: "Turn on Auto to land"
        default: control?.label ?? "Reading workflow"
        }
        // A pending pause already shows the held glyph; a pending unpause the running one.
        let glyph = switch (control?.action, pending) {
        case (.done, _): "checkmark"
        case (_, PendingChanges.pausing?), (.pause, nil), (.pausing, _): "pause.fill"
        default: "play.fill"
        }
        let primaryAction: (() -> Void)? = switch control?.action {
        case .run: { run(nil) }
        case .pause: { pause(.pause) }
        case .unpause, .resume: { pause(.unpause) }
        case .done, .wait, .pausing, nil: nil
        }
        return Button { primaryAction?() } label: {
            Image(systemName: glyph)
                .imageScale(.small)
                .foregroundStyle(filled ? palette.background : ink)
                .frame(width: transport, height: transport)
                .background(RoundedRectangle(cornerRadius: 6).fill(filled ? ink : ink.opacity(0.08)))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(primaryAction == nil || pending != nil || actions.primaryRefusal != nil)
        .help(pending ?? actions.primaryRefusal ?? title)
        .accessibilityLabel(pending ?? title)
        .accessibilityIdentifier("task-workflow-primary")
    }

    // MARK: Auto and Complete

    private var auto: some View {
        Toggle("Auto", isOn: Binding(get: { actions.auto == .on }, set: { _ in toggleAuto() }))
            .toggleStyle(.switch).controlSize(.small).tint(palette.success)
            .fixedSize()
            #if os(iOS)
            .frame(width: 82)
            #else
            .frame(width: 65)
            #endif
            .opacity(task.lifecycle == .paused ? 0.65 : 1)
            .disabled(actions.auto == nil || (actions.auto != .on && pending != nil) || actions.autoRefusal != nil)
            .help(actions.autoRefusal ?? actions.auto?.help(issue: task.task.identifier) ?? "Auto")
            .accessibilityIdentifier("task-auto")
    }

    private var completion: some View {
        Button(action: complete) {
            Image(systemName: "checkmark").fontWeight(actions.completeReady ? .bold : .regular)
                .frame(width: 28, height: transport).contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .foregroundStyle(actions.completeReady ? palette.accentInk : palette.textSecondary)
        .background(RoundedRectangle(cornerRadius: 6).fill(actions.completeReady ? palette.accentInk.opacity(0.14) : .clear))
        .disabled(task.isCompleted)
        .accessibilityLabel("Complete Task").help("Complete Task")
        .accessibilityIdentifier("task-complete")
    }
}

/// An inactive step: plain until hovered or pressed.
private struct StepButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        StepButtonBody(configuration: configuration)
    }
}

private struct StepButtonBody: View {
    let configuration: ButtonStyleConfiguration
    @Environment(\.palette) private var palette
    @State private var hovering = false

    var body: some View {
        configuration.label
            .background(RoundedRectangle(cornerRadius: 6)
                .fill(palette.text.opacity(configuration.isPressed ? 0.12 : hovering ? 0.06 : 0)))
            .onHover { hovering = $0 }
    }
}
