#if os(macOS)
import Foundation
import Loopflow
import Observation

@MainActor @Observable
final class FilesBrowserState {
    struct Presentation: Equatable {
        let path: String
        var lines: ClosedRange<Int>?
        var mode = TaskFilesStore.Mode.read
    }

    enum Notice: Equatable {
        case presented(Presentation)
        case updated(String)

        var path: String {
            switch self {
            case .presented(let request): request.path
            case .updated(let path): path
            }
        }
    }

    var notice: Notice?
    var visibleReadPath: String? {
        didSet {
            guard let visibleReadPath else { return }
            updated.removeAll { $0 == visibleReadPath }
            if notice == .updated(visibleReadPath) { notice = nil }
        }
    }
    var showsRail: Bool?
    var hasChosenInitialFile = false
    var updated: [String] = []
    private(set) var presentations: [Presentation] = []
    func offer(_ request: Presentation) {
        presentations.removeAll { $0.path == request.path }
        presentations.append(request)
        notice = .presented(request)
    }
    let jumps = FileJumps()
    var visit: FileVisit?
    var documents: [String: TaskFileDocument] = [:]
    var tabs: [WorkFileTab] = []
    private var modes: [String: TaskFilesStore.Mode] = [:]
    private var automatic: [String] = []

    var selection: String? {
        didSet {
            if oldValue != selection {
                notice = nil
                visit = nil
                jumps.readingLine = 1
                jumps.dismiss()
            }
            guard let path = selection else { return }
            if notice?.path == path { notice = nil }
            hasChosenInitialFile = true
            if !tabs.contains(where: { $0.path == path }) {
                tabs.append(WorkFileTab(id: UUID().uuidString, path: path))
            }
            updated.removeAll { $0 == path }
            presentations.removeAll { $0.path == path }
        }
    }

    var mode: TaskFilesStore.Mode {
        get { selection.flatMap { modes[$0] } ?? .read }
        set {
            guard let selection else { return }
            if mode == .edit && newValue != .edit { notice = nil }
            if newValue != mode { jumps.dismiss() }
            modes[selection] = newValue == .read ? nil : newValue
        }
    }

    func open(_ path: String, automatically: Bool) {
        let wasAutomatic = automatic.contains(path)
        automatic.removeAll { $0 == path }
        if automatically, wasAutomatic || !tabs.contains(where: { $0.path == path }) { automatic.append(path) }
        selection = path
        let clean = automatic.filter { documents[$0]?.dirty != true && documents[$0]?.saving != true }
        for old in clean.dropLast(12) {
            if let tab = tabs.first(where: { $0.path == old }) { close(tab.id) }
        }
    }

    func close(_ id: String) {
        guard let index = tabs.firstIndex(where: { $0.id == id }) else { return }
        let path = tabs.remove(at: index).path
        automatic.removeAll { $0 == path }
        if selection == path { selection = tabs.isEmpty ? nil : tabs[min(index, tabs.count - 1)].path }
        if documents[path]?.dirty != true && documents[path]?.saving != true {
            documents[path] = nil
            modes[path] = nil
        }
    }

    func noteWrites(_ paths: Set<String>, present: Set<String>) {
        updated.removeAll { paths.contains($0) }
        let unread = present.filter { $0 != visibleReadPath }.sorted()
        updated.append(contentsOf: unread)
        if case .presented = notice { return }
        if let latest = unread.last { notice = .updated(latest) }
        else if case .updated(let path) = notice, paths.contains(path) { notice = nil }
    }
}
#endif
