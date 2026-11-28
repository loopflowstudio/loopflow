import Foundation
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
            guard checkout.hasPrefix("/") else { return nil }
            // Compare the pane's paths without touching this Mac's filesystem.
            let root = checkout.split(separator: "/").filter { $0 != "." }
            let target = path.split(separator: "/").filter { $0 != "." }
            guard target.starts(with: root) else { return nil }
            let relative = target.dropFirst(root.count).joined(separator: "/")
            return relative.isEmpty ? "." : relative
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
            guard let host = parts.host, !host.isEmpty, parts.user == nil,
                  parts.password == nil, parts.port == nil, parts.fragment == nil,
                  !["open", "update", "portfolio", "sessions"].contains(host.lowercased()) else {
                self = .refused("Terminal links can only open Work in Loopflow.")
                return
            }
            self = .work(url)
        default:
            self = .externalScheme(url)
        }
    }

    static func hasUnsafeCharacters(_ text: String) -> Bool {
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
            guard fragment.hasPrefix("L"), digits.allSatisfy({ (48...57).contains($0) }),
                  let number = Int(fragment.dropFirst()), number > 0 else {
                return .refused("Use #L followed by a line number in a file link.")
            }
            line = number
        }
        return .file(File(path: path, host: host?.isEmpty == true ? nil : host, line: line))
    }
}

extension TerminalLink.File {
    enum OpenError: LocalizedError {
        case missing
        case unsafe

        var errorDescription: String? {
            switch self {
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
