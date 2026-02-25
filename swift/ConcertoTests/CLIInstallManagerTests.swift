import Foundation
import Testing
@testable import Concerto

@Suite("CLIInstallManager")
struct CLIInstallManagerTests {
    @Test("installs and uninstalls lf and lfd symlinks")
    func installsAndUninstallsSymlinks() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let sourceDir = root.appendingPathComponent("source", isDirectory: true)
        let installDir = root.appendingPathComponent("bin", isDirectory: true)

        try FileManager.default.createDirectory(at: sourceDir, withIntermediateDirectories: true)
        try "lf".write(to: sourceDir.appendingPathComponent("lf"), atomically: true, encoding: .utf8)
        try "lfd".write(to: sourceDir.appendingPathComponent("lfd"), atomically: true, encoding: .utf8)

        let manager = CLIInstallManager(
            executableProvider: { name in
                sourceDir.appendingPathComponent(name, isDirectory: false)
            }
        )

        try manager.install(to: installDir)
        #expect(manager.isInstalled(in: installDir))

        try manager.uninstall(from: installDir)
        #expect(!manager.isInstalled(in: installDir))

        try? FileManager.default.removeItem(at: root)
    }

    @Test("install refuses to overwrite non-symlink binaries")
    func installRefusesConflictingFiles() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let sourceDir = root.appendingPathComponent("source", isDirectory: true)
        let installDir = root.appendingPathComponent("bin", isDirectory: true)

        try FileManager.default.createDirectory(at: sourceDir, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: installDir, withIntermediateDirectories: true)
        try "lf".write(to: sourceDir.appendingPathComponent("lf"), atomically: true, encoding: .utf8)
        try "lfd".write(to: sourceDir.appendingPathComponent("lfd"), atomically: true, encoding: .utf8)
        try "existing".write(to: installDir.appendingPathComponent("lf"), atomically: true, encoding: .utf8)

        let manager = CLIInstallManager(
            executableProvider: { name in
                sourceDir.appendingPathComponent(name, isDirectory: false)
            }
        )

        do {
            try manager.install(to: installDir)
            Issue.record("Expected install to fail when target binary already exists")
        } catch CLIInstallManager.InstallError.existingFileConflict {
            // Expected path.
        } catch {
            Issue.record("Unexpected error: \(error.localizedDescription)")
        }
        #expect(!manager.isInstalled(in: installDir))

        try? FileManager.default.removeItem(at: root)
    }

    @Test("uninstall only removes symlinks managed by Concerto")
    func uninstallLeavesUnmanagedLinks() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let sourceDir = root.appendingPathComponent("source", isDirectory: true)
        let otherDir = root.appendingPathComponent("other", isDirectory: true)
        let installDir = root.appendingPathComponent("bin", isDirectory: true)

        try FileManager.default.createDirectory(at: sourceDir, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: otherDir, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: installDir, withIntermediateDirectories: true)
        try "lf".write(to: sourceDir.appendingPathComponent("lf"), atomically: true, encoding: .utf8)
        try "lfd".write(to: sourceDir.appendingPathComponent("lfd"), atomically: true, encoding: .utf8)
        try "other".write(to: otherDir.appendingPathComponent("lfd"), atomically: true, encoding: .utf8)

        let manager = CLIInstallManager(
            executableProvider: { name in
                sourceDir.appendingPathComponent(name, isDirectory: false)
            }
        )

        try manager.install(to: installDir)
        let managedLf = installDir.appendingPathComponent("lf")
        let unmanagedLfd = installDir.appendingPathComponent("lfd")
        try FileManager.default.removeItem(at: unmanagedLfd)
        try FileManager.default.createSymbolicLink(
            at: unmanagedLfd,
            withDestinationURL: otherDir.appendingPathComponent("lfd")
        )

        try manager.uninstall(from: installDir)
        #expect(!FileManager.default.fileExists(atPath: managedLf.path))
        #expect(FileManager.default.fileExists(atPath: unmanagedLfd.path))

        try? FileManager.default.removeItem(at: root)
    }
}
