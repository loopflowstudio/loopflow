import SwiftUI

/// The same switch, completion slot and split workflow button in both Task workspaces.
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
            HStack(spacing: 0) {
                primary
                Divider().padding(.vertical, 4)
                Menu {
                    if !task.isCompleted {
                        if !actions.steps.isEmpty {
                            Section("Start from") {
                                ForEach(actions.steps, id: \.self) { step in
                                    Button(step) { run(step) }
                                        .disabled(pending != nil || actions.runRefusal != nil || actions.control?.action == .pause)
                                        .accessibilityIdentifier("task-workflow-move-\(step)")
                                }
                            }
                        }
                        if task.lifecycle == .paused {
                            Button("Unpause without running") { pause(.unpause) }
                                .disabled(pending != nil || actions.pauseRefusal != nil)
                        } else if actions.control?.action == .pause {
                            Button("Interrupt step") { interrupt() }.disabled(pending != nil)
                        } else if actions.pause == .pause {
                            Button("Pause") { pause(.pause) }.disabled(pending != nil)
                        }
                    }
                    items()
                } label: {
                    HStack(spacing: 4) {
                        Text(actions.control?.label ?? "Reading…").lineLimit(1)
                        Spacer(minLength: 0)
                        Image(systemName: "chevron.down").imageScale(.small)
                    }
                    .padding(.horizontal, 7)
                    .contentShape(Rectangle())
                }
                .menuIndicator(.hidden)
                .accessibilityIdentifier("task-workflow-move")
            }
            .buttonStyle(.plain)
            .frame(width: 178, height: controlHeight)
            .foregroundStyle(ink)
            .background(Capsule().fill(ink.opacity(0.07)))
            .overlay { Capsule().strokeBorder(palette.border).allowsHitTesting(false) }

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

            Button(action: complete) {
                Image(systemName: "checkmark").fontWeight(actions.completeReady ? .bold : .regular)
                    .frame(width: 28, height: controlHeight).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .foregroundStyle(actions.completeReady ? palette.accentInk : palette.textSecondary)
            .background(Capsule().fill(actions.completeReady ? palette.accentInk.opacity(0.14) : .clear))
            .disabled(task.isCompleted)
            .accessibilityLabel("Complete Task").help("Complete Task")
            .accessibilityIdentifier("task-complete")
        }
    }

    private var ink: Color {
        switch actions.control?.action {
        case .pause: actions.auto == .on ? palette.success : palette.working
        case .resume, .unpause: palette.warning
        case .done: palette.accentInk
        default: palette.text
        }
    }

    private var controlHeight: CGFloat {
        #if os(iOS)
        44
        #else
        26
        #endif
    }

    private var primary: some View {
        let control = actions.control
        let title: String = switch control?.action {
        case .pause: "Pause"
        case .unpause: "Unpause"
        case .resume: "Unpause and run"
        case .wait: "Turn on Auto to land"
        default: control?.label ?? "Reading workflow"
        }
        return Button {
            switch control?.action {
            case .run: run(nil)
            case .pause: pause(.pause)
            case .unpause: pause(.unpause)
            case .resume: pause(.resume)
            case .done, .wait, nil: break
            }
        } label: {
            Image(systemName: control?.action == .done ? "checkmark" : control?.action == .pause ? "pause.fill" : "play.fill")
                .imageScale(.small).frame(width: controlHeight, height: controlHeight).contentShape(Rectangle())
        }
        .disabled(control == nil || control?.action == .done || control?.action == .wait || pending != nil || actions.primaryRefusal != nil)
        .help(pending ?? actions.primaryRefusal ?? title)
        .accessibilityLabel(title)
        .accessibilityIdentifier("task-workflow-primary")
    }
}
