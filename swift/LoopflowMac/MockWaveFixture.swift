#if os(macOS)
import Foundation
import Loopflow

/// Fixture data for the `mock-waves` UI-test mode (`AppTestMode.mockWaves`).
///
/// It bypasses the `lf` registry entirely so the populated Wave surface renders
/// deterministically offline — the stable list's green/red/blue/black lenses and the
/// selected Wave's full detail hierarchy (objective, Projects, KRs, Task rows
/// with verbatim attention lenses). That makes the "selected" screenshot state
/// real and, crucially, lets the AttributeGraph cycle capture drive the
/// `WaveDetailPane` selection path (its `@ObservedObject` fix site) at cold
/// launch on a machine whose registry can't serve W2-123 lens data.
///
/// Never referenced outside `AppTestMode`; production reads only `lf`.
enum MockWaveFixture {
    /// A path that need not exist: `mock-waves` gates off the on-disk authored
    /// scan, so nothing reads this directory.
    static let repoPath = "/src/loopflow"

    /// The Wave whose detail hierarchy is populated from `detailJSON`.
    static let detailWaveName = "infrastructure"

    /// The Wave the detail pane opens to. `LOOPFLOW_UI_TEST_SELECT_BRANCH`
    /// overrides it so a screenshot run can target a specific list state.
    static var selectedWaveName: String {
        let env = ProcessInfo.processInfo.environment["LOOPFLOW_UI_TEST_SELECT_BRANCH"]
        return (env?.isEmpty == false ? env : nil) ?? detailWaveName
    }

    /// Which detail-pane state the selected Wave renders. `selected` is the
    /// populated hierarchy; `loading` holds the pre-snapshot state so the plan
    /// pane shows its loading affordance; `error` preserves the cached detail
    /// under the "showing cached plan · live status unavailable" footer (the
    /// PR #932 preservation behavior). A screenshot run picks one with
    /// `LOOPFLOW_UI_TEST_DETAIL_STATE`, so the fixture covers every state the
    /// Proof names without a live registry.
    enum DetailState: String {
        case selected
        case loading
        case error
    }

    static var detailState: DetailState {
        let raw = ProcessInfo.processInfo.environment["LOOPFLOW_UI_TEST_DETAIL_STATE"]
        return raw.flatMap(DetailState.init(rawValue:)) ?? .selected
    }

    /// The failure a mock `error` state reports — a plausible offline reason,
    /// never a raw stack. `WaveDetailReading` frames it as the disclosed footer.
    static let refreshError = RegistryQueryError("the local registry is unreachable")

    /// The stable list, one Wave per lens state: green (a live body), red
    /// (stopped with active work), black (off and clean), plus a child Wave
    /// (parent set) to exercise future-ancestry row indentation.
    static var waves: [Wave] {
        [
            Wave(id: "wave-1", name: "infrastructure", repo: repoPath,
                 status: .running(runID: "run_infrastructure"),
                 live: true, activeTasks: 1, activeProjects: 1),
            Wave(id: "wave-2", name: "intelligence", repo: repoPath, status: .ready,
                 live: false, activeTasks: 2, activeProjects: 1),
            Wave(id: "wave-3", name: "feedback", repo: repoPath, status: .ready,
                 live: false, activeTasks: 0, activeProjects: 0),
            Wave(id: "wave-4", name: "cadenza", repo: repoPath,
                 status: .running(runID: "run_cadenza"),
                 live: true, activeTasks: 0, activeProjects: 0, parentWaveId: "wave-1"),
        ]
    }

    /// An objective per Wave, so a plan-only pane (any non-`infrastructure`
    /// selection) still leads with prose. `infrastructure` also gets its
    /// objective from `detailJSON`'s `wave.goal` via `workMap`.
    static var plans: [String: WavePlan] {
        var result: [String: WavePlan] = [:]
        for wave in waves {
            let key = PortfolioRepoState.wavePlanKey(repoPath: wave.repo, waveName: wave.name)
            result[key] = WavePlan(objective: objective(for: wave.name))
        }
        return result
    }

    private static func objective(for name: String) -> String {
        switch name {
        case "infrastructure": return "Make releases boring."
        case "intelligence": return "Loopflow gets sharper from its own evidence."
        case "feedback": return "Every failure becomes one focused piece of work."
        case "cadenza": return "Keep the wave's rhythm honest."
        default: return ""
        }
    }

    /// The selected Wave's populated detail, decoded from the same wire shape
    /// `lf status --json` emits (the round-tripped `wave_detail.json` fixture).
    static func selectedWaveDetail() -> WaveDetailSnapshot? {
        try? JSONDecoder().decode(WaveDetailSnapshot.self, from: Data(detailJSON.utf8))
    }

    /// The detail reading a `mock-waves` capture renders for `waveName` in the
    /// given state, plus whether the pane still awaits its first live read (the
    /// loading affordance stays on screen only while awaiting). Pure so the
    /// screenshot states are covered without launching the app.
    static func detailReading(
        waveName: String,
        state: DetailState
    ) -> (reading: WaveDetailReading, awaitingFirstRead: Bool) {
        let snapshot = waveName == detailWaveName ? selectedWaveDetail() : nil
        var reading = WaveDetailReading()
        switch state {
        case .loading:
            reading.clear()
            return (reading, true)
        case .error:
            if let snapshot { reading.update(snapshot) }
            reading.recordFailure(refreshError)
            return (reading, false)
        case .selected:
            if let snapshot { reading.update(snapshot) } else { reading.clear() }
            return (reading, false)
        }
    }

    static let detailJSON = #"""
    {
      "wave": {
        "id": "wave-1",
        "name": "infrastructure",
        "status": {"running":{"run_id":"run_00000000000000000000000000000004"}},
        "goal": "Make releases boring.",
        "repo": "/src/loopflow",
        "active_tasks": 1,
        "active_projects": 1,
        "live": true,
        "endpoint": "127.0.0.1:7777",
        "created_at": "2026-07-01T00:00:00Z",
        "parent_wave_id": null,
        "home": {
          "id": "home_00000000000000000000000000000001",
          "route": "ssh://jack@mini-heart",
          "created_at": "2026-07-01T00:00:00Z",
          "observed_at": "2026-07-17T00:00:00Z"
        }
      },
      "loop_state": "idle",
      "projects": [
        {
          "project": {
            "id": "project-1",
            "slug": "release-feedback",
            "name": "Release feedback",
            "summary": "Failures become focused work.",
            "definition": "Close the release feedback loop.",
            "flows": {"first": "incident", "loop": "ship-5whys", "finally": "ship"},
            "krs": [
              {"text": "Every failed run has an owner", "holds": false}
            ]
          },
          "runtime": {
            "work_id": "ps_11111111111111111111111111111111",
            "status": "ready",
            "reason": "supervised Tasks are active",
            "updated_at": "2026-07-13T18:00:00Z",
            "iteration": 2,
            "pending_observations": 0,
            "provider": "codex",
            "process_alive": false,
            "observation": {"category": "needs_input", "reason": "supervised Tasks are active", "owner": "user", "controls": ["decide", "resume", "abandon"], "progress_age_secs": null, "deadline_in_secs": null, "step": "iteration 2"}
          },
          "directive": {
            "version": 1,
            "kind": "initial",
            "text": "Own release feedback and supervise failures.",
            "applied_at": "2026-07-13T17:55:00Z",
            "incorporated_at": "2026-07-13T17:56:00Z",
            "incorporated_summary": "Failure ownership is the active priority."
          },
          "next_move": {"owner": "project", "reason": "supervised Tasks are active"},
          "tasks": [
            {
              "task": {
                "id": "issue-1",
                "identifier": "INF-123",
                "name": "Surface nightly failures",
                "description": "Surface one focused failure.",
                "rank": 1,
                "completed": false,
                "assignee": "user-1"
              },
              "reference": {
                "issue_url": "https://linear.app/loopflow/issue/INF-123/surface-nightly-failures",
                "workspace": {
                  "slug": "infrastructure-task",
                  "branch": "jack/infrastructure.task.20260713_1200",
                  "worktree": "/src/loopflow.infrastructure.task"
                }
              },
              "runtime": {
                "work_id": "ts_22222222222222222222222222222222",
                "project_id": "ps_11111111111111111111111111111111",
                "status": "ready",
                "reason": "waiting for user merge",
                "updated_at": "2026-07-13T19:00:00Z",
                "provider": "codex",
                "process_alive": false,
                "observation": {"category": "needs_input", "reason": "waiting for user merge", "owner": "user", "controls": ["decide", "resume", "abandon"], "progress_age_secs": null, "deadline_in_secs": null, "step": "iterate"}
              },
              "directive": {
                "version": 2,
                "kind": "replacement",
                "text": "Surface verification failures before publish failures.",
                "applied_at": "2026-07-13T18:30:00Z",
                "incorporated_at": "2026-07-13T18:31:00Z",
                "incorporated_summary": "Verification failures are now first."
              },
              "next_move": {"owner": "user", "reason": "merge pull request head 333333333333 on GitHub"},
              "attention": {
                "level": "red",
                "reason": "merge pull request head 333333333333 on GitHub",
                "observed_at": "2026-07-13T21:00:00Z",
                "evidence_age_secs": 7200,
                "next_owner": "user",
                "actions":{"recommended":"open_pr","reason":"merge head 333333333333 on GitHub"},
                "pm_completed": false,
                "work_status": "ready",
                "process": {"state": "not_expected", "alive": null, "reason": null},
                "local_progress": {"state": "observed", "unsettled": true, "dirty": false, "authored_commits": true, "recovery_required": false, "reason": null},
                "active_pr_phase": "open"
              },
              "prs": [{
                "id": "pr_33333333333333333333333333333333",
                "sequence": 1,
                "slug": "infrastructure-task",
                "branch": "jack/infrastructure.task.20260713_1200",
                "base_commit": "1111111111111111111111111111111111111111",
                "phase": "open",
                "empty": false,
                "publication": {
                  "requested_at": "2026-07-13T18:45:00Z",
                  "github": {
                    "number": 912,
                    "url": "https://github.com/loopflowstudio/loopflow/pull/912"
                  },
                  "merge": {
                    "mode": "user",
                    "requested_at": "2026-07-13T18:46:00Z",
                    "head_sha": "3333333333333333333333333333333333333333",
                    "after_merge": "complete_task",
                    "next_slug": null
                  }
                },
                "merge_commit": null,
                "abandoned_at": null
              }],
              "active_pr": "pr_33333333333333333333333333333333"
            },
            {
              "task": {
                "id": "issue-2",
                "identifier": "INF-124",
                "name": "Classify publish failures",
                "description": "",
                "rank": 2,
                "completed": false,
                "assignee": null
              },
              "reference": {
                "issue_url": null,
                "workspace": null
              },
              "runtime": null,
              "directive": null,
              "next_move": {"owner": "project", "reason": "Task is ready to start"},
              "attention": {
                "level": "black",
                "reason": "Task is ready to start",
                "observed_at": "2026-07-13T21:00:00Z",
                "evidence_age_secs": null,
                "next_owner": "project",
                "actions":{"recommended":null,"reason":"Task is ready to start"},
                "pm_completed": false,
                "work_status": null,
                "process": {"state": "not_applicable", "alive": null, "reason": null},
                "local_progress": {"state": "not_applicable", "unsettled": false, "dirty": null, "authored_commits": null, "recovery_required": null, "reason": null},
                "active_pr_phase": null
              },
              "prs": [],
              "active_pr": null
            }
          ]
        }
      ],
      "runs": {
        "state": "ok",
        "truncated": false,
        "items": [
          {
            "id": "invocation-1",
            "trace_id": "run-1",
            "exec_id": "proc-1",
            "parent_exec_id": null,
            "repo": "/src/loopflow",
            "worktree": "/src/loopflow.task",
            "wave": "infrastructure",
            "flow": "task",
            "skill": "task_pursue",
            "status": "ok",
            "started": 1784052000,
            "ended": 1784052600,
            "turns": 1,
            "system_tokens": 2000,
            "task_tokens": 1000,
            "supplied_context_tokens": 3000,
            "input_tokens": 12000,
            "output_tokens": 3000,
            "reasoning_tokens": 1000,
            "cache_read_tokens": 8000,
            "cache_write_tokens": 500,
            "cost_usd": 0.42,
            "duration_secs": 600.0,
            "provider": "codex",
            "model": "gpt-5",
            "surface": "headless",
            "capture_status": "complete"
          }
        ]
      },
      "attention": {
        "state": "ok",
        "truncated": false,
        "items": [
          {
            "kind": "task",
            "id": "ts_22222222222222222222222222222222",
            "subject": "INF-123",
            "owner": "user",
            "reason": "merge pull request head 333333333333 on GitHub",
            "since": "2026-07-13T19:00:00Z",
            "age_secs": 7200
          }
        ]
      },
      "home_runtime": {
        "home": {
          "id": "home_00000000000000000000000000000001",
          "route": "ssh://jack@mini-heart",
          "created_at": "2026-07-01T00:00:00Z",
          "observed_at": "2026-07-17T00:00:00Z"
        },
        "state": "running",
        "reason": "resident is serving on the Home",
        "endpoint": "127.0.0.1:7777",
        "action": {"kind": "attach", "endpoint": "127.0.0.1:7777"}
      }
    }
    """#
}
#endif
