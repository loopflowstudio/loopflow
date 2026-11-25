import Loopflow
import SwiftUI

/// One terminal of a Task, its Session or its shell: the live terminal, and a
/// reply field under your thumb.
struct TerminalView: View {
    @Environment(\.theme) private var theme
    let showChanges: () -> Void
    @Binding var draft: String
    @State private var link: TerminalLink
    @State private var actions = TerminalActions()
    @State private var showingKeys = false
    @FocusState private var replying: Bool
    @Environment(\.scenePhase) private var scenePhase

    init(client: RemoteMobileClient, task: MobileTask, target: TerminalTarget, draft: Binding<String>,
         showChanges: @escaping () -> Void) {
        self.showChanges = showChanges
        _draft = draft
        _link = State(initialValue: TerminalLink(client: client, task: task, target: target))
    }

    var body: some View {
        VStack(spacing: 0) {
            if case .unavailable(let reason) = link.state {
                StateView("No Session here", sentence: reason) {
                    Button("Changes", action: showChanges).buttonStyle(LoopflowButtonStyle(.accent))
                }
            } else {
                SessionTerminal(link: link, actions: actions)
                    .overlay { notice }
                if link.state == .live {
                    if showingKeys { keys }
                    replyBar
                }
            }
        }
        .background(theme.palette.background)
        .onChange(of: scenePhase) { _, phase in
            if phase == .background { link.suspend() }
        }
        .task(id: scenePhase == .active) {
            if scenePhase == .active { await link.resume() }
        }
        .onDisappear { link.close() }
    }

    /// Shown only when the terminal is not live here; a healthy terminal has no status chrome.
    @ViewBuilder
    private var notice: some View {
        switch (link.state, link.target) {
        case (.live, _), (.unavailable, _): EmptyView()
        case (.opening, _): ProgressView().controlSize(.large)
        case (.elsewhere, _):
            state("This terminal is open on another device.", action: "Continue here") { link.continueHere() }
        case (.ended, .session):
            state("The Session ended.", action: "Start it again") { Task { await link.start() } }
        case (.ended, .shell):
            state("This shell has ended.", action: "New shell") { Task { await link.start() } }
        case (.notRunning, _):
            state("The Session is saved, but not running.", action: "Start it") { Task { await link.start() } }
        case (.notStarted, _):
            state("Not started", sentence: "The Task has no checkout yet.", action: "Try again") { Task { await link.start() } }
        case (.disconnected(let reason), _):
            state("Disconnected", sentence: reason, action: "Reconnect") { Task { await link.open() } }
        }
    }

    private func state(_ title: String, sentence: String? = nil, action: String,
                       perform: @escaping () -> Void) -> some View {
        StateView(title, sentence: sentence) {
            Button(action, action: perform).buttonStyle(LoopflowButtonStyle(.accent))
        }
        .background(theme.palette.background)
    }

    private var keys: some View {
        HStack(spacing: 0) {
            ForEach(DesktopKey.phoneRow, id: \.self) { key in
                Button { actions.press(key) } label: {
                    Image(systemName: key.symbol).frame(maxWidth: .infinity, minHeight: HitTarget.touch)
                }
                .accessibilityLabel(key.title)
            }
        }
        .padding(.horizontal, Spacing.sm)
    }

    private var replyBar: some View {
        HStack(alignment: .bottom, spacing: Spacing.sm) {
            Button { showingKeys.toggle() } label: {
                Image(systemName: showingKeys ? "keyboard.chevron.compact.down" : "arrow.up.arrow.down")
                    .frame(width: HitTarget.touch, height: HitTarget.touch)
            }
            .accessibilityLabel(showingKeys ? "Hide keys" : "Arrow and menu keys")
            TextField("Reply", text: $draft, axis: .vertical)
                .lineLimit(1...6)
                .font(Typography.text)
                .focused($replying)
                .padding(.horizontal, Spacing.md)
                .padding(.vertical, Spacing.sm)
                .frame(minHeight: HitTarget.touch)
                .background(theme.palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.pane))
                .overlay(RoundedRectangle(cornerRadius: CornerRadius.pane).stroke(theme.palette.border))
            Button {
                actions.reply(draft)
                draft = ""
            } label: {
                Image(systemName: "arrow.up.circle.fill").font(Typography.lede).imageScale(.large).frame(width: HitTarget.touch, height: HitTarget.touch)
            }
            .disabled(draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            .accessibilityLabel("Send reply")
        }
        .padding(.horizontal, Spacing.sm)
        .padding(.vertical, Spacing.xs)
    }
}
