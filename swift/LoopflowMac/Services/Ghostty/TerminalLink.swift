import Foundation
import Loopflow
import UniformTypeIdentifiers

enum TerminalLink: Equatable {
    struct File: Equatable {
        let path: String
        let host: String?
        let line: Int?

        func isLocal(hostName: String = ProcessInfo.processInfo.hostName) -> Bool {
            guard let host, !host.isEmpty else { return true }
            return host.caseInsensitiveCompare("localhost") == .orderedSame
                || host.caseInsensitiveCompare(hostName) == .orderedSame
        }

        func checkoutPath(in checkout: String) -> String? {
            Self.relativePath(path, in: checkout)
        }

        private static func relativePath(_ path: String, in checkout: String) -> String? {
            guard checkout.hasPrefix("/") else { return nil }
            let root = checkout.split(separator: "/").filter { $0 != "." }
            let target = path.split(separator: "/").filter { $0 != "." }
            guard target.starts(with: root) else { return nil }
            return target.dropFirst(root.count).joined(separator: "/")
        }
    }

    case web(URL)
    case mail(URL)
    case work(URL)
    case file(File)
    case externalScheme(URL)
    case refused(String)

    init(target: String?, allowsBarePath: Bool = false) {
        guard let target, !target.isEmpty, !Self.hasUnsafeCharacters(target) else {
            self = .refused("Invalid terminal link.")
            return
        }
        if allowsBarePath, target.hasPrefix("/"), !target.hasPrefix("//") {
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
            self = .web(url)
        case "mailto":
            self = parts.host == nil && !parts.path.isEmpty ? .mail(url) : .refused("Invalid mail link.")
        case "file":
            guard target.dropFirst("file:".count).hasPrefix("//"), parts.user == nil,
                  parts.password == nil, parts.port == nil, parts.query == nil else {
                self = .refused("Invalid file link.")
                return
            }
            self = Self.fileLink(path: parts.path, host: parts.host, fragment: parts.fragment)
        case "loopflow":
            guard Self.isWorkAddress(target) else {
                self = .refused("Terminal links can only open Work in Loopflow.")
                return
            }
            self = .work(url)
        default:
            self = .externalScheme(url)
        }
    }

    /// What ⌘-hover shows: the target a click would open, or why it won't.
    static func preview(_ target: String?) -> String? {
        guard let target, !target.isEmpty else { return nil }
        if case .refused(let reason) = TerminalLink(target: target, allowsBarePath: true) { return reason }
        return target
    }

    private static func hasUnsafeCharacters(_ text: String) -> Bool {
        text.unicodeScalars.contains {
            CharacterSet.controlCharacters.contains($0)
                || $0.properties.isBidiControl
                || $0.value == 0x2028 || $0.value == 0x2029
        }
    }

    /// Check the address shape for hover feedback. The Work resolver validates
    /// identities and looks them up without dispatching app-control URLs.
    private static func isWorkAddress(_ target: String) -> Bool {
        func component(_ raw: String) -> Bool {
            guard let value = raw.removingPercentEncoding else { return false }
            return !value.isEmpty && !value.contains("/") && !value.contains("\\")
        }
        let prefix = "loopflow://"
        guard target.hasPrefix(prefix), !target.contains("#") else { return false }
        let address = target.dropFirst(prefix.count).split(separator: "?", maxSplits: 1, omittingEmptySubsequences: false)
        guard let root = address.first else { return false }
        let location = root.split(separator: "/", maxSplits: 1, omittingEmptySubsequences: false)
        guard let host = location.first, !host.contains("@"), !host.contains(":"), component(String(host)) else { return false }
        let path = location.dropFirst().first ?? ""
        let repository = path.isEmpty
        if !repository {
            let segments = path.split(separator: "/", omittingEmptySubsequences: false)
            guard segments.count == 2, ["task", "wave"].contains(segments[0]), component(String(segments[1])) else {
                return false
            }
        }
        var keys: Set<String> = []
        for pair in address.dropFirst().first?.split(separator: "&") ?? [] {
            let item = pair.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
            let key = String(item[0])
            guard item.count == 2, ["repository", "session", "machine"].contains(key),
                  keys.insert(key).inserted, component(String(item[1])) else { return false }
        }
        return !keys.contains("machine") || (repository && keys.contains("session"))
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

extension TerminalLink.File {
    enum Destination: Equatable {
        case document(URL)
        case checkout(path: String, line: Int?)
    }

    @MainActor
    func destination(from pane: WorkspaceIdentity, localMachine: String?, files: SessionsWorkspace.FilesPlace?,
                     query: RegistryQuery) async throws -> Destination {
        let local = pane.machineId == localMachine
        if local, !isLocal() { throw OpenError.anotherMachine }

        if let files, !files.cwd.isEmpty, let machine = files.machine ?? localMachine, pane.machineId == machine {
            if local {
                let checkout = URL(fileURLWithPath: files.cwd).resolvingSymlinksInPath().path
                let target = URL(fileURLWithPath: path).resolvingSymlinksInPath().path
                let paneCwd = pane.worktree.isEmpty ? checkout
                    : URL(fileURLWithPath: pane.worktree).resolvingSymlinksInPath().path
                if paneCwd == checkout,
                   let relative = Self.relativePath(target, in: checkout), !relative.isEmpty {
                    guard let kind = try? URL(fileURLWithPath: target).resourceValues(forKeys: [.isRegularFileKey, .isDirectoryKey]) else {
                        throw OpenError.missing
                    }
                    if kind.isRegularFile == true {
                        return .checkout(path: relative, line: line)
                    }
                    guard kind.isDirectory == true else { throw OpenError.unsafe }
                }
            } else if pane.worktree.isEmpty || pane.worktree == files.cwd,
                      let relative = checkoutPath(in: files.cwd) {
                guard !relative.isEmpty else { throw OpenError.remoteDirectory }
                // A directory read is resolved on the owning Machine, including
                // symlinked directories. Its non-directory response lets Files
                // perform the regular file read and its own containment check.
                do {
                    _ = try await query.taskFiles(issue: files.taskId, directory: relative, showIgnored: true)
                } catch let error as ServerError where error.code == .invalid && error.data == .object(["reason": .string("file")]) {
                    return .checkout(path: relative, line: line)
                }
                throw OpenError.remoteDirectory
            }
        }
        guard local else { throw OpenError.anotherMachine }
        return .document(try localDocument())
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

    /// Launch Services receives the resolved target after its type was checked.
    func localDocument() throws -> URL {
        let url = URL(fileURLWithPath: path).resolvingSymlinksInPath()
        guard let info = try? url.resourceValues(forKeys: [.isRegularFileKey, .isDirectoryKey,
                                                          .isPackageKey, .isAliasFileKey, .contentTypeKey]) else {
            throw OpenError.missing
        }
        guard info.isPackage != true, info.isAliasFile != true,
              !Self.launcherExtensions.contains(url.pathExtension.lowercased()) else { throw OpenError.unsafe }
        if info.isDirectory == true { return url }
        guard info.isRegularFile == true,
              !FileManager.default.isExecutableFile(atPath: url.path),
              info.contentType?.conforms(to: .executable) != true,
              info.contentType?.conforms(to: .script) != true else {
            throw OpenError.unsafe
        }
        guard let handle = try? FileHandle(forReadingFrom: url) else { throw OpenError.missing }
        defer { try? handle.close() }
        guard try handle.read(upToCount: 2) != Data("#!".utf8) else { throw OpenError.unsafe }
        return url
    }

    private static let launcherExtensions: Set<String> = [
        "app", "scpt", "scptd", "command", "sh", "bash", "fish", "zsh", "csh", "ksh", "tcsh",
        "py", "pyw", "rb", "pl", "jar", "tool", "workflow", "webloc", "inetloc", "url", "lnk", "desktop",
    ]
}
