import Foundation
import Loopflow
import UniformTypeIdentifiers

enum TerminalHyperlink: Equatable {
    struct File: Equatable {
        let path: String
        let host: String?
        let line: Int?

        func isLocal(hostName: String = ProcessInfo.processInfo.hostName) -> Bool {
            guard let host, !host.isEmpty else { return true }
            return host.caseInsensitiveCompare("localhost") == .orderedSame
                || host.caseInsensitiveCompare(hostName) == .orderedSame
        }
    }

    case defaultApp(URL)
    case work(URL)
    case file(File)
    case externalScheme(URL)
    case refused(String)

    init(target: String?) {
        guard let target, !target.isEmpty, !Self.hasUnsafeCharacters(target) else {
            self = .refused("Invalid terminal link.")
            return
        }
        if target.hasPrefix("/"), !target.hasPrefix("//") {
            let parts = target.split(separator: "#", maxSplits: 1, omittingEmptySubsequences: false)
            self = Self.fileLink(path: String(parts[0]), host: nil, fragment: parts.dropFirst().first.map(String.init))
            return
        }
        guard let decoded = target.removingPercentEncoding, !Self.hasUnsafeCharacters(decoded),
              !target.contains("\\"),
              let url = URL(string: target, encodingInvalidCharacters: false),
              let parts = URLComponents(url: url, resolvingAgainstBaseURL: false),
              let scheme = parts.scheme?.lowercased() else {
            self = .refused("Invalid terminal link.")
            return
        }
        switch scheme {
        case "http", "https":
            guard let host = parts.host, !host.isEmpty, parts.user == nil, parts.password == nil else {
                self = .refused("Invalid web link.")
                return
            }
            self = .defaultApp(url)
        case "mailto":
            self = parts.host == nil && !parts.path.isEmpty ? .defaultApp(url) : .refused("Invalid mail link.")
        case "file":
            guard target.dropFirst("file:".count).hasPrefix("//"), parts.user == nil,
                  parts.password == nil, parts.port == nil, parts.query == nil else {
                self = .refused("Invalid file link.")
                return
            }
            self = Self.fileLink(path: parts.path, host: parts.host, fragment: parts.fragment)
        case "loopflow":
            self = .work(url)
        default:
            self = .externalScheme(url)
        }
    }

    var refusal: String? {
        if case .refused(let reason) = self { return reason }
        return nil
    }

    private static func hasUnsafeCharacters(_ text: String) -> Bool {
        text.unicodeScalars.contains {
            CharacterSet.controlCharacters.contains($0)
                || $0.properties.isBidiControl
                || $0.value == 0x2028 || $0.value == 0x2029
        }
    }

    private static func fileLink(path: String, host: String?, fragment: String?) -> Self {
        guard path.hasPrefix("/"), !path.hasPrefix("//") else {
            return .refused("File links need an absolute path.")
        }
        guard !path.split(separator: "/").contains("..") else {
            return .refused("File links need a path without parent-directory segments.")
        }
        var line: Int?
        if let fragment {
            let digits = fragment.utf8.dropFirst()
            // The source reader highlights the line in a Foundation IndexSet.
            guard fragment.hasPrefix("L"), digits.allSatisfy({ (48...57).contains($0) }),
                  let number = Int(fragment.dropFirst()), (1..<NSNotFound).contains(number) else {
                return .refused("Use #L followed by a valid line number in a file link.")
            }
            line = number
        }
        return .file(File(path: path, host: host?.isEmpty == true ? nil : host, line: line))
    }
}

extension TerminalHyperlink.File {
    enum Destination: Equatable {
        case document(URL)
        case checkout(path: String, snapshot: TaskFileSnapshot? = nil)
    }

    @MainActor
    func destination(from pane: WorkspaceIdentity, localMachine: String?, files: SessionsWorkspace.FilesPlace?,
                     query: RegistryQuery) async throws -> Destination {
        if pane.machineId == localMachine {
            guard isLocal() else { throw OpenError.anotherMachine }
            return try localDestination(from: pane.worktree, checkout: files?.machine == nil ? files?.cwd : nil)
        }

        // Remote paths belong to the Task's Machine. Never resolve them on this Mac.
        guard let files, files.machine == pane.machineId, pane.worktree.isEmpty || pane.worktree == files.cwd,
              let relative = Self.relativePath(path, in: files.cwd) else { throw OpenError.anotherMachine }
        guard !relative.isEmpty else { throw OpenError.remoteDirectory }
        do {
            let snapshot = try await query.taskFile(issue: files.taskId, path: relative)
            guard snapshot.state != .missing else { throw OpenError.missing }
            return .checkout(path: snapshot.path, snapshot: snapshot)
        } catch let error as ServerError where error.code == .invalid && error.data == .object(["reason": .string("directory")]) {
            throw OpenError.remoteDirectory
        }
    }

    private static func relativePath(_ path: String, in checkout: String) -> String? {
        guard checkout.hasPrefix("/") else { return nil }
        let root = checkout.split(separator: "/").filter { $0 != "." }
        let target = path.split(separator: "/").filter { $0 != "." }
        guard target.starts(with: root) else { return nil }
        return target.dropFirst(root.count).joined(separator: "/")
    }

    enum OpenError: LocalizedError {
        case anotherMachine
        case remoteDirectory
        case missing
        case unsafe

        var errorDescription: String? {
            switch self {
            case .anotherMachine: "File is on another Machine."
            case .remoteDirectory: "Folders on another Machine cannot open from a terminal link."
            case .missing: "File is missing or unreadable."
            case .unsafe: "Terminal links cannot open apps, scripts, or executable files."
            }
        }
    }

    private func localDestination(from worktree: String, checkout: String?) throws -> Destination {
        func resolved(_ path: String) -> URL { URL(fileURLWithPath: path).resolvingSymlinksInPath() }
        let url = resolved(path)
        guard let info = try? url.resourceValues(forKeys: [.isRegularFileKey, .isDirectoryKey,
                                                          .isPackageKey, .isAliasFileKey, .contentTypeKey]) else {
            throw OpenError.missing
        }
        if info.isRegularFile == true, let checkout, !checkout.isEmpty {
            let root = resolved(checkout).path
            if worktree.isEmpty || resolved(worktree).path == root,
               let relative = Self.relativePath(url.path, in: root), !relative.isEmpty {
                return .checkout(path: relative)
            }
        }

        // Launch Services receives only the resolved document or folder we inspected.
        guard info.isPackage != true, info.isAliasFile != true,
              !Self.launcherExtensions.contains(url.pathExtension.lowercased()) else { throw OpenError.unsafe }
        if info.isDirectory == true { return .document(url) }
        guard info.isRegularFile == true,
              !FileManager.default.isExecutableFile(atPath: url.path),
              info.contentType?.conforms(to: .executable) != true,
              info.contentType?.conforms(to: .script) != true else {
            throw OpenError.unsafe
        }
        guard let handle = try? FileHandle(forReadingFrom: url) else { throw OpenError.missing }
        defer { try? handle.close() }
        guard try handle.read(upToCount: 2) != Data("#!".utf8) else { throw OpenError.unsafe }
        return .document(url)
    }

    private static let launcherExtensions: Set<String> = [
        "app", "scpt", "scptd", "command", "sh", "bash", "fish", "zsh", "csh", "ksh", "tcsh",
        "py", "pyw", "rb", "pl", "jar", "tool", "workflow", "webloc", "inetloc", "url", "lnk", "desktop",
    ]
}
