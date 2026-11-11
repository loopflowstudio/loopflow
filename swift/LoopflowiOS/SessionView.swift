import Loopflow
import SwiftUI

/// One Task's Session: the live terminal, and a reply field under your thumb.
struct SessionView: View {
    let theme: ThemeDefinition
    @Binding var draft: String
    @State private var link: SessionLink
    @State private var actions = TerminalActions()
    @State private var showingKeys = false
    @FocusState private var replying: Bool
    @Environment(\.scenePhase) private var scenePhase

    init(client: RemoteMobileClient, task: WorkViewPlan.Content, theme: ThemeDefinition, draft: Binding<String>) {
        self.theme = theme
        _draft = draft
        _link = State(initialValue: SessionLink(client: client, task: task))
    }

    var body: some View {
        VStack(spacing: 0) {
            if case .unavailable(let reason) = link.state {
                ContentUnavailableView("No Session here", systemImage: "terminal", description: Text(reason))
            } else {
                SessionTerminal(link: link, actions: actions, theme: theme)
                    .overlay { notice }
                if link.state == .live {
                    if showingKeys { keys }
                    replyBar
                }
            }
        }
        .background(theme.palette.background)
        .navigationTitle(link.task.title)
        .navigationBarTitleDisplayMode(.inline)
        .onChange(of: scenePhase) { _, phase in
            if phase == .background { link.suspend() }
        }
        .task(id: scenePhase == .active) {
            if scenePhase == .active { await link.resume() }
        }
        .onDisappear { link.close() }
    }

    /// Shown only when the terminal is not live here; a healthy Session has no status chrome.
    @ViewBuilder
    private var notice: some View {
        switch link.state {
        case .live, .unavailable: EmptyView()
        case .opening: ProgressView().controlSize(.large)
        case .elsewhere:
            card("This Session is open on another device.", action: "Continue here") { link.continueHere() }
        case .ended:
            card("The Session ended.", action: "Start it again") { Task { await link.start() } }
        case .notRunning:
            card("The Session is saved, but not running.", action: "Start it") { Task { await link.start() } }
        case .disconnected(let reason):
            card(reason, action: "Reconnect") { Task { await link.open() } }
        }
    }

    private func card(_ message: String, action: String, perform: @escaping () -> Void) -> some View {
        VStack(spacing: 16) {
            Text(message)
                .font(Typography.body(16))
                .multilineTextAlignment(.center)
            Button(action: perform) {
                Text(action).font(Typography.strong(16)).frame(minWidth: 160, minHeight: 44)
            }
            .buttonStyle(.borderedProminent)
            .tint(theme.palette.accent)
            .foregroundStyle(theme.palette.accentText)
        }
        .padding(24)
        .background(theme.palette.surface, in: RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(theme.palette.border))
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(theme.palette.background.opacity(0.85))
    }

    private var keys: some View {
        HStack(spacing: 0) {
            ForEach(TerminalKey.allCases, id: \.self) { key in
                Button { actions.press(key) } label: {
                    Image(systemName: key.symbol).frame(maxWidth: .infinity, minHeight: 44)
                }
                .accessibilityLabel(key.title)
            }
        }
        .padding(.horizontal, 8)
    }

    private var replyBar: some View {
        HStack(alignment: .bottom, spacing: 8) {
            Button { showingKeys.toggle() } label: {
                Image(systemName: showingKeys ? "keyboard.chevron.compact.down" : "arrow.up.arrow.down")
                    .frame(width: 44, height: 44)
            }
            .accessibilityLabel(showingKeys ? "Hide keys" : "Arrow and menu keys")
            TextField("Reply", text: $draft, axis: .vertical)
                .lineLimit(1...6)
                .font(Typography.body(16))
                .focused($replying)
                .padding(.horizontal, 14)
                .padding(.vertical, 10)
                .background(theme.palette.surface, in: RoundedRectangle(cornerRadius: 20))
                .overlay(RoundedRectangle(cornerRadius: 20).stroke(theme.palette.border))
            Button {
                actions.reply(draft)
                draft = ""
            } label: {
                Image(systemName: "arrow.up.circle.fill").font(.system(size: 32)).frame(width: 44, height: 44)
            }
            .disabled(draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            .accessibilityLabel("Send reply")
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 6)
    }
}
