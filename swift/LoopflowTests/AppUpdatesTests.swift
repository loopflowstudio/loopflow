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
        #expect(AppUpdates.isInstalled(URL(filePath: "/Applications/Loopflow.app"), environment: [:]))
        #expect(!AppUpdates.isInstalled(URL(filePath: "/src/loopflow/local-bin/Loopflow.app"), environment: [:]))
        #expect(AppUpdates.isInstalled(URL(filePath: "/tmp/apps/Loopflow.app"),
                                       environment: ["LF_APPLICATIONS_DIR": "/tmp/apps"]))
    }

    @Test func aNewBundleVersionIsAdoptedOnce() {
        #expect(AppUpdates.needsAdoption(bundleVersion: "0.16.1", adopted: nil))
        #expect(AppUpdates.needsAdoption(bundleVersion: "0.16.1", adopted: "0.16.0"))
        #expect(!AppUpdates.needsAdoption(bundleVersion: "0.16.1", adopted: "0.16.1"))
    }
}
