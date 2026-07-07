// RegistryQuery decodes the `lf ls/status/runs --json` wire snapshots (the Rust
// query types) and maps them onto the app models the stores hold. The runner is
// injected, so these exercise the parse + mapping without spawning `lf`.

import Foundation
import Testing
@testable import LoopflowCore

@Suite("RegistryQuery")
struct RegistryQueryTests {
    @Test("lf ls decodes and scopes to the repo")
    func wavesDecodeAndScope() async throws {
        let json = """
        [
          {"id":"goals","name":"goals","status":"running","paused":false,"goal":"ship the roadmap","repo":"/tmp/repo-a","iteration":3,"workers":2,"active_runs":1,"live":true,"endpoint":"127.0.0.1:5678","created_at":null,"parent_wave_id":null},
          {"id":"other","name":"other","status":"idle","paused":false,"goal":"g","repo":"/tmp/repo-b","iteration":0,"workers":1,"active_runs":0,"live":false,"endpoint":null,"created_at":null,"parent_wave_id":null}
        ]
        """
        let query = RegistryQuery { args, _ in
            #expect(args == ["ls", "--json"])
            return json
        }

        let waves = try await query.waves(repoPath: "/tmp/repo-a")
        #expect(waves.map(\.id) == ["goals"])
        #expect(waves[0].status == .running)
        #expect(waves[0].repo == "/tmp/repo-a")
    }

    @Test("lf status maps runs and attention onto the wave")
    func statusMapsRunsAndAttention() async throws {
        let json = """
        {
          "wave":{"id":"goals","name":"goals","status":"waiting","paused":false,"goal":"g","repo":"/tmp/repo-a","iteration":0,"workers":1,"active_runs":0,"live":false,"endpoint":null,"created_at":null,"parent_wave_id":null},
          "mind":"turning",
          "runs":[{"id":"run-1","flow":"implement","task":"wire it","status":"running","branch":"b","worktree":"/wt","started_at":null,"ended_at":null,"error":null,"pr_url":null}],
          "attention":[{"id":"att-1","kind":"interactive","status":"surfaced","title":"needs a human","summary":"review the design","run_id":"run-1","surfaced_at":"2026-07-06T00:00:00Z"}]
        }
        """
        let query = RegistryQuery { args, _ in
            #expect(args == ["status", "goals", "--json"])
            return json
        }

        let result = try await query.status(wave: "goals", waveId: "wave-1", cwd: nil)
        #expect(result.mind == "turning")
        #expect(result.runs.map(\.id) == ["run-1"])
        #expect(result.runs[0].waveId == "wave-1")
        #expect(result.runs[0].status == .running)
        #expect(result.attention.map(\.id) == ["att-1"])
        #expect(result.attention[0].waveId == "wave-1")
        #expect(result.attention[0].kind == .interactive)
    }

    @Test("lf runs decodes the ledger window")
    func runsDecode() async throws {
        let json = """
        [{"id":"abc","repo":"loopflow","wave":"goals","label":"gate","status":"ok","started":100,"ended":110,"input_tokens":1000,"output_tokens":200}]
        """
        let query = RegistryQuery { _, _ in json }

        let runs = try await query.recentRuns()
        #expect(runs.count == 1)
        #expect(runs[0].id == "abc")
        #expect(runs[0].wave == "goals")
        #expect(runs[0].status == "ok")
        #expect(runs[0].ended == 110)
    }

    @Test("a failed lf query surfaces as an error")
    func failedQueryThrows() async {
        let query = RegistryQuery { _, _ in throw RegistryQueryError("lf exploded") }
        await #expect(throws: RegistryQueryError.self) {
            _ = try await query.waves(repoPath: "/tmp/repo-a")
        }
    }
}
