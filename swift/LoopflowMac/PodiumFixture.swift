import Foundation
import Loopflow

@MainActor
enum PodiumFixture {
    static func applyIfRequested(to model: PodiumModel, sourceFile: String = #filePath) {
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
                    processActivity: .available(try emptyProcessActivity()),
                    workActivity: .available(try emptyWorkActivity()),
                    repos: [],
                    fixed: true
                )
            case .mockWaves:
                let fixture = try loadRoadmap(sourceFile: sourceFile)
                let reading: PodiumReading<RoadmapSnapshot>
                let processActivity: PodiumReading<ActivitySnapshot>
                let workActivity: PodiumReading<WorkActivitySnapshot>
                switch MockWaveFixture.detailState {
                case .selected:
                    reading = .available(fixture.roadmap)
                    processActivity = .available(try loadProcessActivity(sourceFile: sourceFile))
                    workActivity = .available(try loadWorkActivity(sourceFile: sourceFile))
                case .loading:
                    reading = .loading
                    processActivity = .loading
                    workActivity = .loading
                case .error:
                    reading = .unavailable(
                        lastGood: nil,
                        reason: "the local registry is unreachable"
                    )
                    processActivity = .unavailable(
                        lastGood: nil,
                        reason: "live process evidence is unavailable"
                    )
                    workActivity = .unavailable(
                        lastGood: nil,
                        reason: "durable Work activity is unavailable"
                    )
                }
                model.applyFixture(
                    roadmap: reading,
                    waves: .available(fixture.waves),
                    processActivity: processActivity,
                    workActivity: workActivity,
                    repos: fixture.repos,
                    fixed: true
                )
                if let requested = AppTestMode.selectBranch,
                   let wave = fixture.roadmap.waves.first(where: { $0.wave.name == requested }) {
                    model.select(.wave(waveId: wave.wave.id))
                }
            case .live:
                break
            }
        } catch {
            model.applyFixture(
                roadmap: .unavailable(
                    lastGood: nil,
                    reason: "Podium fixture unavailable: \(error.localizedDescription)"
                ),
                waves: .unavailable(lastGood: nil, reason: error.localizedDescription),
                processActivity: .unavailable(lastGood: nil, reason: error.localizedDescription),
                workActivity: .unavailable(lastGood: nil, reason: error.localizedDescription),
                repos: [],
                fixed: true
            )
        }
    }

    private static func loadRoadmap(
        sourceFile: String
    ) throws -> (roadmap: RoadmapSnapshot, waves: [Wave], repos: [PortfolioRepo]) {
        let url = try fixtureURL(named: "roadmap_snapshot.json", sourceFile: sourceFile)
        let data = try Data(contentsOf: url)
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: data)
        let waves = roadmap.waves.map { $0.wave.toWave() }
        var seen = Set<String>()
        let repos = roadmap.waves.compactMap { roadmap -> PortfolioRepo? in
            let path = WaveOrigin.resolve(roadmap.wave.repo).normalizedFilePath
            guard seen.insert(path).inserted else { return nil }
            return PortfolioRepo(path: path, lastOpened: .distantPast)
        }
        return (roadmap, waves, repos)
    }

    private static func loadProcessActivity(sourceFile: String) throws -> ActivitySnapshot {
        let url = try fixtureURL(named: "activity_snapshot.json", sourceFile: sourceFile)
        return try JSONDecoder().decode(ActivitySnapshot.self, from: Data(contentsOf: url))
    }

    private static func loadWorkActivity(sourceFile: String) throws -> WorkActivitySnapshot {
        let url = try fixtureURL(named: "work_activity_snapshot.json", sourceFile: sourceFile)
        return try JSONDecoder().decode(WorkActivitySnapshot.self, from: Data(contentsOf: url))
    }

    private static func emptyProcessActivity() throws -> ActivitySnapshot {
        let json = #"{"schema_version":1,"observed_at":1784606400,"fast_window_seconds":300,"slow_window_seconds":1800,"aggregate":{"measured_output_tokens":0,"output_tokens_fast":0,"output_tokens_slow":0,"output_tokens_per_second_fast":0.0,"output_tokens_per_second_slow":0.0,"measured_turns":0,"unmeasured_turns":0},"nodes":[],"provider_processes":[]}"#
        return try JSONDecoder().decode(ActivitySnapshot.self, from: Data(json.utf8))
    }

    private static func emptyWorkActivity() throws -> WorkActivitySnapshot {
        let json = #"{"generated_at":1784606400,"since":1784001600,"limit":50,"truncated":false,"items":[]}"#
        return try JSONDecoder().decode(WorkActivitySnapshot.self, from: Data(json.utf8))
    }

    private static func fixtureURL(named name: String, sourceFile: String) throws -> URL {
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
                    .appendingPathComponent("tests/fixtures/dto")
                    .appendingPathComponent(name)
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
            userInfo: [NSFilePathErrorKey: "tests/fixtures/dto/\(name)"]
        )
    }
}
