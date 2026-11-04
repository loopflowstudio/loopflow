import Foundation
import Testing
@testable import LoopflowMac

struct AppUpdatesTests {
    @Test func theAppFollowsTheStreamLfInstallRecorded() {
        #expect(AppUpdates.recordedStream(config: nil) == "stable")
        #expect(AppUpdates.recordedStream(config: "repo_root: /src\nstream: preview\n") == "preview")
        #expect(AppUpdates.feedURL(stream: "preview")?.lastPathComponent == "appcast-preview.xml")
        #expect(AppUpdates.feedURL(stream: "stable")?.lastPathComponent == "appcast-stable.xml")
    }

    @Test func devRunsNoUpdater() {
        #expect(AppUpdates.feedURL(stream: AppUpdates.recordedStream(config: "stream: dev\n")) == nil)
    }

    @Test func onlyTheInstalledAppUpdatesItself() {
        let home = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString)
        #expect(AppUpdates.isInstalled(URL(filePath: "/Applications/Loopflow.app"), environment: [:], home: home))
        #expect(!AppUpdates.isInstalled(URL(filePath: "/src/loopflow/local-bin/app.noindex/Loopflow.app"), environment: [:], home: home))
        #expect(AppUpdates.isInstalled(URL(filePath: "/tmp/apps/Loopflow.app"),
                                       environment: ["LF_APPLICATIONS_DIR": "/tmp/apps"], home: home))
    }

    @Test func customInstallationSurvivesLaunchWithoutTheInstallEnvironment() throws {
        let home = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: home) }
        let app = home.appending(path: "Custom Tools/Loopflow.app")
        let helper = app.appending(path: "Contents/MacOS/lf")
        let entry = home.appending(path: ".local/bin/lf")
        try FileManager.default.createDirectory(at: helper.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data().write(to: helper)
        try FileManager.default.createDirectory(at: entry.deletingLastPathComponent(), withIntermediateDirectories: true)
        try FileManager.default.createSymbolicLink(atPath: entry.path, withDestinationPath: "../../Custom Tools/Loopflow.app/Contents/MacOS/lf")
        #expect(AppUpdates.isInstalled(app, environment: [:], home: home))
        #expect(!AppUpdates.isInstalled(home.appending(path: "local-bin/app.noindex/Loopflow.app"), environment: [:], home: home))
        #expect(!AppUpdates.isInstalled(URL(filePath: "/Applications/Loopflow.app"), environment: [:], home: home))
        #expect(!AppUpdates.isInstalled(app, environment: ["LF_APPLICATIONS_DIR": "/tmp/other"], home: home))
    }

    @Test func aNewBundleVersionIsAdoptedOnce() {
        #expect(AppUpdates.needsAdoption(bundleVersion: "0.16.1", adopted: nil))
        #expect(AppUpdates.needsAdoption(bundleVersion: "0.16.1", adopted: "0.16.0"))
        #expect(!AppUpdates.needsAdoption(bundleVersion: "0.16.1", adopted: "0.16.1"))
    }
}
