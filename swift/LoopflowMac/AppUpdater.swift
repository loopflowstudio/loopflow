import Foundation
import Sparkle

/// The installed Loopflow.app updates itself through Sparkle from the stream
/// `~/.lf/config.yaml` records (`lf install --preview`), one appcast per stream,
/// and only to a release whose build can open this Machine's store. Dev builds,
/// test hosts and a Machine recording `dev` never update themselves.
enum AppUpdates {
    static let downloads = URL(string: "https://downloads.loopflow.studio")!

    /// The stream `lf install` recorded; `stable` when none is.
    static func recordedStream(config: String?) -> String {
        let recorded = config?.split(whereSeparator: \.isNewline).lazy.compactMap { line -> String? in
            guard line.hasPrefix("stream:") else { return nil }
            return line.dropFirst("stream:".count).trimmingCharacters(in: .whitespaces)
        }.first
        return recorded.flatMap { $0.isEmpty ? nil : $0 } ?? "stable"
    }

    /// dev has no appcast: `lf install --dev` links the CLI alone, and an app that
    /// followed preview would relink `lf` away from it on its next launch.
    static func feedURL(stream: String) -> URL? {
        switch stream {
        case "dev": nil
        case "preview": downloads.appending(path: "appcast-preview.xml")
        default: downloads.appending(path: "appcast-stable.xml")
        }
    }

    /// `/Applications/Loopflow.app` (or `LF_APPLICATIONS_DIR`'s), the bundle `lf install` manages.
    static func isInstalled(_ bundle: URL, environment: [String: String] = ProcessInfo.processInfo.environment) -> Bool {
        let applications = environment["LF_APPLICATIONS_DIR"] ?? "/Applications"
        return bundle.standardizedFileURL.path == URL(filePath: applications).appending(path: "Loopflow.app").standardizedFileURL.path
    }

    /// Whether this launch is the first of a bundle version `lf install adopt` hasn't recorded.
    static func needsAdoption(bundleVersion: String?, adopted: String?) -> Bool {
        bundleVersion != nil && bundleVersion != adopted
    }

    static let adoptedVersionKey = "adoptedBundleVersion"

    static func recordedStream(home: URL = FileManager.default.homeDirectoryForCurrentUser) -> String {
        recordedStream(config: try? String(contentsOf: home.appending(path: ".lf/config.yaml"), encoding: .utf8))
    }

    /// Run the bundled `lf` and throw its own message when it refuses.
    static func runLf(_ arguments: [String]) throws {
        let process = Process()
        process.executableURL = URL(filePath: try LocalWaveAgentLauncher.controlLfPath())
        process.arguments = arguments
        let errors = Pipe()
        process.standardError = errors
        process.standardOutput = FileHandle.nullDevice
        try process.run()
        let message = errors.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            let detail = String(decoding: message, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
            throw LocalLfError(errorDescription: detail.isEmpty ? "lf \(arguments.joined(separator: " ")) failed" : detail)
        }
    }

    /// After Sparkle replaced the bundle, migrate the store and link `lf` into it
    /// before anything reads the store. Runs once per bundle version.
    static func adoptIfUpdated(bundle: Bundle = .main, defaults: UserDefaults = .standard) {
        let version = bundle.object(forInfoDictionaryKey: "CFBundleVersion") as? String
        guard isInstalled(bundle.bundleURL),
              needsAdoption(bundleVersion: version, adopted: defaults.string(forKey: adoptedVersionKey)) else { return }
        do {
            try runLf(["install", "adopt"])
            defaults.set(version, forKey: adoptedVersionKey)
        } catch {
            DesktopLog.home.error("Adopting the updated app failed", ["error": error.localizedDescription])
        }
    }
}

/// Sparkle calls its delegate on the main thread.
final class AppUpdater: NSObject, SPUUpdaterDelegate, @unchecked Sendable {
    @MainActor static let shared = AppUpdater()
    private var controller: SPUStandardUpdaterController?

    @MainActor func start(bundle: Bundle = .main) {
        guard controller == nil, AppUpdates.isInstalled(bundle.bundleURL),
              AppUpdates.feedURL(stream: AppUpdates.recordedStream()) != nil else { return }
        controller = SPUStandardUpdaterController(startingUpdater: true, updaterDelegate: self, userDriverDelegate: nil)
    }

    /// `lf install` asks a running app to update (`loopflow://update`).
    @MainActor func checkForUpdates() {
        controller?.checkForUpdates(nil)
    }

    func feedURLString(for updater: SPUUpdater) -> String? {
        AppUpdates.feedURL(stream: AppUpdates.recordedStream())?.absoluteString
    }

    /// A switch to dev while the app runs stops its checks until the next launch.
    func updater(_ updater: SPUUpdater, mayPerform updateCheck: SPUUpdateCheck) throws {
        guard feedURLString(for: updater) != nil else {
            throw LocalLfError(errorDescription: "this Machine follows dev; `lf install --dev` updates the CLI alone")
        }
    }

    /// Veto a release whose build can't open the store; `lf` names when that changes.
    func updater(_ updater: SPUUpdater, shouldProceedWithUpdate item: SUAppcastItem,
                 updateCheck: SPUUpdateCheck) throws {
        try AppUpdates.runLf(["install", "can-open", "v\(item.versionString)"])
    }
}
