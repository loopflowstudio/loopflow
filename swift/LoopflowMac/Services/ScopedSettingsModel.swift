import Loopflow
import SwiftUI

@MainActor @Observable
final class ScopedSettingsModel {
    let location: ServerLocation
    private(set) var checkout: String?
    private(set) var wave: String?
    var target = ConfigurationTarget.general {
        didSet { if target != oldValue { reading = .loading; editError = nil; refresh() } }
    }
    var destination = ConfigurationDestination.personalGlobal
    private(set) var observation = UUID()
    private(set) var reading = Reading<ConfigurationScope>.loading
    private(set) var catalogue: [ConfigurationDefinition] = []
    private(set) var editError: String?
    private(set) var saving = false

    init(location: ServerLocation = .shared) { self.location = location }

    var file: ConfigurationScope.File? { reading.value?.files.first { $0.owner == destination } }

    func file(for pane: SettingsPane) -> ConfigurationScope.File? {
        pane == .general ? reading.value?.files.first { $0.owner == .personalGlobal } : file
    }

    var editsDisabled: Bool { saving || editError != nil || reading.failure != nil }

    func select(checkout: String?, wave: String? = nil) {
        guard self.checkout != checkout || self.wave != wave else { return }
        target = .general
        self.checkout = checkout
        self.wave = wave
        destination = wave != nil ? .personalWave : checkout != nil ? .personalRepository : .personalGlobal
        reading = .loading
        editError = nil
        refresh()
    }

    func refresh() { observation = UUID() }

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
        var first = true
        let stream = ServerClient.subscribe(.configScope(directory: checkout, wave: wave, target: target), location: location)
        do {
            for try await line in stream.lines {
                guard !Task.isCancelled, observation == id else { break }
                let frame = try JSONDecoder().decode(SubscriptionFrame<ConfigurationScope>.self, from: line)
                if let value = frame.body {
                    reading = .available(value)
                    if first {
                        editError = nil
                        if destination == .personalWave && file == nil { destination = .sharedWave }
                    }
                    first = false
                } else { reading = reading.failed(frame.unavailable ?? "Settings unavailable") }
            }
            if !Task.isCancelled, observation == id { reading = reading.failed("Settings disconnected") }
        } catch {
            if !Task.isCancelled, observation == id { reading = reading.failed(error.localizedDescription) }
        }
        await stream.cancel()
    }

    @discardableResult
    func apply(_ change: ConfigurationChange, key: String, to file: ConfigurationScope.File,
               notices: NoticeCenter, manager: UndoManager?) async -> Bool {
        guard !saving, let slot = file.slots[key] else { return false }
        saving = true
        defer { saving = false }
        let id = observation
        do {
            let destination = file.destination
            let address = slot.key
            let receipt = try await ServerClient.edit(change, destination: destination, key: address,
                                                      expected: slot.entries, location: location)
            guard receipt.before != receipt.after else { return true }
            let name = "\(key) in \(file.owner?.title ?? file.path)"
            let undo = Notice.Action.undo(name, manager: manager, target: self) { [weak notices] model in
                Task {
                    do {
                        _ = try await ServerClient.edit(.restore(receipt.before), destination: destination, key: address,
                                                        expected: receipt.after, location: model.location)
                        notices?.clear(key: key)
                    } catch {
                        model.report(error, id: id, key: key, notices: notices)
                    }
                }
            }
            notices.post(Notice("Saved \(name)", key: key, action: undo))
            return true
        } catch {
            report(error, id: id, key: key, notices: notices)
            return false
        }
    }

    func promote(_ action: MoveAction, key: String, file: ConfigurationScope.File,
                 notices: NoticeCenter, manager: UndoManager?) async -> MoveReview? {
        guard !saving else {
            if case .confirm(let review) = action { return review }
            return nil
        }
        saving = true
        defer { saving = false }
        let id = observation
        guard let address = file.slots[key]?.key else { return nil }
        let destination = file.destination
        let wave = self.wave
        do {
            switch try await move(address, destination: destination, wave: wave, action: action) {
            case .preview(let review): return review
            case .pending: refresh()
            case .recovered: notices.post(Notice("Finished moving \(key)", key: key))
            case .applied(let receipt):
                let name = "Move \(key)"
                let undo = Notice.Action.undo(name, manager: manager, target: self) { [weak notices] model in
                    Task {
                        do {
                            switch try await model.move(address, destination: destination, wave: wave, action: .undo(receipt)) {
                            case .applied: notices?.clear(key: key)
                            default: notices?.post(Notice("Refresh to inspect the move in progress", key: key))
                            }
                        } catch { model.report(error, id: id, key: key, notices: notices) }
                    }
                }
                notices.post(Notice("Moved \(key)", key: key, action: undo))
            }
        } catch { report(error, id: id, key: key, notices: notices) }
        return nil
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

    private func report(_ error: Error, id: UUID, key: String, notices: NoticeCenter?) {
        if observation == id { editError = error.localizedDescription }
        notices?.post(Notice(error.localizedDescription, key: key))
    }
}
