#if os(macOS)
import Loopflow
import SwiftUI

struct UserAskAttentionButton: View {
    @Bindable var model: PodiumModel
    let onOpen: (AskAttentionRecord) -> Void

    @State private var isPresented = false

    private var asks: [AskAttentionRecord] {
        model.userAskAttention.value ?? []
    }

    var body: some View {
        Button {
            isPresented.toggle()
        } label: {
            Label("\(asks.count)", systemImage: "person.crop.circle.badge.questionmark")
                .font(Typography.caption(9).weight(.semibold))
                .foregroundStyle(.white)
                .padding(.horizontal, Spacing.sm)
                .frame(height: HitTarget.comfortable)
                .background(Color.white.opacity(asks.isEmpty ? 0.06 : 0.16), in: Capsule())
        }
        .buttonStyle(.plain)
        .help(asks.isEmpty ? "No requested sessions" : "\(asks.count) requested sessions")
        .accessibilityLabel("User attention")
        .accessibilityValue(asks.isEmpty ? "No requested sessions" : "\(asks.count) requested sessions")
        .accessibilityIdentifier("podium-user-attention")
        .popover(isPresented: $isPresented, arrowEdge: .bottom) {
            attentionList
        }
    }

    private var attentionList: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("Requested sessions")
                    .font(Typography.sectionTitle(15))
                Spacer()
                Button {
                    Task { await model.refreshUserAskAttention() }
                } label: {
                    Image(systemName: "arrow.clockwise")
                }
                .buttonStyle(.borderless)
                .help("Refresh requested sessions")
            }
            .padding(Spacing.md)

            Divider()

            if let error = model.userAskAttention.errorMessage {
                Label(error, systemImage: "exclamationmark.triangle.fill")
                    .font(Typography.caption(9))
                    .foregroundStyle(Color.statusWarning)
                    .lineLimit(3)
                    .padding(Spacing.md)
            }

            if asks.isEmpty {
                ContentUnavailableView(
                    "No requested sessions",
                    systemImage: "checkmark.circle"
                )
                .frame(minHeight: 150)
            } else {
                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(asks) { item in
                            Button {
                                isPresented = false
                                onOpen(item)
                            } label: {
                                askRow(item)
                            }
                            .buttonStyle(.plain)
                            Divider()
                        }
                    }
                }
            }
        }
        .frame(
            width: 390,
            height: max(200, min(CGFloat(asks.count * 88 + 90), 480))
        )
    }

    private func askRow(_ item: AskAttentionRecord) -> some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            Circle()
                .fill(attentionColor(item.attention))
                .frame(width: 8, height: 8)
                .padding(.top, 5)
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                Text(item.ask.request.label)
                    .font(Typography.body(11).weight(.semibold))
                    .foregroundStyle(.primary)
                    .multilineTextAlignment(.leading)
                    .lineLimit(3)
                Text(
                    "\(item.ask.origin.work.kind.rawValue) · "
                        + "\(item.ask.origin.work.id) · \(attentionLabel(item.attention))"
                )
                .font(Typography.caption(9))
                .foregroundStyle(.secondary)
                .lineLimit(1)
            }
            Spacer(minLength: Spacing.sm)
            Image(systemName: "arrow.up.forward.app")
                .foregroundStyle(.secondary)
        }
        .padding(Spacing.md)
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
        .accessibilityLabel(
            "\(item.ask.request.label), \(attentionLabel(item.attention)), "
                + "\(item.ask.origin.work.kind.rawValue) \(item.ask.origin.work.id)"
        )
    }

    private func attentionLabel(_ state: AskAttentionStateRecord) -> String {
        switch state {
        case .queued: "queued"
        case .claimed: "claimed"
        case .notPresented: "not presented"
        case .active: "active"
        case .stale: "stale"
        }
    }

    private func attentionColor(_ state: AskAttentionStateRecord) -> Color {
        switch state {
        case .active: .statusSuccess
        case .stale: .statusWarning
        case .queued, .claimed, .notPresented: WaveLensColor.blue.glow
        }
    }
}

struct AskSessionView: View {
    let initialAsk: AskAttentionRecord
    let query: RegistryQuery
    let onQueueChanged: () async -> Void

    @Environment(\.palette) private var palette
    @Environment(\.dismiss) private var dismiss
    @State private var askSurface: InvocationSurfaceRecord?
    @State private var capability: LaunchTargetCapability?
    @State private var selectedSurface: LaunchTarget?
    @State private var externalNote: String?
    @State private var fallbackNotice: String?
    @State private var error: String?
    @State private var confirming = false
    @State private var confirmedInvocationId: String?
    @State private var ghosttyUserInitiated = false

    init(
        initialAsk: AskAttentionRecord,
        query: RegistryQuery = RegistryQueryLocal.shared,
        onQueueChanged: @escaping () async -> Void
    ) {
        self.initialAsk = initialAsk
        self.query = query
        self.onQueueChanged = onQueueChanged
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            content
        }
        .frame(minWidth: 760, minHeight: 500)
        .background(palette.background)
        .task(id: initialAsk.id) { await prepare() }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack {
                VStack(alignment: .leading, spacing: Spacing.xxs) {
                    Text("Requested session")
                        .font(Typography.sectionTitle(16))
                        .foregroundStyle(palette.text)
                    Text(initialAsk.ask.request.label)
                        .font(Typography.body(11))
                        .foregroundStyle(palette.textSecondary)
                        .lineLimit(2)
                }
                Spacer()
                if let capability, let askSurface {
                    surfaceMenu(capability: capability, askSurface: askSurface)
                }
                Button {
                    dismiss()
                } label: {
                    Image(systemName: "xmark")
                }
                .buttonStyle(.borderless)
                .accessibilityLabel("Close requested session")
            }
            if let fallbackNotice {
                Text(fallbackNotice)
                    .font(Typography.caption(9))
                    .foregroundStyle(Color.statusWarning)
            }
            if let error {
                Text(error)
                    .font(Typography.caption(9))
                    .foregroundStyle(Color.statusError)
                    .textSelection(.enabled)
            }
        }
        .padding(Spacing.md)
    }

    private func surfaceMenu(
        capability: LaunchTargetCapability,
        askSurface: InvocationSurfaceRecord
    ) -> some View {
        Menu {
            ForEach(capability.attachOptions) { option in
                Button(option.label) {
                    Task {
                        await present(
                            option.surface,
                            askSurface: askSurface,
                            capability: capability,
                            userInitiated: true
                        )
                    }
                }
            }
        } label: {
            Label(
                "Open in \(selectedSurface?.appName ?? "…")",
                systemImage: "rectangle.on.rectangle"
            )
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
    }

    @ViewBuilder
    private var content: some View {
        if let askSurface, selectedSurface == .ghostty {
            let command = LaunchTargetLauncher.command(
                for: askSurface,
                home: askSurface.home
            )
            GhosttyTerminalView(
                workingDirectory: command.cwd,
                argv: command.argv,
                env: command.environment,
                sessionId: "ask-\(initialAsk.id)-\(askSurface.invocation.id)",
                onSurfaceCreated: {
                    Task {
                        await confirmPresented(askSurface)
                        if confirmedInvocationId == askSurface.invocation.id {
                            recordLaunch(
                                .ghostty,
                                askSurface: askSurface,
                                reach: .attach,
                                userInitiated: ghosttyUserInitiated
                            )
                        }
                    }
                }
            )
            .id(askSurface.invocation.id)
            .background(LoopflowPalette.dark.background)
        } else if let selectedSurface, let externalNote {
            ContentUnavailableView {
                Label(
                    "Presented in \(selectedSurface.appName)",
                    systemImage: "arrow.up.forward.app"
                )
            } description: {
                Text(externalNote).textSelection(.enabled)
            } actions: {
                if let askSurface, let capability {
                    Button("Open here in Ghostty") {
                        Task {
                            await present(
                                .ghostty,
                                askSurface: askSurface,
                                capability: capability,
                                userInitiated: true
                            )
                        }
                    }
                }
            }
        } else if let error {
            ContentUnavailableView {
                Label("Could not open session", systemImage: "exclamationmark.triangle")
            } description: {
                Text(error).textSelection(.enabled)
            } actions: {
                Button("Retry") { Task { await prepare() } }
            }
        } else {
            ProgressView("Preparing session…")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    @MainActor
    private func prepare() async {
        error = nil
        do {
            let opened = try await query.prepareAskOpen(askId: initialAsk.id)
            askSurface = opened
            await onQueueChanged()
            let nextCapability = LaunchTargetLauncher.capability(
                host: opened.host,
                cwd: opened.cwd,
                provider: opened.provider,
                providerSessionId: opened.providerSessionId
            )
            capability = nextCapability
            let resolution = LaunchTargetResolver.resolve(
                provider: opened.provider,
                home: opened.home,
                memory: LaunchTargetPreferences.shared.memory,
                capability: nextCapability
            )
            fallbackNotice = resolution.fallbackReason
            await present(
                resolution.surface,
                askSurface: opened,
                capability: nextCapability,
                userInitiated: false
            )
        } catch {
            self.error = error.localizedDescription
        }
    }

    @MainActor
    private func present(
        _ target: LaunchTarget,
        askSurface: InvocationSurfaceRecord,
        capability: LaunchTargetCapability,
        userInitiated: Bool
    ) async {
        error = nil
        externalNote = nil
        let reach = capability.reach(target)
        guard reach == .attach else {
            fallbackNotice = "\(target.appName) cannot attach — using the embedded terminal."
            selectedSurface = .ghostty
            return
        }

        if target == .ghostty {
            ghosttyUserInitiated = userInitiated
            selectedSurface = .ghostty
            if userInitiated { fallbackNotice = nil }
            return
        }

        let result = await LaunchTargetLauncher.launch(
            target,
            attach: askSurface,
            home: askSurface.home,
            reach: reach
        )
        switch result {
        case .attached:
            selectedSurface = target
            externalNote = "The exact Ask Invocation is attached there."
            if userInitiated { fallbackNotice = nil }
            await confirmPresented(askSurface)
            if confirmedInvocationId == askSurface.invocation.id {
                recordLaunch(
                    target,
                    askSurface: askSurface,
                    reach: reach,
                    userInitiated: userInitiated
                )
            }
        case .worktreeOnly, .failed:
            selectedSurface = .ghostty
            fallbackNotice = "\(target.appName) could not attach — using the embedded terminal."
        }
    }

    @MainActor
    private func confirmPresented(_ askSurface: InvocationSurfaceRecord) async {
        let invocationId = askSurface.invocation.id
        guard confirmedInvocationId != invocationId, !confirming else { return }
        confirming = true
        defer { confirming = false }
        do {
            _ = try await query.confirmAskPresented(
                askId: initialAsk.id,
                invocationId: invocationId
            )
            confirmedInvocationId = invocationId
            await onQueueChanged()
        } catch {
            self.error = error.localizedDescription
        }
    }

    @MainActor
    private func recordLaunch(
        _ surface: LaunchTarget,
        askSurface: InvocationSurfaceRecord,
        reach: LaunchTargetReach,
        userInitiated: Bool
    ) {
        LaunchTargetPreferences.shared.recordLaunch(
            surface,
            provider: askSurface.provider,
            home: askSurface.home,
            reach: reach,
            userInitiated: userInitiated,
            launchSucceeded: true
        )
    }
}
#endif
