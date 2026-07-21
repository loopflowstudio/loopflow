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
                    activity: .available(try emptyActivity()),
                    repos: [],
                    fixed: true
                )
            case .mockWaves:
                let fixture = try loadRoadmap(sourceFile: sourceFile)
                let reading: ControlRoomReading<RoadmapSnapshot>
                let activity: ControlRoomReading<ActivitySnapshot>
                switch MockWaveFixture.detailState {
                case .selected:
                    reading = .available(fixture.roadmap)
                    activity = .available(try loadActivity(sourceFile: sourceFile))
                case .loading:
                    reading = .loading
                    activity = .loading
                case .error:
                    reading = .unavailable(
                        lastGood: nil,
                        reason: "the local registry is unreachable"
                    )
                    activity = .unavailable(
                        lastGood: nil,
                        reason: "live process evidence is unavailable"
                    )
                }
                model.applyFixture(
                    roadmap: reading,
                    waves: .available(fixture.waves),
                    activity: activity,
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
                activity: .unavailable(lastGood: nil, reason: error.localizedDescription),
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

    private static func loadActivity(sourceFile: String) throws -> ActivitySnapshot {
        let url = try fixtureURL(named: "activity_snapshot.json", sourceFile: sourceFile)
        return try JSONDecoder().decode(ActivitySnapshot.self, from: Data(contentsOf: url))
    }

    private static func emptyActivity() throws -> ActivitySnapshot {
        let json = #"{"schema_version":1,"observed_at":1784606400,"fast_window_seconds":300,"slow_window_seconds":1800,"aggregate":{"measured_output_tokens":0,"output_tokens_fast":0,"output_tokens_slow":0,"output_tokens_per_second_fast":0.0,"output_tokens_per_second_slow":0.0,"measured_turns":0,"unmeasured_turns":0},"nodes":[],"provider_processes":[]}"#
        return try JSONDecoder().decode(ActivitySnapshot.self, from: Data(json.utf8))
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
