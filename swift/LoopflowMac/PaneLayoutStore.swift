// PaneLayoutStore — the pane layouts this Mac keeps for each Work, one JSON
// file per Work: `<desktop-cache>/pane-layouts/<machine>/<kind>/<id>.json`.
//
// The layout belongs to the Mac showing it, not the Machine it drives: two Macs
// driving one Machine each keep their own, and saving one wakes no server reader.

import Foundation
import Loopflow

struct PaneLayoutStore: Sendable {
    let directory: URL

    static let home = PaneLayoutStore(directory: WorkCache.homeDirectory.appendingPathComponent("pane-layouts", isDirectory: true))

    func url(machineId: String, work: WorkWorkspaceKey) -> URL {
        directory.appendingPathComponent(Self.component(machineId), isDirectory: true)
            .appendingPathComponent(work.kind.rawValue, isDirectory: true)
            .appendingPathComponent(Self.component(work.id) + ".json", isDirectory: false)
    }

    /// The saved layout, or `nil` when absent or unreadable. An unreadable file
    /// is removed so the next save starts clean.
    func load(machineId: String, work: WorkWorkspaceKey) -> WorkPaneLayout? {
        let url = url(machineId: machineId, work: work)
        guard let data = try? Data(contentsOf: url) else { return nil }
        guard let layout = try? JSONDecoder().decode(WorkPaneLayout.self, from: data) else {
            try? FileManager.default.removeItem(at: url)
            return nil
        }
        return layout
    }

    func save(_ layout: WorkPaneLayout, machineId: String, work: WorkWorkspaceKey) throws {
        let url = url(machineId: machineId, work: work)
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try JSONEncoder().encode(layout).write(to: url, options: .atomic)
    }

    /// Identities are structured, but a path component must never climb or nest.
    private static func component(_ value: String) -> String {
        value.addingPercentEncoding(withAllowedCharacters: .alphanumerics.union(CharacterSet(charactersIn: "-_"))) ?? value
    }
}
