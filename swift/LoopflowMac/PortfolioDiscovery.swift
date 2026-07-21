import Foundation

struct PortfolioLaunchRepo: Sendable {
    let path: String
    let mainPath: String
    let displayName: String
}

enum PortfolioDiscovery {
    nonisolated static func resolveLaunchRepo(_ initialPath: String) -> PortfolioLaunchRepo {
        let scanner = RepoScanner()
        let dev = ProcessInfo.processInfo.environment["LOOPFLOW_DEV_WAVE_REPO"]
        let devOverride = dev?.isEmpty == false ? dev : nil
        let readURL = URL(fileURLWithPath: devOverride ?? initialPath)
        let mainURL = scanner.resolveMainWorktree(readURL)
        let readPath = devOverride != nil ? readURL.normalizedFilePath : mainURL.normalizedFilePath
        return PortfolioLaunchRepo(
            path: readPath,
            mainPath: mainURL.normalizedFilePath,
            displayName: mainURL.lastPathComponent
        )
    }

    static func repos(
        initialRepoPath: String?,
        persistedRepos: [PortfolioRepo] = []
    ) async -> [PortfolioRepo] {
        await Task.detached {
            let scanner = RepoScanner()
            var seen = Set<String>()
            var result: [PortfolioRepo] = []
            for url in scanner.scanDefaultRoot() {
                let path = url.normalizedFilePath
                guard seen.insert(path).inserted else { continue }
                result.append(PortfolioRepo(path: path, lastOpened: Date()))
            }
            for repo in persistedRepos {
                let path = repo.path.normalizedFilePath
                guard seen.insert(path).inserted else { continue }
                result.append(repo)
            }
            if let initialRepoPath {
                let launch = resolveLaunchRepo(initialRepoPath)
                if launch.path == launch.mainPath {
                    if seen.insert(launch.path).inserted {
                        result.append(PortfolioRepo(
                            path: launch.path,
                            lastOpened: Date(),
                            displayNameOverride: launch.displayName
                        ))
                    }
                } else {
                    result.removeAll { $0.path == launch.mainPath || $0.path == launch.path }
                    result.append(PortfolioRepo(
                        path: launch.path,
                        lastOpened: Date(),
                        displayNameOverride: launch.displayName
                    ))
                }
            }
            return result
        }.value
    }

    static func authoredWaves(in repos: [PortfolioRepo]) async -> [String: [String]] {
        await Task.detached {
            var result: [String: [String]] = [:]
            for repo in repos {
                result[repo.path] = authoredWaves(inRepo: repo.path)
            }
            return result
        }.value
    }

    nonisolated static func authoredWaves(inRepo repoPath: String) -> [String] {
        let waveDir = URL(fileURLWithPath: repoPath).appendingPathComponent("wave", isDirectory: true)
        let fileManager = FileManager.default
        guard let children = try? fileManager.contentsOfDirectory(
            at: waveDir,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: [.skipsHiddenFiles]
        ) else {
            return []
        }
        return children
            .filter { url in
                var isDirectory = ObjCBool(false)
                let goal = url.appendingPathComponent("GOAL.md")
                return fileManager.fileExists(atPath: goal.path, isDirectory: &isDirectory)
                    && !isDirectory.boolValue
            }
            .map(\.lastPathComponent)
            .sorted { $0.localizedCaseInsensitiveCompare($1) == .orderedAscending }
    }
}
