#if os(macOS)
import Foundation
import Loopflow

/// Fixture data for the `mock-waves` UI-test mode (`AppTestMode.mockWaves`).
///
/// It bypasses the `lf` registry so the two states no data can produce render
/// deterministically: `loading` (before the first read) and `error` (a failed
/// read under the cached plan). Populated states, metrics included, render
/// from an `lf-world` world through the real registry in `live` mode.
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

    /// Which detail-pane state the selected Wave renders: `loading` holds the
    /// pre-snapshot state so the plan pane shows its loading affordance;
    /// `error` discards volatile detail and leaves the authored plan under the
    /// "showing cached plan · live status unavailable" footer. A screenshot run
    /// picks one with `LOOPFLOW_UI_TEST_DETAIL_STATE`.
    enum DetailState: String {
        case loading
        case error
    }

    static var detailState: DetailState {
        let raw = ProcessInfo.processInfo.environment["LOOPFLOW_UI_TEST_DETAIL_STATE"]
        return raw.flatMap(DetailState.init(rawValue:)) ?? .loading
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
                 status: .ready,
                 activeTasks: 1),
            Wave(id: "wave-2", name: "intelligence", repo: repoPath, status: .ready,
                 activeTasks: 2),
            Wave(id: "wave-3", name: "feedback", repo: repoPath, status: .ready,
                 activeTasks: 0),
            Wave(id: "wave-4", name: "cadenza", repo: repoPath,
                 status: .ready,
                 activeTasks: 0, parentWaveId: "wave-1"),
        ]
    }

    /// An objective per Wave, so a plan-only pane (any non-`infrastructure`
    /// selection) still leads with prose. `infrastructure` also gets its
    /// objective from `detailJSON`'s `wave.goal` via `workMap`.
    static var plans: [String: WavePlan] {
        var result: [String: WavePlan] = [:]
        for wave in waves {
            let key = PortfolioRepoState.wavePlanKey(repoPath: wave.repo, waveName: wave.name)
            result[key] = WavePlan(objective: objective(for: wave.name), projects: wave.name == detailWaveName ? (selectedWaveDetail()?.projects ?? .unavailable(reason: "Fixture missing")) : .available(items: [], truncated: false))
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
    /// `lf wave status --json` emits (the round-tripped `wave_detail.json` fixture).
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
        }
    }

    static let detailJSON = #"""
        {
          "project_readiness": {
            "state": "ready",
            "project_id": "project-1",
            "observed_at": null,
            "pending_successor": null,
            "activation": null
          },
          "wave": {
            "id": "wave-1",
            "name": "infrastructure",
            "status": "ready",
            "goal": "Make releases boring.",
            "repo": "/src/loopflow",
            "active_tasks": 1,
            "created_at": "2026-07-01T00:00:00Z",
            "parent_wave_id": null,
            "retired_at": null,
            "superseded_by_wave_id": null,
            "retirement_reason": null,
            "machine": {
              "id": "home_00000000000000000000000000000001",
              "route": "ssh://jack@mini-heart",
              "created_at": "2026-07-01T00:00:00Z",
              "observed_at": "2026-07-17T00:00:00Z"
            }
          },
          "metric_portfolio": {"metrics": [], "contract_issues": []},
          "history": {
            "state": "ok",
            "truncated": false,
            "items": [
              {
                "repo": "/src/loopflow",
                "worktree": "/src/loopflow.task",
                "skill": "task/pursue",
                "usage": {
                  "streams": 1,
                  "final_streams": 1,
                  "gaps": 0,
                  "input_tokens": 12000,
                  "output_tokens": 3000,
                  "total_input_tokens": 12000,
                  "peak_input_tokens": 10000,
                  "context_window_tokens": 200000,
                  "reasoning_tokens": 1000,
                  "cache_read_tokens": 8000,
                  "cache_write_tokens": 500,
                  "cost_usd": 0.42
                },
                "evidence_gaps": 0,
                "harness": "codex",
                "model": "gpt-5",
                "surface": "headless",
                "first_provider_attempt_at": 1784052010,
                "task_pr_id": "pr_33333333333333333333333333333333",
                "session_id": "run_00000000000000000000000000000001",
                "providers": [],
                "task_id": "task_22222222222222222222222222222222",
                "wave_id": "11111111-1111-4111-8111-111111111111",
                "task_identifier": "INF-123",
                "wave_name": "infrastructure",
                "work_source": null,
                "artifact_key": "run_00000000000000000000000000000001",
                "captured": 12,
                "caller_artifact_key": null,
                "observed_at": 1784052000,
                "recorded_outcome": "completed",
                "recorded_at": 1784052600
              }
            ]
          },
          "projects": {
            "state": "ok",
            "items": [
              {
                "id": "project-1",
                "linear_id": null,
                "sync": {"connected": false, "changes": []},
                "slug": "release-feedback",
                "name": "current",
                "workflow": "incident",
                "status": "started", "current": true,
                "metric_targets": [],
                "krs": [
                  {
                    "text": "Every failed run has an owner",
                    "holds": false
                  }
                ]
              }
            ],
            "truncated": false
          },
          "tasks": {
            "state": "ok",
            "items": [
              {
                "task": {
                  "repository_id": "repo_00000000000040008000000000000001",
                  "repo": "/src/loopflow",
                  "project_id": "project-1",
                  "wave_id": "wave-1",
                  "id": "issue-1",
                  "identifier": "INF-123",
                  "name": "Surface nightly failures",
                  "description": "Surface one focused failure.",
                  "rank": 1,
                  "state": "unstarted", "completed_at": null, "due_date": null, "acquisition": {"state": "available", "observed_at": null}, "linear_id": null, "sync": {"connected": false, "changes": []},
                  "assignee": "user-1"
                },
                "reference": {
                  "issue_url": "https://linear.app/loopflow/issue/INF-123/surface-nightly-failures",
                  "workspace": {
                    "slug": "infrastructure-task",
                    "branch": "jack/infrastructure.task.20260713_1200", "base_commit": "fixture-base",
                    "worktree": "/src/loopflow.infrastructure.task"
                  }
                },
                "runtime": {
                  "work_id": "ts_22222222222222222222222222222222",
                  "status": "ready",
                  "reason": "ready",
                  "updated_at": "2026-07-13T19:00:00Z",
                  "provider": "codex",
                  "owner": {"person": "Jack Heart", "machine": {"id": null, "label": null}}
                },
                "graph_name":"feature","graph_source":"wave","latest_graph_process":null,"execution":null,"run_control":{"unavailable":null},
                "directive": {
                  "version": 2,
                  "kind": "replacement",
                  "text": "Surface verification failures before publish failures.",
                  "applied_at": "2026-07-13T18:30:00Z",
                  "incorporated_at": "2026-07-13T18:31:00Z",
                  "incorporated_summary": "Verification failures are now first."
                },
                "next_move": {
                  "owner": "user",
                  "reason": "merge pull request head 333333333333 on GitHub"
                },
                "condition": {
                  "state": "waiting",
                  "reason": "merge pull request head 333333333333 on GitHub",
                  "observed_at": "2026-07-13T21:00:00Z",
                  "evidence_age_secs": 7200,
                  "local_progress": {
                    "state": "observed",
                    "unsettled": true,
                    "dirty": false,
                    "authored_commits": true,
                    "recovery_required": false,
                    "reason": null
                  },
                  "unresolved_execution": true
                },
                "actions": {
                  "recommended": "open_pr",
                  "reason": "merge head 333333333333 on GitHub"
                },
                "pr": {
                    "id": "pr_33333333333333333333333333333333",
                    "sequence": 1,
                    "slug": "infrastructure-task",
                    "branch": "jack/infrastructure.task.20260713_1200",
                    "base_commit": "1111111111111111111111111111111111111111",
                    "phase": "open",
                    "empty": false,
                    "publication": {
                      "requested_at": "2026-07-13T18:45:00Z",
                      "presentation": {
                        "title": "Ship infrastructure task",
                        "body": "Explains the intent and proof for this head.",
                        "head_sha": "3333333333333333333333333333333333333333"
                      },
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
                  }
              },
              {
                "task": {
                  "repository_id": "repo_00000000000040008000000000000001",
                  "repo": "/src/loopflow",
                  "project_id": "project-1",
                  "wave_id": "wave-1",
                  "id": "issue-2",
                  "identifier": "INF-124",
                  "name": "Classify publish failures",
                  "description": "",
                  "rank": 2,
                  "state": "unstarted", "completed_at": null, "due_date": null, "acquisition": {"state": "available", "observed_at": null}, "linear_id": null, "sync": {"connected": false, "changes": []},
                  "assignee": null
                },
                "reference": {
                  "issue_url": null,
                  "workspace": null
                },
                "runtime": null,
                "graph_name":"feature","graph_source":"wave","latest_graph_process":null,"execution":null,"run_control":{"unavailable":null},
                "directive": null,
                "next_move": {
                  "owner": "wave",
                  "reason": "Task is ready to start"
                },
                "condition": {
                  "state": "clear",
                  "reason": "Task is ready to start",
                  "observed_at": "2026-07-13T21:00:00Z",
                  "evidence_age_secs": null,
                  "local_progress": {
                    "state": "not_applicable",
                    "unsettled": false,
                    "dirty": null,
                    "authored_commits": null,
                    "recovery_required": null,
                    "reason": null
                  },
                  "unresolved_execution": false
                },
                "actions": {
                  "recommended": null,
                  "reason": "Task is ready to start"
                },
                "pr": null
              }
            ],
            "truncated": false
          },
          "unavailable_tasks": []
        }
        """#
}
#endif
