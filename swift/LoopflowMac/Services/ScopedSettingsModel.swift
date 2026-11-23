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
    struct RowAddress: Hashable {
        let destination: JSONValue
        let key: String

        init(_ file: ConfigurationScope.File, _ key: String) {
            destination = file.destination
            self.key = file.slots[key]?.key ?? key
        }
    }

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
    private(set) var errors: [RowAddress: RowError] = [:]
    private(set) var saving = false
    @ObservationIgnored private var stream: LineSubscription?
    @ObservationIgnored private var checkpoint = 0
    @ObservationIgnored private var confirmations: [Int: CheckedContinuation<Void, Never>] = [:]

    init(location: ServerLocation = .shared) { self.location = location }

    var file: ConfigurationScope.File? { reading.value?.files.first { $0.owner == destination } }

    func editableFile(for definition: ConfigurationDefinition) -> ConfigurationScope.File? {
        guard let file, file.slots[definition.key] != nil else { return nil }
        return file
    }

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

    func refresh(key: String, file: ConfigurationScope.File) async -> ConfigurationScope.File? {
        let id = observation
        await confirmReading()
        guard observation == id, let scope = reading.current else { return nil }
        errors[RowAddress(file, key)] = nil
        return scope.files.first { $0.destination == file.destination }
    }

    func observe() async {
        let id = observation
        if catalogue.isEmpty {
            do {
                catalogue = try await ServerClient.configuration(.configCatalog, as: [ConfigurationDefinition].self, location: location)
            } catch {
                if observation == id { reading = reading.failed(error.localizedDescription) }
                return
            }
        }
        guard !Task.isCancelled, observation == id else { return }
        let stream = ServerClient.subscribe(.configScope(directory: work.checkout, wave: work.wave, target: work.target), location: location)
        self.stream = stream
        do {
            for try await line in stream.lines {
                guard !Task.isCancelled, observation == id else { break }
                let frame = try JSONDecoder().decode(SubscriptionFrame<ConfigurationScope>.self, from: line)
                if let value = frame.body {
                    reading = .available(value)
                    if destination == .personalWave && file == nil { destination = .sharedWave }
                } else { reading = reading.failed(frame.unavailable ?? "Settings unavailable") }
                if let answered = frame.answers { finishConfirmations(through: answered) }
            }
            if !Task.isCancelled, observation == id { reading = reading.failed("Settings disconnected") }
        } catch {
            if !Task.isCancelled, observation == id { reading = reading.failed(error.localizedDescription) }
        }
        if observation == id {
            self.stream = nil
            finishConfirmations(through: .max)
        }
        await stream.cancel()
    }

    @discardableResult
    func apply(_ change: ConfigurationChange, key: String, to file: ConfigurationScope.File,
               notices: NoticeCenter, manager: UndoManager?) async -> Bool {
        guard !editsDisabled, let slot = file.slots[key] else { return false }
        saving = true
        defer { saving = false }
        let id = observation
        do {
            let destination = file.destination
            let address = slot.key
            let receipt = try await ServerClient.edit(change, destination: destination, key: address,
                                                      expected: slot.entries, location: location)
            errors[RowAddress(file, key)] = nil
            guard receipt.before != receipt.after else { return true }
            // A second click needs the new slot even if the subscription is between frames.
            if observation == id, let shown = reading.value?.files.first(where: { $0.id == file.id })?.slots[key],
               shown.key == address, shown.entries != receipt.after {
                await confirmReading()
            }
            let title = title(for: key)
            let name = file.owner == .personalGlobal ? title : "\(title) in \(file.owner?.title ?? file.path)"
            let undo = Notice.Action.undo(name, manager: manager, target: self) { [weak notices] model in
                Task {
                    do {
                        _ = try await ServerClient.edit(.restore(receipt.before), destination: destination, key: address,
                                                        expected: receipt.after, location: model.location)
                        notices?.clear(key: key)
                    } catch {
                        model.report(error, id: id, key: key, file: file, notices: notices)
                    }
                }
            }
            let verb = receipt.after.isEmpty ? "Reset" : "Saved"
            notices.post(Notice("\(verb) \(name)", key: key, action: undo))
            return true
        } catch {
            report(error, id: id, key: key, file: file, notices: nil)
            return false
        }
    }

    func promote(_ action: MoveAction, key: String, file: ConfigurationScope.File,
                 notices: NoticeCenter, manager: UndoManager?) async -> MoveReview? {
        guard !editsDisabled else {
            if case .confirm(let review) = action { return review }
            return nil
        }
        saving = true
        defer { saving = false }
        let id = observation
        guard let address = file.slots[key]?.key else { return nil }
        let destination = file.destination
        let wave = work.wave
        do {
            let result = try await move(address, destination: destination, wave: wave, action: action)
            errors[RowAddress(file, key)] = nil
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
                            switch try await model.move(address, destination: destination, wave: wave, action: .undo(receipt)) {
                            case .applied: notices?.clear(key: key)
                            default: notices?.post(Notice("Refresh to inspect the move in progress", key: key))
                            }
                        } catch { model.report(error, id: id, key: key, file: file, notices: notices) }
                    }
                }
                notices.post(Notice("Moved \(title)", key: key, action: undo))
            }
        } catch { report(error, id: id, key: key, file: file, notices: nil, move: action) }
        return nil
    }

    private func title(for key: String) -> String {
        catalogue.first { $0.key == key }?.title ?? key
    }

    private func move(_ key: String, destination: JSONValue, wave: String?, action: MoveAction) async throws -> MoveResult {
        try await ServerClient.configuration(.promote(destination: destination, key: key, wave: wave, action: action),
                                             as: MoveResult.self, location: location)
    }

    func preview(_ change: ConfigurationChange, key: String, file: ConfigurationScope.File) async throws -> String {
        guard let slot = file.slots[key] else { throw RegistryQueryError("Choose a destination for this setting") }
        return try await ServerClient.configuration(.preview(destination: file.destination, key: slot.key,
            expected: slot.entries, change: change), as: String.self, location: location)
    }

    private func report(_ error: Error, id: UUID, key: String, file: ConfigurationScope.File, notices: NoticeCenter?, move: MoveAction? = nil) {
        if observation == id { errors[RowAddress(file, key)] = RowError(error, move: move) }
        notices?.post(Notice(error.localizedDescription, key: key))
    }
}
