import Foundation
import Loopflow

@MainActor
enum ControlRoomFixture {
    static func applyIfRequested(to model: ControlRoomModel, sourceFile: String = #filePath) {
        guard let mode = AppTestMode.current(), mode != .live else { return }
        do {
            switch mode {
            case .emptyWorkspaces:
                let snapshot = try JSONDecoder().decode(
                    RoadmapSnapshot.self,
                    from: Data(#"{"generated_at":"2026-07-20T00:00:00Z","waves":[]}"#.utf8)
                )
                model.applyFixture(
                    roadmap: .available(snapshot),
                    waves: .available([]),
                    repos: [],
                    fixed: true
                )
            case .mockWaves:
                let fixture = try loadRoadmap(sourceFile: sourceFile)
                let reading: ControlRoomReading<RoadmapSnapshot>
                switch MockWaveFixture.detailState {
                case .selected:
                    reading = .available(fixture.roadmap)
                case .loading:
                    reading = .loading
                case .error:
                    reading = .unavailable(
                        lastGood: nil,
                        reason: "the local registry is unreachable"
                    )
                }
                model.applyFixture(
                    roadmap: reading,
                    waves: .available(fixture.waves),
                    repos: fixture.repos,
                    fixed: true
                )
                let requested = AppTestMode.selectBranch ?? fixture.roadmap.waves.first?.wave.name
                if let wave = fixture.roadmap.waves.first(where: { $0.wave.name == requested }) {
                    model.select(.wave(waveId: wave.wave.id))
                }
            case .live:
                break
            }
        } catch {
            model.applyFixture(
                roadmap: .unavailable(
                    lastGood: nil,
                    reason: "Control-room fixture unavailable: \(error.localizedDescription)"
                ),
                waves: .unavailable(lastGood: nil, reason: error.localizedDescription),
                repos: [],
                fixed: true
            )
        }
    }

    private static func loadRoadmap(
        sourceFile: String
    ) throws -> (roadmap: RoadmapSnapshot, waves: [Wave], repos: [PortfolioRepo]) {
        let url = try roadmapFixtureURL(sourceFile: sourceFile)
        let data = try Data(contentsOf: url)
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: data)
        let waves = roadmap.waves.map { snapshot in
            Wave(
                id: snapshot.wave.id,
                name: snapshot.wave.name,
                repo: snapshot.wave.repo,
                status: snapshot.wave.status,
                live: snapshot.wave.live,
                activeTasks: snapshot.wave.activeTasks,
                activeProjects: snapshot.wave.activeProjects,
                parentWaveId: snapshot.wave.parentWaveId
            )
        }
        var seen = Set<String>()
        let repos = roadmap.waves.compactMap { roadmap -> PortfolioRepo? in
            let path = WaveOrigin.resolve(roadmap.wave.repo).normalizedFilePath
            guard seen.insert(path).inserted else { return nil }
            return PortfolioRepo(path: path, lastOpened: .distantPast)
        }
        return (roadmap, waves, repos)
    }

    private static func roadmapFixtureURL(sourceFile: String) throws -> URL {
        let fileManager = FileManager.default
        var roots = [URL(fileURLWithPath: fileManager.currentDirectoryPath, isDirectory: true)]
        if let executableURL = Bundle.main.executableURL {
            roots.append(executableURL.deletingLastPathComponent())
        }
        if sourceFile.hasPrefix("/") {
            roots.append(URL(fileURLWithPath: sourceFile).deletingLastPathComponent())
        }

        var visited = Set<String>()
        for root in roots {
            var directory = root.standardizedFileURL
            while visited.insert(directory.path).inserted {
                let candidate = directory
                    .appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
                if fileManager.fileExists(atPath: candidate.path) {
                    return candidate
                }
                let parent = directory.deletingLastPathComponent()
                guard parent.path != directory.path else { break }
                directory = parent
            }
        }

        throw CocoaError(
            .fileNoSuchFile,
            userInfo: [NSFilePathErrorKey: "tests/fixtures/dto/roadmap_snapshot.json"]
        )
    }
}
