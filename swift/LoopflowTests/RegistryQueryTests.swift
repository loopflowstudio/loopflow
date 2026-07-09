// RegistryQuery decodes the `lf ls/status/runs --json` wire snapshots (the Rust
// query types) and maps them onto the app models the stores hold. The runner is
// injected, so these exercise the parse + mapping without spawning `lf`.

import Foundation
import Testing
@testable import Loopflow

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

    @Test("lf ls can be decoded once for every repo")
    func allWavesDecode() async throws {
        let json = """
        [
          {"id":"goals","name":"goals","status":"running","paused":false,"goal":"ship the roadmap","repo":"/tmp/repo-a","iteration":3,"workers":2,"active_runs":1,"live":true,"endpoint":"127.0.0.1:5678","created_at":null,"parent_wave_id":null},
          {"id":"other","name":"other","status":"idle","paused":false,"goal":"g","repo":"/tmp/repo-b","iteration":0,"workers":1,"active_runs":0,"live":false,"endpoint":null,"created_at":null,"parent_wave_id":null}
        ]
        """
        let counter = CallCounter()
        let query = RegistryQuery { args, _ in
            await counter.increment()
            #expect(args == ["ls", "--json"])
            return json
        }

        let waves = try await query.allWaves()
        #expect(await counter.value == 1)
        #expect(waves.map(\.id) == ["goals", "other"])
    }


    @Test("lf status maps runs and attention onto the wave")
    func statusMapsRunsAndAttention() async throws {
        let json = """
        {
          "wave":{"id":"goals","name":"goals","status":"waiting","paused":false,"goal":"g","repo":"/tmp/repo-a","iteration":0,"workers":1,"active_runs":0,"live":false,"endpoint":null,"created_at":null,"parent_wave_id":null},
          "flowloop":"turning",
          "runs":[{"id":"run-1","flow":"implement","task":"wire it","step_index":2,"status":"running","branch":"b","worktree":"/wt","started_at":null,"ended_at":null,"error":null,"pr_url":null}],
          "attention":[{"id":"att-1","kind":"interactive","status":"surfaced","title":"needs a human","summary":"review the design","run_id":"run-1","surfaced_at":"2026-07-06T00:00:00Z"}]
        }
        """
        let query = RegistryQuery { args, _ in
            #expect(args == ["status", "goals", "--json"])
            return json
        }

        let result = try await query.status(wave: "goals", waveId: "wave-1", cwd: nil)
        #expect(result.flowloop == "turning")
        #expect(result.runs.map(\.id) == ["run-1"])
        #expect(result.runs[0].waveId == "wave-1")
        #expect(result.runs[0].status == .running)
        #expect(result.runs[0].stepIndex == 2)
        #expect(result.attention.map(\.id) == ["att-1"])
        #expect(result.attention[0].waveId == "wave-1")
        #expect(result.attention[0].kind == .interactive)
    }

    @Test("lf status preserves completed and unknown run statuses")
    func statusPreservesRunStatuses() async throws {
        let json = """
        {
          "wave":{"id":"goals","name":"goals","status":"waiting","paused":false,"goal":"g","repo":"/tmp/repo-a","iteration":0,"workers":1,"active_runs":0,"live":false,"endpoint":null,"created_at":null,"parent_wave_id":null},
          "mind":null,
          "runs":[
            {"id":"run-1","flow":"implement","task":null,"step_index":0,"status":"completed","branch":"b","worktree":"/wt","started_at":"2026-07-06T00:00:00Z","ended_at":null,"error":null,"pr_url":null},
            {"id":"run-2","flow":"gate","task":null,"step_index":0,"status":"new-token","branch":"b","worktree":"/wt","started_at":null,"ended_at":null,"error":null,"pr_url":null}
          ],
          "attention":[]
        }
        """
        let query = RegistryQuery { _, _ in json }

        let result = try await query.status(wave: "goals", waveId: "wave-1", cwd: nil)

        #expect(result.runs[0].status == .completed)
        #expect(result.runs[0].area == nil)
        #expect(result.runs[0].createdAt != nil)
        #expect(result.runs[1].status == .unknown("new-token"))
        #expect(result.runs[1].status.displayName == "Unknown: new-token")
        #expect(result.runs[1].createdAt == nil)
    }

    @Test("lf runs decodes the ledger window")
    func runsDecode() async throws {
        let json = """
        [{"id":"span-1","run_id":"abc","process_id":"span-1","parent_process_id":null,"repo":"/src/loopflow","wave":"goals","label":"gate","status":"ok","started":100,"ended":110,"input_tokens":1000,"output_tokens":200,"cache_read_tokens":800,"cost_usd":0.25,"duration_secs":10.0,"provider":"claude","model":"opus"}]
        """
        let query = RegistryQuery { _, _ in json }

        let runs = try await query.recentRuns()
        #expect(runs.count == 1)
        #expect(runs[0].id == "span-1")
        #expect(runs[0].runId == "abc")
        #expect(runs[0].wave == "goals")
        #expect(runs[0].status == "ok")
        #expect(runs[0].ended == 110)
        #expect(runs[0].cacheReadTokens == 800)
        #expect(runs[0].model == "opus")
    }

    @Test("lf usage --json preserves lineage and unspent spans")
    func spendDecodes() async throws {
        let json = """
        [{"run_id":"abc","process_id":"child","parent_process_id":"parent","seq":7,"node":"run","name":"lf pm show","repo":null,"wave":null,"flow":null,"skill":null,"started_at":100,"ended_at":null,"status":"open","input_tokens":null,"output_tokens":null,"cache_read_tokens":null,"cost_usd":null,"duration_secs":null,"provider":null,"model":null}]
        """
        let query = RegistryQuery { args, _ in
            #expect(args == ["usage", "--json", "--days", "30"])
            return json
        }

        let spans = try await query.spend()
        #expect(spans[0].processId == "child")
        #expect(spans[0].id == "child-7")
        #expect(spans[0].parentProcessId == "parent")
        #expect(spans[0].status == "open")
        #expect(spans[0].endedAt == nil)
        // A span that spent nothing carries nulls, not zeros.
        #expect(spans[0].totalTokens == 0)
        #expect(spans[0].agent == "unattributed")
    }

    @Test("lf doctor decodes every check")
    func doctorDecodes() async throws {
        let json = """
        {"rows":2,"checks":[{"name":"lineage","status":"ok","detail":"every parent process resolves"}]}
        """
        let query = RegistryQuery { args, _ in
            #expect(args == ["doctor", "--json"])
            return json
        }

        let report = try await query.doctor()
        #expect(report.rows == 2)
        #expect(report.checks[0].name == "lineage")
        #expect(report.checks[0].status == "ok")
    }

    @Test("PM snapshot exposes only filed open backlog")
    func backlogDecodesOpenTasks() async throws {
        let json = """
        {"wave":"goals","provider":"linear","project":"p1","local_project":null,"items":[
          {"id":"TASK-1","name":"Ship loop","description":"","rank":1,"completed":false,"labels":["project:runtime"],"assignee":null},
          {"id":"TASK-2","name":"Already done","description":"","rank":2,"completed":true,"labels":[],"assignee":"user-1"}
        ]}
        """
        let query = RegistryQuery { args, cwd in
            #expect(args == ["pm", "show", "--wave", "goals", "--json"])
            #expect(cwd == "/tmp/repo")
            return json
        }

        let items = try await query.backlog(wave: "goals", cwd: "/tmp/repo")
        #expect(items.map(\.id) == ["TASK-1"])
        #expect(items[0].labels == ["project:runtime"])
    }

    @Test("run status accepts lf runs folded ok token")
    func runStatusAcceptsFoldedOkToken() {
        #expect(RunStatus(lfToken: "ok") == .ok)
        #expect(RunStatus(lfToken: "escal.") == .escalated)
    }

    @Test("a failed lf query surfaces as an error")
    func failedQueryThrows() async {
        let query = RegistryQuery { _, _ in throw RegistryQueryError("lf exploded") }
        await #expect(throws: RegistryQueryError.self) {
            _ = try await query.waves(repoPath: "/tmp/repo-a")
        }
    }
}

private actor CallCounter {
    private var count = 0

    var value: Int { count }

    func increment() {
        count += 1
    }
}
