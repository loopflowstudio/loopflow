#if os(iOS)
import SwiftUI
import LoopflowCore

struct DiscoveryView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @Environment(\.authService) private var authService

    @State private var discoveryService: DiscoveryService?
    @State private var phase: DiscoveryPhase = .signedOut
    @State private var daemons: [DiscoveredDaemon] = []
    @State private var reachability: [String: ReachabilityState] = [:]
    @State private var errorMessage: String?
    @State private var probeGeneration = 0
    @State private var autoConnectAttempted = false

    var body: some View {
        NavigationStack {
            content
                .navigationTitle("Discover")
                .toolbar {
                    if isAuthenticated {
                        ToolbarItem(placement: .topBarTrailing) {
                            Button("Sign out") {
                                signOut()
                            }
                        }
                    }
                }
        }
        .task {
            await bootstrap()
        }
        .tint(.loopflowBurgundy)
    }

    @ViewBuilder
    private var content: some View {
        switch phase {
        case .signedOut, .signingIn:
            signedOutContent
        case .discovering:
            discoveringContent
        case .daemonList, .connecting:
            daemonListContent
        }
    }

    private var signedOutContent: some View {
        VStack(spacing: Spacing.xl) {
            Spacer()

            Image(systemName: "dot.radiowaves.left.and.right")
                .font(.system(size: 44))
                .foregroundStyle(.loopflowBurgundy)
                .accessibilityHidden(true)

            VStack(spacing: Spacing.sm) {
                Text("Sign in to discover your running lfds")
                    .font(Typography.sectionTitle())
                    .multilineTextAlignment(.center)

                if let errorMessage {
                    Text(errorMessage)
                        .font(Typography.caption())
                        .foregroundStyle(Color.statusError)
                        .multilineTextAlignment(.center)
                }
            }

            Button {
                signIn()
            } label: {
                if phase == .signingIn {
                    ProgressView()
                } else {
                    Text("Sign in")
                }
            }
            .buttonStyle(.borderedProminent)

            NavigationLink("Manual connection ›") {
                ConnectionSetupView()
            }
            .font(Typography.body())

            Spacer()
        }
        .padding(Spacing.xl)
    }

    private var discoveringContent: some View {
        VStack(spacing: Spacing.lg) {
            ProgressView()
            Text("Looking for your lfds…")
                .font(Typography.body())
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var daemonListContent: some View {
        List {
            if daemons.isEmpty {
                Section {
                    VStack(alignment: .leading, spacing: Spacing.sm) {
                        Text("No lfds found")
                            .font(Typography.sectionTitle())
                        Text("Start lfd on your Mac with studio auth to see it here.")
                            .font(Typography.body())
                            .foregroundStyle(.secondary)
                    }
                    .padding(.vertical, Spacing.sm)
                }
            } else {
                Section("Available daemons") {
                    ForEach(daemons) { daemon in
                        Button {
                            connect(to: daemon)
                        } label: {
                            daemonRow(daemon)
                        }
                        .buttonStyle(.plain)
                        .disabled(isConnecting && !isCurrentlyConnecting(daemon))
                    }
                }
            }

            Section {
                NavigationLink("Manual connection ›") {
                    ConnectionSetupView()
                }
            }

            if let errorMessage {
                Section {
                    Text(errorMessage)
                        .font(Typography.caption())
                        .foregroundStyle(Color.statusError)
                }
            }
        }
        .refreshable {
            await discoverDaemons(showSpinner: false)
        }
    }

    private func daemonRow(_ daemon: DiscoveredDaemon) -> some View {
        HStack(spacing: Spacing.md) {
            reachabilityIndicator(for: daemon)

            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text(daemon.displayName)
                    .font(Typography.body())
                    .foregroundStyle(.primary)

                Text(daemon.repoSummary)
                    .font(Typography.caption())
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.leading)
            }

            Spacer()

            if isCurrentlyConnecting(daemon) {
                ProgressView()
                    .controlSize(.small)
            }
        }
        .padding(.vertical, Spacing.xs)
    }

    @ViewBuilder
    private func reachabilityIndicator(for daemon: DiscoveredDaemon) -> some View {
        switch reachability[daemon.id] ?? .checking {
        case .checking:
            ProgressView()
                .controlSize(.small)
                .frame(width: 14, height: 14)
        case .reachable:
            Circle()
                .fill(Color.statusSuccess)
                .frame(width: 10, height: 10)
        case .unreachable:
            Circle()
                .fill(Color.statusNeutral)
                .frame(width: 10, height: 10)
        }
    }

    private var isAuthenticated: Bool {
        authService.currentToken() != nil
    }

    private var isConnecting: Bool {
        phase.connectingMachineId != nil
    }

    private func isCurrentlyConnecting(_ daemon: DiscoveredDaemon) -> Bool {
        phase.connectingMachineId == daemon.machineId
    }

    @MainActor
    private func bootstrap() async {
        if discoveryService == nil {
            discoveryService = DiscoveryService(authService: authService)
        }

        guard isAuthenticated else {
            phase = .signedOut
            return
        }

        if phase == .signedOut {
            await discoverDaemons()
        }
    }

    private func signIn() {
        phase = .signingIn
        errorMessage = nil

        Task {
            do {
                _ = try await authService.signIn()
                await discoverDaemons()
            } catch {
                await MainActor.run {
                    phase = .signedOut
                    errorMessage = error.localizedDescription
                }
            }
        }
    }

    private func signOut() {
        do {
            try authService.signOut()
        } catch {
            errorMessage = error.localizedDescription
        }

        daemons = []
        reachability = [:]
        phase = .signedOut
        autoConnectAttempted = false
    }

    @MainActor
    private func discoverDaemons(showSpinner: Bool = true) async {
        if showSpinner {
            phase = .discovering
        }
        errorMessage = nil
        autoConnectAttempted = false

        do {
            guard let service = discoveryService else { return }
            let discovered = try await service.discoverDaemons()
            daemons = discovered
            phase = .daemonList
            startReachabilityProbes(for: discovered)
        } catch {
            if case DiscoveryServiceError.notAuthenticated = error {
                daemons = []
                reachability = [:]
                phase = .signedOut
            } else {
                phase = .daemonList
                errorMessage = error.localizedDescription
            }
        }
    }

    @MainActor
    private func startReachabilityProbes(for discovered: [DiscoveredDaemon]) {
        probeGeneration += 1
        let generation = probeGeneration
        reachability = Dictionary(uniqueKeysWithValues: discovered.map { ($0.id, .checking) })

        for daemon in discovered {
            Task {
                let status = await probeReachability(for: daemon) ? ReachabilityState.reachable : .unreachable
                await MainActor.run {
                    guard generation == probeGeneration else { return }
                    reachability[daemon.id] = status
                    maybeAutoConnect()
                }
            }
        }
    }

    private func probeReachability(for daemon: DiscoveredDaemon) async -> Bool {
        guard let baseURL = daemon.daemonURL else {
            return false
        }

        let healthURL = baseURL.appendingPathComponent("health")
        var request = URLRequest(url: healthURL)
        request.httpMethod = "GET"
        request.timeoutInterval = 3

        do {
            let (_, response) = try await URLSession.shared.data(for: request)
            guard let httpResponse = response as? HTTPURLResponse else {
                return false
            }
            return (200 ... 299).contains(httpResponse.statusCode)
        } catch {
            return false
        }
    }

    @MainActor
    private func maybeAutoConnect() {
        guard !autoConnectAttempted else { return }
        guard case .daemonList = phase else { return }
        guard !daemons.isEmpty else { return }

        let statuses = daemons.map { reachability[$0.id] ?? .checking }
        guard statuses.allSatisfy({ $0 != .checking }) else { return }

        let reachable = zip(daemons, statuses)
            .filter { _, status in status == .reachable }
            .map(\.0)
        guard reachable.count == 1 else { return }

        autoConnectAttempted = true
        connect(to: reachable[0])
    }

    @MainActor
    private func connect(to daemon: DiscoveredDaemon) {
        phase = .connecting(machineId: daemon.machineId)
        errorMessage = nil

        Task {
            do {
                let connection = try daemon.makeConnection()
                try await repoState.connect(to: connection, outputBuffer: outputBuffer)
            } catch {
                await MainActor.run {
                    phase = .daemonList
                    errorMessage = error.localizedDescription
                }
            }
        }
    }
}

private enum DiscoveryPhase: Equatable {
    case signedOut
    case signingIn
    case discovering
    case daemonList
    case connecting(machineId: String)

    var connectingMachineId: String? {
        guard case let .connecting(machineId) = self else {
            return nil
        }
        return machineId
    }
}

private enum ReachabilityState: Equatable {
    case checking
    case reachable
    case unreachable
}
#endif
