import Foundation

struct CLIInstallManager {
    enum InstallError: LocalizedError {
        case missingBundledExecutable(String)
        case invalidInstallDirectory(String)

        var errorDescription: String? {
            switch self {
            case .missingBundledExecutable(let name):
                return "Missing bundled executable: \(name)"
            case .invalidInstallDirectory(let path):
                return "Install directory is not valid: \(path)"
            }
        }
    }

    static let defaultInstallDir = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".local/bin", isDirectory: true)
    static let systemInstallDir = URL(fileURLWithPath: "/usr/local/bin", isDirectory: true)

    private let fileManager: FileManager
    private let executableProvider: @Sendable (String) -> URL?

    init(
        fileManager: FileManager = .default,
        executableProvider: @escaping @Sendable (String) -> URL? = { name in
            Bundle.main.url(forAuxiliaryExecutable: name)
        }
    ) {
        self.fileManager = fileManager
        self.executableProvider = executableProvider
    }

    func install(to directory: URL) throws {
        guard !directory.path.isEmpty else {
            throw InstallError.invalidInstallDirectory(directory.path)
        }

        try fileManager.createDirectory(at: directory, withIntermediateDirectories: true)
        try installExecutable(named: "lf", into: directory)
        try installExecutable(named: "lfd", into: directory)
    }

    func uninstall(from directory: URL) throws {
        for binary in ["lf", "lfd"] {
            let linkURL = directory.appendingPathComponent(binary, isDirectory: false)
            if fileManager.fileExists(atPath: linkURL.path) {
                try fileManager.removeItem(at: linkURL)
            }
        }
    }

    func isInstalled(in directory: URL) -> Bool {
        for binary in ["lf", "lfd"] {
            guard let source = executableProvider(binary)?.resolvingSymlinksInPath() else { return false }
            let linkURL = directory.appendingPathComponent(binary, isDirectory: false)
            guard let destination = resolveSymlinkDestination(linkURL) else { return false }
            if destination.resolvingSymlinksInPath().path != source.path {
                return false
            }
        }
        return true
    }

    func pathExportLineIfNeeded(for directory: URL) -> String? {
        let pathValue = ProcessInfo.processInfo.environment["PATH"] ?? ""
        let directories = pathValue.split(separator: ":").map(String.init)
        if directories.contains(directory.path) {
            return nil
        }
        return "export PATH=\"\(directory.path):$PATH\""
    }

    private func installExecutable(named name: String, into directory: URL) throws {
        guard let sourceURL = executableProvider(name) else {
            throw InstallError.missingBundledExecutable(name)
        }

        let linkURL = directory.appendingPathComponent(name, isDirectory: false)
        if fileManager.fileExists(atPath: linkURL.path) {
            try fileManager.removeItem(at: linkURL)
        }

        try fileManager.createSymbolicLink(at: linkURL, withDestinationURL: sourceURL)
    }

    private func resolveSymlinkDestination(_ symlink: URL) -> URL? {
        guard fileManager.fileExists(atPath: symlink.path) else { return nil }

        guard let destinationPath = try? fileManager.destinationOfSymbolicLink(atPath: symlink.path) else {
            return nil
        }

        if destinationPath.hasPrefix("/") {
            return URL(fileURLWithPath: destinationPath, isDirectory: false)
        }

        let parent = symlink.deletingLastPathComponent()
        return parent.appendingPathComponent(destinationPath).standardizedFileURL
    }
}
