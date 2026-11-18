import Foundation
import Loopflow
import Observation

struct TerminalGeometry: Equatable {
    let columns: Int
    let rows: Int
}

struct SessionNotRunning: LocalizedError {
    var errorDescription: String? { "The Session is saved, but its terminal is not running." }
}

/// The phone's hold on one Task's running Session.
///
/// Every device viewing the Session may type; the one touched last sizes the
/// terminal. Opening fits it to the phone. When another device takes the size,
/// this one stops drawing and waits for **Continue here**. Nothing typed is
/// ever sent twice.
@MainActor
@Observable
final class SessionLink {
    enum State: Equatable {
        case opening, live, elsewhere, ended, notRunning
        case disconnected(String), unavailable(String)
    }

    let task: MobileTask
    let session: String
    private(set) var state: State = .opening
    /// Draws terminal bytes; `true` clears the screen first.
    @ObservationIgnored var render: ((Data, Bool) -> Void)?
    @ObservationIgnored var prepareFrame: (() -> Void)?
    @ObservationIgnored private let client: RemoteMobileClient
    @ObservationIgnored private var attachment: RemoteAttachment?
    @ObservationIgnored private var events: Task<Void, Never>?
    @ObservationIgnored private var generation = 0
    @ObservationIgnored private var size: TerminalGeometry?
    @ObservationIgnored private var sentSize: TerminalGeometry?
    @ObservationIgnored private var wantsOpen = true
    @ObservationIgnored private var resumeOnReturn = false
    /// From asking for the Session to its first screen, and from input to the next output.
    @ObservationIgnored private var opening: Interaction?
    @ObservationIgnored private var echo: Interaction?
    @ObservationIgnored private var hasSnapshot = false
    @ObservationIgnored private var openingObserved = false

    init(client: RemoteMobileClient, task: MobileTask, session: String) {
        self.client = client
        self.task = task
        self.session = session
    }

    var canStart: Bool { state == .ended || state == .notRunning }

    /// The terminal view reports the grid that fits the phone. The first report opens the Session.
    func fit(columns: Int, rows: Int) {
        guard columns > 0, rows > 0 else { return }
        size = TerminalGeometry(columns: columns, rows: rows)
        if wantsOpen {
            wantsOpen = false
            Task { await open() }
        } else if state == .live, sentSize != size {
            do {
                try attachment?.resize(columns: columns, rows: rows)
                sentSize = size
            } catch { fail(error.localizedDescription) }
        }
    }

    func open() async {
        guard let size else { wantsOpen = true; return }
        close()
        state = .opening
        hasSnapshot = false
        openingObserved = false
        self.opening = Perf.takeOpening(task.id) ?? Perf.begin("return_session", scenario: "remote_attach")
        let opening = generation
        do {
            let link = try await client.attach(sessionID: session, size: size)
            guard opening == generation else { link.close(); return }
            attachment = link
            sentSize = size
            events = Task { [weak self] in
                for await event in link.events {
                    guard let self, self.generation == opening else { return }
                    self.receive(event)
                }
                guard let self, self.generation == opening else { return }
                self.fail("Connection closed.")
            }
        } catch is SessionNotRunning {
            guard opening == generation else { return }
            state = .notRunning
        } catch let error as SessionUnavailable {
            guard opening == generation else { return }
            state = .unavailable(error.message)
        } catch {
            guard opening == generation else { return }
            fail(machineConnectionMessage(error))
        }
    }

    /// Continue a recorded conversation whose terminal stopped, then open it.
    func start() async {
        guard canStart, let size else { return }
        close()
        state = .opening
        let opening = generation
        do {
            try await client.startSession(sessionID: session, request: UUID().uuidString.lowercased(), size: size)
            guard opening == generation else { return }
            await open()
        } catch {
            guard opening == generation else { return }
            fail("Starting was not confirmed. Reconnect to check before trying again. \(error.localizedDescription)")
        }
    }

    func continueHere() {
        guard state == .elsewhere, let size else { return }
        state = .opening
        do {
            try attachment?.takeSize(size)
            sentSize = size
        } catch { fail(error.localizedDescription) }
    }

    func send(_ bytes: Data) {
        guard state == .live, !bytes.isEmpty else { return }
        if echo == nil { echo = Perf.begin("typing_echo", scenario: "remote_terminal") }
        do { try attachment?.send(bytes) }
        catch { fail("That may not have been delivered. Reconnect and check before sending it again.") }
    }

    /// Leaving the foreground drops the connection; the Session keeps running.
    func suspend() {
        switch state {
        case .opening, .live, .disconnected:
            resumeOnReturn = true
            close()
            state = .disconnected("Paused while the app was away.")
        case .elsewhere, .ended, .notRunning, .unavailable: break
        }
    }

    func resume() async {
        guard resumeOnReturn else { return }
        resumeOnReturn = false
        await open()
    }

    func close() {
        generation += 1
        events?.cancel()
        events = nil
        attachment?.close()
        attachment = nil
        sentSize = nil
        opening?.finish(.cancelled)
        echo?.finish(.cancelled)
        opening = nil
        echo = nil
    }

    private func receive(_ event: TerminalEvent) {
        switch event {
        case .snapshot(let data):
            render?(data, true)
            hasSnapshot = true
            finishOpeningIfUsable()
        case .output(let data):
            // Another device's grid would garble this one; its next snapshot replaces the screen.
            if state != .elsewhere { render?(data, false) }
            if let echo { Perf.end(echo) }
            echo = nil
        case .sizing(let sizing):
            state = sizing ? .live : .elsewhere
            if state == .live, let size, sentSize != size { fit(columns: size.columns, rows: size.rows) }
        case .ended:
            close()
            state = .ended
        case .disconnected(let reason): fail(reason)
        case .geometry, .programStatus: break
        }
        finishOpeningIfUsable()
    }

    private func finishOpeningIfUsable() {
        guard hasSnapshot, state == .live, let opening, !openingObserved else { return }
        openingObserved = true
        Perf.end(opening) { [weak self] in self?.prepareFrame?() }
        Task {
            await opening.waitForObservation()
            if self.opening === opening { self.opening = nil }
        }
    }

    private func fail(_ message: String) {
        opening?.finish(.failed)
        echo?.finish(.failed)
        close()
        state = .disconnected(message)
    }
}
