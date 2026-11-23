import Loopflow
import SwiftUI

/// The work inspected by one Settings subscription. Save in can change independently.
struct SettingsWork: Equatable {
    var checkout: String?
    var wave: String?
    var target = ConfigurationTarget.general

    func scope(for definition: ConfigurationDefinition) -> Self {
        var work = self
        if definition.applicability == "repository" { work.wave = nil }
        if definition.applicability != "launch" { work.target = .general }
        return work
    }
}

@MainActor @Observable
final class ScopedSettingsModel {
    let location: ServerLocation
    var work = SettingsWork() {
        didSet {
            guard work != oldValue else { return }
            if work.checkout != oldValue.checkout || work.wave != oldValue.wave {
                if work.wave != nil { destination = .personalWave }
                else if work.checkout != nil { destination = .personalRepository }
                else { destination = .personalGlobal }
            }
            reading = .loading
            errors.removeAll()
            refresh()
        }
    }
    var destination = ConfigurationDestination.personalGlobal
    private(set) var observation = UUID()
    private(set) var reading = Reading<ConfigurationScope>.loading
    private(set) var catalogue: [ConfigurationDefinition] = []
    struct RowError {
        enum Recovery { case refresh, dismissMove }
        let message: String
        let recovery: Recovery?

        init(_ error: Error, move: MoveAction? = nil) {
            message = error.localizedDescription
            if (error as? ServerError)?.code == .conflict {
                if case .resume = move { recovery = .dismissMove } else { recovery = .refresh }
            } else { recovery = nil }
        }
    }
    private(set) var errors: [SettingDeclaration.ID: RowError] = [:]
    private(set) var saving = false
    @ObservationIgnored private var stream: LineSubscription?
    @ObservationIgnored private var checkpoint = 0
    @ObservationIgnored private var confirmations: [Int: CheckedContinuation<Void, Never>] = [:]

    init(location: ServerLocation = .shared) { self.location = location }

    var file: ConfigurationScope.File? { reading.value?.files.first { $0.owner == destination } }

    var editsDisabled: Bool { saving || reading.failure != nil }

    func select(checkout: String?, wave: String? = nil) {
        guard checkout != work.checkout || wave != work.wave else { return }
        work = SettingsWork(checkout: checkout, wave: wave)
    }

    func refresh() {
        observation = UUID()
        stream = nil
        finishConfirmations(through: .max)
    }

    private func confirmReading() async {
        guard let stream else { return }
        checkpoint += 1
        await withCheckedContinuation { continuation in
            confirmations[checkpoint] = continuation
            stream.checkpoint(checkpoint)
        }
    }

    private func finishConfirmations(through id: Int) {
        for key in confirmations.keys.filter({ $0 <= id }) {
            confirmations.removeValue(forKey: key)?.resume()
        }
    }

    func refresh(_ declaration: SettingDeclaration) async -> SettingDeclaration? {
        let id = observation
        await confirmReading()
        guard observation == id, let scope = reading.current else { return nil }
        errors[declaration.id] = nil
        return scope.current(declaration)
    }

    func observe() async {
        let id = observation
        var delay = Duration.seconds(1)
        while !Task.isCancelled, observation == id {
            var opened: LineSubscription?
            do {
                if catalogue.isEmpty || reading.failure != nil {
                    let definitions = try await ServerClient.configuration(.configCatalog, as: [ConfigurationDefinition].self, location: location)
                    guard !Task.isCancelled, observation == id else { return }
                    catalogue = definitions
                }
                let scope = ServedRead.configScope(directory: work.checkout, wave: work.wave, target: work.target)
                let subscription = ServerClient.subscribe(scope, location: location)
                opened = subscription
                stream = subscription
                for try await line in subscription.lines {
                    guard !Task.isCancelled, observation == id else { break }
                    let frame = try JSONDecoder().decode(SubscriptionFrame<ConfigurationScope>.self, from: line)
                    if let value = frame.body {
                        delay = .seconds(1)
                        reading = .available(value)
                        if destination == .personalWave && file == nil { destination = .sharedWave }
                    } else { reading = reading.failed(frame.unavailable ?? "Settings unavailable") }
                    if let answered = frame.answers { finishConfirmations(through: answered) }
                }
                throw RegistryQueryError("Settings disconnected")
            } catch {
                if !Task.isCancelled, observation == id { reading = reading.failed(error.localizedDescription) }
            }
            if observation == id {
                stream = nil
                finishConfirmations(through: .max)
            }
            await opened?.cancel()
            guard observation == id else { return }
            do { try await Task.sleep(for: delay) } catch { return }
            delay = min(delay * 2, .seconds(30))
        }
    }

    @discardableResult
    func apply(_ change: ConfigurationChange, to declaration: SettingDeclaration,
               notices: NoticeCenter, manager: UndoManager?) async -> Bool {
        guard !editsDisabled else { return false }
        saving = true
        defer { saving = false }
        let id = observation
        do {
            let key = declaration.catalogKey
            let address = declaration.id
            let receipt = try await ServerClient.edit(change, destination: address.destination, key: address.key,
                                                      expected: declaration.slot.entries, location: location)
            errors[declaration.id] = nil
            guard receipt.before != receipt.after else { return true }
            // A second click needs the new slot even if the subscription is between frames.
            if observation == id, let shown = reading.value?.current(declaration),
               shown.slot.entries != receipt.after {
                await confirmReading()
            }
            let title = title(for: key)
            let name = declaration.owner == .personalGlobal ? title : "\(title) in \(declaration.owner?.title ?? declaration.path)"
            let undo = Notice.Action.undo(name, manager: manager, target: self) { [weak notices] model in
                Task {
                    do {
                        _ = try await ServerClient.edit(.restore(receipt.before), destination: address.destination, key: address.key,
                                                        expected: receipt.after, location: model.location)
                        notices?.clear(key: key)
                    } catch {
                        model.report(error, id: id, declaration: declaration, notices: notices)
                    }
                }
            }
            let verb = receipt.after.isEmpty ? "Reset" : "Saved"
            notices.post(Notice("\(verb) \(name)", key: key, action: undo))
            return true
        } catch {
            report(error, id: id, declaration: declaration, notices: nil)
            return false
        }
    }

    func promote(_ action: MoveAction, for declaration: SettingDeclaration,
                 notices: NoticeCenter, manager: UndoManager?) async -> MoveReview? {
        guard !editsDisabled else {
            if case .confirm(let review) = action { return review }
            return nil
        }
        saving = true
        defer { saving = false }
        let id = observation
        let key = declaration.catalogKey
        let wave = work.wave
        do {
            let result = try await move(declaration.id, wave: wave, action: action)
            errors[declaration.id] = nil
            let title = title(for: key)
            switch result {
            case .preview(let review): return review
            case .pending: refresh()
            case .dismissed: notices.post(Notice("Dismissed pending move for \(title)", key: key))
            case .recovered: notices.post(Notice("Finished moving \(title)", key: key))
            case .applied(let receipt):
                let name = "Move \(title)"
                let undo = Notice.Action.undo(name, manager: manager, target: self) { [weak notices] model in
                    Task {
                        do {
                            switch try await model.move(declaration.id, wave: wave, action: .undo(receipt)) {
                            case .applied: notices?.clear(key: key)
                            default: notices?.post(Notice("Refresh to inspect the move in progress", key: key))
                            }
                        } catch { model.report(error, id: id, declaration: declaration, notices: notices) }
                    }
                }
                notices.post(Notice("Moved \(title)", key: key, action: undo))
            }
        } catch { report(error, id: id, declaration: declaration, notices: nil, move: action) }
        return nil
    }

    private func title(for key: String) -> String {
        catalogue.first { $0.key == key }?.title ?? key
    }

    private func move(_ address: SettingDeclaration.ID, wave: String?, action: MoveAction) async throws -> MoveResult {
        try await ServerClient.configuration(.promote(destination: address.destination, key: address.key, wave: wave, action: action),
                                             as: MoveResult.self, location: location)
    }

    func preview(_ change: ConfigurationChange, for declaration: SettingDeclaration) async throws -> String {
        try await ServerClient.configuration(.preview(destination: declaration.destination, key: declaration.slot.key,
            expected: declaration.slot.entries, change: change), as: String.self, location: location)
    }

    private func report(_ error: Error, id: UUID, declaration: SettingDeclaration, notices: NoticeCenter?, move: MoveAction? = nil) {
        if observation == id { errors[declaration.id] = RowError(error, move: move) }
        notices?.post(Notice(error.localizedDescription, key: declaration.catalogKey))
    }
}
