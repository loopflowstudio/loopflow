import AppKit
import Loopflow
import SwiftUI

struct ConfigurationScope: Codable {
    let contextDiff: ConfigurationValue
    let files: [File]

    enum CodingKeys: String, CodingKey { case contextDiff = "context_diff", files }

    struct File: Codable, Identifiable {
        let destination: JSONValue
        let path: String
        let diff: [String: JSONValue]
        let resetPreview: String
        var id: String { path }

        enum CodingKeys: String, CodingKey { case destination, path, diff, resetPreview = "reset_preview" }

        var owner: ConfigurationDestination? {
            guard case .object(let fields) = destination, case .string(let kind) = fields["kind"] else { return nil }
            return ConfigurationDestination(rawValue: kind)
        }

        var declared: JSONValue? { diff["context.diff"] ?? diff["diff"] }
    }

    var source: String? {
        guard let selected = contextDiff.candidates.first(where: { $0.reason == "selected" }) else { return nil }
        if let file = files.first(where: { $0.path == selected.locator }) { return file.owner?.title }
        return selected.locator
    }
}

struct DiffEditReceipt: Codable {
    let before: [String: JSONValue]
    let after: [String: JSONValue]
}

@MainActor @Observable
final class ContextSettingsModel {
    let location: ServerLocation
    var checkout: String? {
        didSet {
            guard checkout != oldValue else { return }
            reading = .loading
            editError = nil
            destination = .personalRepository
            refresh()
        }
    }
    var destination = ConfigurationDestination.personalRepository
    private(set) var observation = UUID()
    private(set) var reading = Reading<ConfigurationScope>.loading
    private(set) var editError: String?
    private(set) var saving = false

    init(location: ServerLocation = .shared) { self.location = location }

    var file: ConfigurationScope.File? { reading.value?.files.first { $0.owner == destination } }

    func refresh() {
        observation = UUID()
    }

    func observe(_ directory: String) async {
        let id = observation
        var first = true
        let stream = ServerClient.subscribe(.configScope(directory: directory), location: location)
        do {
            for try await line in stream.lines {
                guard !Task.isCancelled, observation == id else { break }
                let frame = try JSONDecoder().decode(SubscriptionFrame<ConfigurationScope>.self, from: line)
                if let value = frame.body {
                    reading = .available(value)
                    if first { editError = nil }
                    first = false
                }
                else { reading = reading.failed(frame.unavailable ?? "Checkout settings are unavailable") }
            }
            if !Task.isCancelled, observation == id { reading = reading.failed("Settings disconnected") }
        } catch {
            if !Task.isCancelled, observation == id { reading = reading.failed(error.localizedDescription) }
        }
        await stream.cancel()
    }

    @discardableResult
    func apply(_ change: DiffChange, to file: ConfigurationScope.File, notices: NoticeCenter, manager: UndoManager?) async -> Bool {
        guard !saving else { return false }
        saving = true
        defer { saving = false }
        let directory = checkout
        do {
            let receipt = try await write(change, destination: file.destination, expected: file.diff)
            guard receipt.before != receipt.after else { return true }
            let name = "Diff in \(file.owner?.title ?? file.path)"
            let restore: @MainActor (ContextSettingsModel) -> Void = { model in
                Task {
                    do {
                        _ = try await model.write(.restore(receipt.before), destination: file.destination, expected: receipt.after)
                        notices.clear(key: "context.diff")
                    } catch {
                        model.report(error, directory: directory, notices: notices)
                    }
                }
            }
            manager?.registerUndo(withTarget: self, handler: restore)
            manager?.setActionName(name)
            notices.post(Notice("Saved \(name)", key: "context.diff", action: .init("Undo") { [weak manager, weak self] in
                if let manager, manager.undoActionName == name { manager.undo() }
                else if let self { restore(self) }
            }))
            return true
        } catch {
            report(error, directory: directory, notices: notices)
            return false
        }
    }

    private func write(_ change: DiffChange, destination: JSONValue, expected: [String: JSONValue]) async throws -> DiffEditReceipt {
        let data = try await ServerClient.configuration(.editDiff(destination: destination, expected: expected, change: change), location: location)
        return try JSONDecoder().decode(DiffEditReceipt.self, from: data)
    }

    private func report(_ error: Error, directory: String?, notices: NoticeCenter) {
        if checkout == directory { editError = error.localizedDescription }
        notices.post(Notice(error.localizedDescription, key: "context.diff"))
    }
}

struct ContextSettings: View {
    @Bindable var model: ContextSettingsModel
    let notices: NoticeCenter
    @Environment(\.undoManager) private var undoManager
    @State private var reset: ConfigurationScope.File?

    var body: some View {
        Form {
            Section {
                LabeledContent("Applies to") {
                    if let checkout = model.checkout {
                        Text(checkout).textSelection(.enabled)
                    }
                    Button("Choose Checkout…") {
                        let panel = NSOpenPanel()
                        panel.canChooseDirectories = true
                        panel.canChooseFiles = false
                        if panel.runModal() == .OK { model.checkout = panel.url?.path }
                    }
                }
                if model.checkout != nil {
                    Picker("Save in", selection: $model.destination) {
                        Text("My settings").tag(ConfigurationDestination.personalRepository)
                        Text("Shared settings").tag(ConfigurationDestination.sharedRepository)
                    }
                    .pickerStyle(.segmented)
                }
            }
            if let scope = model.reading.value, let file = model.file {
                Section {
                    Toggle("Include diff in agent context", isOn: Binding(
                        get: { (file.declared ?? scope.contextDiff.value) == .bool(true) },
                        set: { apply(.set($0), to: file) }
                    ))
                    .toggleStyle(.switch)
                    DisclosureGroup {
                        ForEach(Array(scope.contextDiff.candidates.enumerated()), id: \.offset) { _, candidate in
                            LabeledContent(candidate.locator) {
                                Text("\(candidate.value == .bool(true) ? "On" : "Off") · \(candidate.reason)")
                            }
                            .textSelection(.enabled)
                        }
                    } label: {
                        LabeledContent(file.diff.isEmpty ? "Inherited" : "In effect") {
                            Text("\(scope.contextDiff.value == .bool(true) ? "On" : "Off") · \(scope.source ?? "Unknown source")")
                        }
                    }
                    if !file.diff.isEmpty {
                        if let reset {
                            Text(reset.resetPreview).font(Typography.mono).textSelection(.enabled)
                            HStack {
                                Button("Cancel") { self.reset = nil }
                                Button("Reset in this file") {
                                    Task {
                                        if await model.apply(.reset, to: reset, notices: notices, manager: undoManager) {
                                            self.reset = nil
                                        }
                                    }
                                }
                            }
                        } else {
                            Button("Reset layer…") { reset = file }
                        }
                    }
                } footer: {
                    Text(file.path).textSelection(.enabled)
                }
                .disabled(model.saving || model.editError != nil || model.reading.failure != nil)
            } else if model.checkout != nil && model.reading.isLoading {
                ProgressView("Loading Context settings")
            }
            if let failure = model.editError ?? model.reading.failure {
                Section {
                    Text(failure).textSelection(.enabled)
                    Button("Refresh") { reset = nil; model.refresh() }
                }
            }
        }
        .formStyle(.grouped)
        .onChange(of: model.destination) { reset = nil }
        .onChange(of: model.checkout) { reset = nil }
    }

    private func apply(_ change: DiffChange, to file: ConfigurationScope.File) {
        Task { await model.apply(change, to: file, notices: notices, manager: undoManager) }
    }
}
