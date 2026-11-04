import Foundation
import Loopflow
import Observation

/// The fixture and live client consume the same backend-owned Work projection.
func decodeMobileTasks(_ data: Data, requestID: String) throws -> [WorkViewPlan.Content] {
    let response = try JSONDecoder().decode(MachineResponse.self, from: data)
    guard response.id == requestID else { throw RegistryQueryError("Received a different Task request") }
    return try mobileTasks(response)
}

func mobileTasks(_ response: MachineResponse) throws -> [WorkViewPlan.Content] {
    guard case .read(let output) = response.result else {
        if case .error(_, let message) = response.result { throw RegistryQueryError(message) }
        throw RegistryQueryError("The Machine did not return a Task reading")
    }
    return try mobileTasks(JSONDecoder().decode(WorkViewPlan.self, from: Data(output.utf8)))
}

func mobileTasks(_ plan: WorkViewPlan) throws -> [WorkViewPlan.Content] {
    guard plan.resolution.unavailable.isEmpty else {
        throw RegistryQueryError(plan.resolution.unavailable.joined(separator: "; "))
    }
    let contents = Dictionary(plan.contents.map { ($0.work, $0) }, uniquingKeysWith: { first, _ in first })
    return try plan.projection.visible.filter { $0.work.kind == .task }.map { member in
        guard let task = contents[member.work] else {
            throw RegistryQueryError("The Machine returned an incomplete Task reading")
        }
        return task
    }
}

/// UI dependency, not a wire schema. The Machine adapter supplies existing domain readings.
@MainActor
protocol MobileClient {
    func workingTasks() async throws -> [WorkViewPlan.Content]
    func observeTasks() async throws -> any MobileTaskObservation
    func attach(sessionID: String) async throws -> any MobileAttachment
}

@MainActor
protocol MobileTaskObservation: AnyObject {
    var readings: AsyncThrowingStream<MobileTaskReading, any Error> { get }
    func refresh() async throws
    func close()
}

typealias MobileTaskReading = Result<[WorkViewPlan.Content], RegistryQueryError>

/// One attachment to an existing Session. No provider creation or input retry lives here.
@MainActor
protocol MobileAttachment: AnyObject {
    var sessionID: String { get }
    var events: AsyncStream<MobileTerminalEvent> { get }
    func requestControl(size: TerminalGeometry) throws
    func send(_ bytes: Data) throws
    func resize(columns: Int, rows: Int) throws
    func close()
}

// Renderer-facing events after the transport has validated identity and ownership.
// These are intentionally not Codable; LOO-593 owns the actual framing and codec.
enum MobileTerminalEvent {
    case geometry(columns: Int, rows: Int)
    case snapshot(Data)
    case output(Data)
    case control(input: Bool, geometry: Bool)
    case ended
    case disconnected(String)
}

struct TerminalGeometry: Equatable {
    let columns: Int
    let rows: Int
}

struct SessionUnavailable: LocalizedError {
    let message: String
    init(_ message: String) { self.message = message }
    var errorDescription: String? { message }
}

enum MobileConnectionState: Equatable {
    case loading, viewing, requestingControl, controlling, ended
    case disconnected(String), unavailable(String)

    var label: String {
        switch self {
        case .loading: "Opening Session…"
        case .viewing: "Viewing Session"
        case .requestingControl: "Connecting keyboard…"
        case .controlling: "Keyboard connected"
        case .ended: "Session ended"
        case .disconnected(let reason): "Disconnected · \(reason)"
        case .unavailable(let reason): reason
        }
    }
}

@MainActor
@Observable
final class MobileWorkspace {
    private let client: any MobileClient
    private var attachment: (any MobileAttachment)?
    private var eventTask: Task<Void, Never>?
    private var generation = 0
    private var taskGeneration = 0
    private var taskObservation: (any MobileTaskObservation)?
    private var taskReader: Task<Void, Never>?
    private var ownsGeometry = false
    private var terminalGeometry: TerminalGeometry?
    private var replaying = false
    private var viewport: TerminalGeometry?
    private var sentViewport: TerminalGeometry?
    private(set) var tasks: [WorkViewPlan.Content] = []
    private(set) var isLoadingTasks = true
    private(set) var tasksError: String?
    private(set) var selected: WorkViewPlan.Content?
    private(set) var state: MobileConnectionState = .loading
    // A mounted renderer consumes output immediately; no second scrollback store.
    var render: ((Data, Bool) -> Void)?
    // Grid dimensions, not pixels. Observers preserve the owner's terminal geometry.
    var setGeometry: ((TerminalGeometry?) -> Void)?

    init(client: any MobileClient) { self.client = client }

    func startTaskUpdates() async {
        stopTaskUpdates()
        let opening = taskGeneration
        isLoadingTasks = true
        do {
            let observation = try await client.observeTasks()
            guard opening == taskGeneration, !Task.isCancelled else { observation.close(); return }
            taskObservation = observation
            taskReader = Task { [weak self] in
                do {
                    for try await reading in observation.readings {
                        guard let self, opening == self.taskGeneration else { return }
                        self.isLoadingTasks = false
                        switch reading {
                        case .success(let tasks): self.tasks = tasks; self.tasksError = nil
                        case .failure(let error): self.tasksError = error.message
                        }
                    }
                    throw RegistryQueryError("Task updates ended. Refresh to reconnect.")
                } catch {
                    guard let self, opening == self.taskGeneration else { return }
                    self.stopTaskUpdates()
                    self.tasksError = error.localizedDescription
                }
            }
        } catch {
            guard opening == taskGeneration, !Task.isCancelled else { return }
            isLoadingTasks = false
            tasksError = error.localizedDescription
        }
    }

    func refresh() async {
        guard let observation = taskObservation else { await startTaskUpdates(); return }
        let refreshing = taskGeneration
        isLoadingTasks = true
        defer { if refreshing == taskGeneration { isLoadingTasks = false } }
        do { try await observation.refresh() }
        catch {
            guard refreshing == taskGeneration, !Task.isCancelled else { return }
            stopTaskUpdates()
            tasksError = error.localizedDescription
        }
    }

    func stopTaskUpdates() {
        taskGeneration += 1
        taskReader?.cancel()
        taskReader = nil
        taskObservation?.close()
        taskObservation = nil
        isLoadingTasks = false
    }

    func open(_ task: WorkViewPlan.Content) async {
        let sameSession = selected?.primarySession == task.primarySession
        disconnect()
        if !sameSession {
            terminalGeometry = nil
            setGeometry?(nil)
            render?(Data(), true)
        }
        selected = task
        state = .loading
        guard let session = task.primarySession else {
            state = .unavailable(task.unavailable.first ?? "No existing Session is recorded")
            return
        }
        let opening = generation
        do {
            let link = try await client.attach(sessionID: session)
            guard opening == generation else { link.close(); return }
            guard link.sessionID == session else {
                link.close()
                state = .unavailable("The attachment belongs to a different Session")
                return
            }
            attachment = link
            // The initial control event follows geometry and the replacement
            // screen. Do not expose keyboard takeover against the old screen.
            eventTask = Task { [weak self] in
                for await event in link.events {
                    guard let self, self.generation == opening else { return }
                    self.receive(event)
                }
                // Explicit disconnects (including ended) invalidate this attachment's generation.
                guard let self, self.generation == opening else { return }
                self.fail("Connection closed; input was not retried")
            }
        } catch let error as SessionUnavailable {
            guard opening == generation else { return }
            disconnect()
            state = .unavailable(error.message)
        } catch {
            guard opening == generation else { return }
            fail(error.localizedDescription)
        }
    }

    func reconnect() async {
        guard let selected else { return }
        await open(selected)
    }

    func requestControl() {
        guard state == .viewing else { return }
        state = .requestingControl
        do { try attachment?.requestControl(size: viewport ?? TerminalGeometry(columns: 80, rows: 24)) }
        catch { fail(error.localizedDescription) }
    }

    func send(_ bytes: Data) {
        guard state == .controlling, !replaying, !bytes.isEmpty else { return }
        do { try attachment?.send(bytes) }
        catch { fail("Input delivery is uncertain. Reconnect before typing again.") }
    }

    func resize(columns: Int, rows: Int) {
        guard columns > 0, rows > 0 else { return }
        viewport = TerminalGeometry(columns: columns, rows: rows)
        guard ownsGeometry, sentViewport != viewport else { return }
        do {
            try attachment?.resize(columns: columns, rows: rows)
            sentViewport = viewport
        }
        catch { fail(error.localizedDescription) }
    }

    func suspend() {
        stopTaskUpdates()
        guard selected != nil else { return }
        switch state {
        case .loading, .viewing, .requestingControl, .controlling: fail("Paused while the app is away")
        case .disconnected, .unavailable, .ended: break
        }
    }

    func leave() {
        disconnect()
        selected = nil
        render = nil
        setGeometry = nil
    }

    private func receive(_ event: MobileTerminalEvent) {
        switch event {
        case .geometry(let columns, let rows):
            guard columns > 0, rows > 0 else { return }
            terminalGeometry = TerminalGeometry(columns: columns, rows: rows)
            if !ownsGeometry { setGeometry?(terminalGeometry) }
        case .snapshot(let data):
            replaying = true
            render?(data, true)
            replaying = false
        case .output(let data): render?(data, false)
        case .control(let input, let geometry):
            if ownsGeometry != geometry { sentViewport = nil }
            ownsGeometry = geometry
            setGeometry?(geometry ? nil : terminalGeometry)
            state = input ? .controlling : .viewing
            if geometry, let viewport { resize(columns: viewport.columns, rows: viewport.rows) }
        case .ended:
            disconnect()
            state = .ended
        case .disconnected(let reason): fail(reason)
        }
    }

    private func fail(_ message: String) {
        disconnect()
        state = .disconnected(message)
    }

    private func disconnect() {
        generation += 1
        ownsGeometry = false
        eventTask?.cancel()
        eventTask = nil
        attachment?.close()
        attachment = nil
        sentViewport = nil
        setGeometry?(terminalGeometry)
    }
}
