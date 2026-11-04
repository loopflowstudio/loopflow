import Foundation
import Loopflow
import Observation

/// The fixture exercises the backend-owned envelope and existing Work projection.
/// Live view-plan admission remains with the Machine service.
func decodeMobileTasks(_ data: Data, requestID: String) throws -> [WorkViewPlan.Content] {
    let response = try JSONDecoder().decode(MachineResponse.self, from: data)
    guard response.id == requestID else { throw RegistryQueryError("Received a different Task request") }
    guard case .read(let output) = response.result else {
        if case .error(_, let message) = response.result { throw RegistryQueryError(message) }
        throw RegistryQueryError("The Machine did not return a Task reading")
    }
    let plan = try JSONDecoder().decode(WorkViewPlan.self, from: Data(output.utf8))
    guard plan.resolution.unavailable.isEmpty else {
        throw RegistryQueryError(plan.resolution.unavailable.joined(separator: "; "))
    }
    return plan.projection.visible.compactMap { member in
        guard member.work.kind == .task else { return nil }
        return plan.contents.first { $0.work == member.work }
    }
}

/// UI dependency, not a wire schema. The Machine adapter supplies existing domain readings.
@MainActor
protocol MobileClient {
    func workingTasks() async throws -> [WorkViewPlan.Content]
    func attach(sessionID: String) async throws -> any MobileAttachment
}

/// One attachment to an existing Session. No provider creation or input retry lives here.
@MainActor
protocol MobileAttachment: AnyObject {
    var sessionID: String { get }
    var events: AsyncStream<MobileTerminalEvent> { get }
    func requestControl() throws
    func send(_ bytes: Data) throws
    func resize(columns: Int, rows: Int) throws
    func close()
}

// Renderer-facing events after the transport has validated identity and ownership.
// These are intentionally not Codable; LOO-593 owns the actual framing and codec.
enum MobileTerminalEvent {
    case snapshot(Data)
    case output(Data)
    case control(input: Bool, geometry: Bool)
    case ended
    case disconnected(String)
}

enum MobileConnectionState: Equatable {
    case loading, viewing, controlling, ended
    case disconnected(String), unavailable(String)

    var label: String {
        switch self {
        case .loading: "Opening Session…"
        case .viewing: "Viewing · keyboard control is elsewhere or unclaimed"
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
    private var ownsGeometry = false
    private var replaying = false
    private var viewport: (columns: Int, rows: Int)?
    private(set) var tasks: [WorkViewPlan.Content] = []
    private(set) var isLoadingTasks = false
    private(set) var tasksError: String?
    private(set) var selected: WorkViewPlan.Content?
    private(set) var state: MobileConnectionState = .loading
    // A mounted renderer consumes output immediately; no second scrollback store.
    var render: ((Data, Bool) -> Void)?

    init(client: any MobileClient) { self.client = client }

    func refresh() async {
        taskGeneration += 1
        let refreshing = taskGeneration
        isLoadingTasks = true
        defer { if refreshing == taskGeneration { isLoadingTasks = false } }
        do {
            let replacement = try await client.workingTasks()
            guard refreshing == taskGeneration else { return }
            tasks = replacement
            tasksError = nil
        } catch {
            guard refreshing == taskGeneration else { return }
            tasksError = error.localizedDescription
        }
    }

    func open(_ task: WorkViewPlan.Content) async {
        let sameSession = selected?.primarySession == task.primarySession
        disconnect()
        if !sameSession { render?(Data(), true) }
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
            state = .viewing
            eventTask = Task { [weak self] in
                for await event in link.events {
                    guard let self, self.generation == opening else { return }
                    self.receive(event)
                }
                // Explicit disconnects (including ended) invalidate this attachment's generation.
                guard let self, self.generation == opening else { return }
                self.fail("Connection closed; input was not retried")
            }
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
        do { try attachment?.requestControl() }
        catch { fail(error.localizedDescription) }
    }

    func send(_ bytes: Data) {
        guard state == .controlling, !replaying, !bytes.isEmpty else { return }
        do { try attachment?.send(bytes) }
        catch { fail("Input delivery is uncertain. Reconnect before typing again.") }
    }

    func resize(columns: Int, rows: Int) {
        guard columns > 0, rows > 0 else { return }
        viewport = (columns, rows)
        guard ownsGeometry else { return }
        do { try attachment?.resize(columns: columns, rows: rows) }
        catch { fail(error.localizedDescription) }
    }

    func suspend() {
        guard selected != nil else { return }
        switch state {
        case .loading, .viewing, .controlling: fail("Paused while the app is away")
        case .disconnected, .unavailable, .ended: break
        }
    }

    func leave() {
        disconnect()
        selected = nil
        render = nil
    }

    private func receive(_ event: MobileTerminalEvent) {
        switch event {
        case .snapshot(let data):
            replaying = true
            render?(data, true)
            replaying = false
        case .output(let data): render?(data, false)
        case .control(let input, let geometry):
            ownsGeometry = geometry
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
    }
}
