import Foundation
import Loopflow

enum PortfolioDiscovery {
    nonisolated static func resolveLaunchRepo(_ initialPath: String) -> String? {
        let scanner = RepositoryPath()
        let candidate = URL(fileURLWithPath: initialPath)
        guard let mainURL = scanner.mainRepository(candidate) else { return nil }
        return mainURL.normalizedFilePath
    }
}
