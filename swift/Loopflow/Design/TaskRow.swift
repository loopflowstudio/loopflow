import SwiftUI

/// A Task row's status, in the one vocabulary every surface prints:
/// `ProgramStatusState.label` for what the conversation reports and
/// `TaskState.label` for the lifecycle.
public struct TaskRowStatus: Equatable, Sendable {
    public enum Source: Sendable { case program, condition, lifecycle }

    public let source: Source
    /// Nil draws an info mark that carries `detail`.
    public let word: String?
    /// Nil sets the word in the row's secondary ink.
    public let tone: WorkTone?
    /// What a tooltip or VoiceOver reads.
    public let detail: String?
    /// What a blocked conversation is asking, in its own words.
    public let ask: String?
    private var reason: String?

    /// What the Task needs, with the program's own words first.
    public var line: String? { ask ?? reason }

    /// What the Task's conversation reports. `inferred` marks a word taken
    /// from the Session's availability when no program reported.
    public static func program(_ state: ProgramStatusState, kind: ProgramStatusKind? = nil, progress: UInt8? = nil,
                               inferred: Bool = false, detail: String? = nil, ask: String? = nil) -> TaskRowStatus {
        let facts = [state.label, kind?.rawValue.capitalized, progress.map { "\($0)%" }, inferred ? "inferred" : nil]
        return TaskRowStatus(source: .program, word: facts.compactMap { $0 }.joined(separator: " · "),
                      tone: state.workTone, detail: detail, ask: ask)
    }

    /// The Task's lifecycle, when no conversation reports.
    public static func lifecycle(_ state: TaskState) -> TaskRowStatus {
        TaskRowStatus(source: .lifecycle, word: state.label.capitalized, tone: nil, detail: nil, ask: nil)
    }

    /// The Task's reading says it cannot be opened; `reasons` say why.
    public static func unavailable(_ reasons: [String]) -> TaskRowStatus {
        TaskRowStatus(source: .program, word: "Unavailable", tone: nil, detail: reasons.joined(separator: "\n"), ask: nil)
    }

    /// The status could not be read; `reason` says why.
    public static func unread(_ reason: String) -> TaskRowStatus {
        TaskRowStatus(source: .program, word: nil, tone: nil, detail: reason, ask: nil)
    }
}

extension TaskRowStatus {
    /// Where a Task stands: what its program reports when one does, else what
    /// it needs, else its lifecycle. Whatever the word, a Task that needs
    /// someone keeps the reason as its line.
    public init(task: RoadmapTask, program: TaskRowStatus? = nil) {
        let need = task.need
        if let program {
            self = program
        } else if let need, need != .unknown {
            self.init(source: .condition, word: need.title, tone: .warning, detail: task.condition.reason, ask: nil)
        } else if task.lifecycle == .done && task.task.completedAt == nil {
            self.init(source: .lifecycle, word: "Done, date unavailable", tone: nil, detail: nil, ask: nil)
        } else {
            self = .lifecycle(task.lifecycle)
        }
        if need != nil, !task.condition.reason.isEmpty { reason = task.condition.reason }
    }
}

/// The one Task row: the title in full, then the ID and the status word (what
/// its conversation reports, else its condition or lifecycle). A Task that needs
/// someone adds one line saying why. No stripe, no border, no summary; selection is the
/// host's fill.
public struct TaskRow<Trailing: View>: View {
    let id: String
    let title: String
    let identifier: String?
    /// Opens the Task's issue; without one the identifier is plain text.
    let link: URL?
    let status: TaskRowStatus?
    let auto: Bool
    let context: String?
    let selected: Bool
    let muted: Bool
    let trailing: Trailing
    @Environment(\.palette) private var palette

    /// - Parameters:
    ///   - id: names the row's accessibility identifiers.
    ///   - auto: the Task's auto setting is on.
    ///   - muted: the row belongs to another Wave than the one selected.
    public init(id: String, title: String, identifier: String?, link: URL? = nil, status: TaskRowStatus?,
                auto: Bool = false, context: String? = nil, selected: Bool = false, muted: Bool = false,
                @ViewBuilder trailing: () -> Trailing) {
        self.id = id
        self.title = title
        self.identifier = identifier
        self.link = link
        self.status = status
        self.auto = auto
        self.context = context
        self.selected = selected
        self.muted = muted
        self.trailing = trailing()
    }

    private var secondary: Color { selected ? palette.workSelectedSecondaryText : palette.textSecondary }

    public var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text(title)
                .font(selected ? Typography.textStrong : Typography.text)
                .foregroundStyle(selected ? palette.workSelectedText : muted ? palette.textSecondary : palette.text)
                .fixedSize(horizontal: false, vertical: true)
                .allowsHitTesting(false)
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                identifierText
                if let status { word(status) }
                if auto {
                    Text("Auto")
                        .help("Auto is on: the run takes the interactive steps and lands the Task")
                        .accessibilityIdentifier("work-task-auto-\(id)")
                }
                if let context {
                    Text(context).lineLimit(1)
                }
                Spacer(minLength: 0)
                trailing
            }
            .font(Typography.meta)
            .foregroundStyle(secondary)
            .lineLimit(1)
            if let line = status?.line {
                Text(verbatim: line)
                    .font(Typography.text)
                    .foregroundStyle(secondary)
                    .lineLimit(2)
                    .allowsHitTesting(false)
                    .accessibilityIdentifier("work-task-ask-\(id)")
            }
        }
        .padding(.vertical, Spacing.sm)
        .padding(.horizontal, Spacing.md)
        .frame(maxWidth: .infinity, minHeight: HitTarget.control, alignment: .leading)
    }

    /// The whole row opens the Task; the ID's link stays above the target.
    public func opening(_ action: @escaping () -> Void) -> some View {
        background {
            Button(action: action) { Color.clear.contentShape(Rectangle()) }
                .buttonStyle(.plain)
                .accessibilityLabel(title)
                .accessibilityIdentifier("task-row-open-\(id)")
        }
    }

    @ViewBuilder private func word(_ status: TaskRowStatus) -> some View {
        let detail = status.detail ?? ""
        if let word = status.word {
            switch status.source {
            case .lifecycle:
                Text(verbatim: word).accessibilityIdentifier("work-task-lifecycle-\(id)")
            case .program, .condition:
                Text(verbatim: word)
                    .fontWeight(selected && status.tone != nil ? .semibold : nil)
                    .foregroundStyle(selected ? palette.workSelectedText : status.tone?.ink(in: palette) ?? secondary)
                    .help(Text(verbatim: detail))
                    .accessibilityHint(Text(verbatim: detail))
                    .accessibilityIdentifier(status.source == .condition ? "work-task-condition-\(id)" : "work-program-status-\(id)")
            }
        } else {
            Image(systemName: "info.circle")
                .help(Text(verbatim: detail))
                .accessibilityLabel(Text(verbatim: detail))
                .accessibilityIdentifier("work-program-status-unavailable-\(id)")
        }
    }

    @ViewBuilder private var identifierText: some View {
        if let identifier {
            if let link {
                // Above the row selection target: the issue link never selects Work.
                Link(destination: link) {
                    Text(verbatim: identifier).foregroundStyle(selected ? palette.workSelectedText : palette.linkInk)
                }
                    .buttonStyle(WorkLinkStyle())
                    .help(link.absoluteString)
                    .accessibilityLabel("Open Task \(identifier)")
                    .accessibilityIdentifier("work-task-link-\(id)")
                    .fixedSize()
            } else {
                Text(verbatim: identifier)
                    .accessibilityIdentifier("work-task-id-\(id)")
            }
        }
    }
}

extension TaskRow where Trailing == EmptyView {
    public init(id: String, title: String, identifier: String?, link: URL? = nil, status: TaskRowStatus?,
                auto: Bool = false, context: String? = nil, selected: Bool = false, muted: Bool = false) {
        self.init(id: id, title: title, identifier: identifier, link: link, status: status, auto: auto,
                  context: context, selected: selected, muted: muted) { EmptyView() }
    }
}

/// A link in running text: secondary ink, underlined under the pointer or keyboard focus.
public struct WorkLinkStyle: ButtonStyle {
    @Environment(\.palette) private var palette
    @Environment(\.isFocused) private var focused
    @State private var hovering = false

    public init() {}

    public func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundStyle(hovering || focused ? palette.linkHover : palette.linkInk)
            .underline(hovering || focused)
            .keyboardFocusRing(focused, cornerRadius: CornerRadius.control)
            .onHover { hovering = $0 }
    }
}

extension ProgramStatusState {
    public var workTone: WorkTone {
        switch self {
        case .working: .running
        case .blocked: .warning
        case .error: .failure
        case .done: .done
        case .idle: .available
        case .clear: .neutral
        }
    }
}

extension WorkViewPlan.Content {
    /// A Task's status from the states its list reading carries: what its
    /// conversation reports, else its lifecycle. The reading carries no time
    /// the status began and no ask, so neither is shown. `unavailable` says why
    /// no conversation opens from here, which every Task without one lists, so
    /// it is the word only when the reading gives no other.
    public var status: TaskRowStatus? {
        if states.contains(.blocked) { return .program(.blocked) }
        if states.contains(.error) { return .program(.error) }
        if states.contains(.busy) { return .program(.working) }
        if states.contains(.available) { return .program(.idle) }
        if let state = states.lazy.compactMap({ TaskState(rawValue: $0.rawValue) }).first { return .lifecycle(state) }
        return unavailable.isEmpty ? nil : .unavailable(unavailable)
    }
}
