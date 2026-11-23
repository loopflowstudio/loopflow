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
                catalogue = try JSONDecoder().decode([ConfigurationDefinition].self, from:
                    await ServerClient.configuration(.configCatalog, location: location))
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
            let receipt = try await write(change, key: slot.key, file: file, expected: slot.entries)
            guard receipt.before != receipt.after else { return true }
            let name = "\(key) in \(file.owner?.title ?? file.path)"
            let restore: @MainActor (ScopedSettingsModel) -> Void = { model in
                Task {
                    do {
                        _ = try await model.write(.restore(receipt.before), key: slot.key, file: file, expected: receipt.after)
                        notices.clear(key: key)
                    } catch {
                        model.report(error, id: id, key: key, notices: notices)
                    }
                }
            }
            manager?.registerUndo(withTarget: self, handler: restore)
            manager?.setActionName(name)
            notices.post(Notice("Saved \(name)", key: key, action: .init("Undo") { [weak manager, weak self] in
                if let manager, manager.undoActionName == name { manager.undo() }
                else if let self { restore(self) }
            }))
            return true
        } catch {
            report(error, id: id, key: key, notices: notices)
            return false
        }
    }

    func promote(key: String, file: ConfigurationScope.File, review: MoveReview?, resume: Bool,
                 notices: NoticeCenter, manager: UndoManager?) async -> MoveReview? {
        guard !saving else { return review }
        saving = true
        defer { saving = false }
        let id = observation
        guard let address = file.slots[key]?.key else { return nil }
        let destination = file.destination
        let wave = self.wave
        do {
            var action: [String: JSONValue] = ["kind": .string(resume ? "resume" : "preview")]
            if let review { action = ["kind": .string("confirm"), "review": try JSONValue(settingsValue: review)] }
            switch try await move(address, destination: destination, wave: wave, action: action) {
            case .preview(let review): return review
            case .pending: refresh()
            case .recovered: notices.post(Notice("Finished moving \(key)", key: key))
            case .applied(let receipt):
                let name = "Move \(key)"
                let restore: @MainActor (ScopedSettingsModel) -> Void = { model in
                    Task {
                        do {
                            let action: [String: JSONValue] = ["kind": .string("undo"), "receipt": try JSONValue(settingsValue: receipt)]
                            switch try await model.move(address, destination: destination, wave: wave, action: action) {
                            case .applied: notices.clear(key: key)
                            default: notices.post(Notice("Refresh to inspect the move in progress", key: key))
                            }
                        } catch { model.report(error, id: id, key: key, notices: notices) }
                    }
                }
                manager?.registerUndo(withTarget: self, handler: restore)
                manager?.setActionName(name)
                notices.post(Notice("Moved \(key)", key: key, action: .init("Undo") { [weak self, weak manager] in
                    if let manager, manager.undoActionName == name { manager.undo() }
                    else if let self { restore(self) }
                }))
            }
        } catch { report(error, id: id, key: key, notices: notices) }
        return nil
    }

    private func move(_ key: String, destination: JSONValue, wave: String?, action: [String: JSONValue]) async throws -> MoveResult {
        let data = try await ServerClient.configuration(.promote(destination: destination, key: key, wave: wave,
            action: .object(action)), location: location)
        return try JSONDecoder().decode(MoveResult.self, from: data)
    }

    func preview(_ change: ConfigurationChange, key: String, file: ConfigurationScope.File) async throws -> String {
        guard let slot = file.slots[key] else { throw RegistryQueryError("Choose a destination for this setting") }
        let data = try await ServerClient.configuration(.preview(destination: file.destination, key: slot.key,
            expected: slot.entries, change: change), location: location)
        return try JSONDecoder().decode(String.self, from: data)
    }

    private func write(_ change: ConfigurationChange, key: String, file: ConfigurationScope.File,
                       expected: [ConfigurationEntry]) async throws -> ConfigurationEditReceipt {
        let data = try await ServerClient.configuration(.edit(destination: file.destination, key: key,
            expected: expected, change: change), location: location)
        return try JSONDecoder().decode(ConfigurationEditReceipt.self, from: data)
    }

    private func report(_ error: Error, id: UUID, key: String, notices: NoticeCenter) {
        if observation == id { editError = error.localizedDescription }
        notices.post(Notice(error.localizedDescription, key: key))
    }
}
