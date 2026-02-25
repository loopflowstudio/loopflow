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
}
