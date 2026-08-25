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
    /// pane shows its loading affordance; `error` discards volatile detail and
    /// leaves the authored plan under the "showing cached plan · live status
    /// unavailable" footer. A screenshot run picks one with
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
                 status: .ready,
                 live: true, activeTasks: 1, activeProjects: 1),
            Wave(id: "wave-2", name: "intelligence", repo: repoPath, status: .ready,
                 live: false, activeTasks: 2, activeProjects: 1),
            Wave(id: "wave-3", name: "feedback", repo: repoPath, status: .ready,
                 live: false, enabled: false, activeTasks: 0, activeProjects: 0),
            Wave(id: "wave-4", name: "cadenza", repo: repoPath,
                 status: .ready,
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
        "status": "ready",
        "goal": "Make releases boring.",
        "repo": "/src/loopflow",
        "active_tasks": 1,
        "active_projects": 1,
        "live": true,
        "paused": false,
        "enabled": true,
        "endpoint": "127.0.0.1:7777",
        "created_at": "2026-07-01T00:00:00Z",
        "parent_wave_id": null,
        "retired_at": null,
        "superseded_by_wave_id": null,
        "retirement_reason": null,
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
            "reason": "ready",
            "updated_at": "2026-07-13T18:00:00Z",
            "iteration": 2,
            "pending_observations": 0,
            "provider": "codex",
            "last_failure": {
              "message": "project runner failed: credential is missing",
              "occurred_at": "2026-07-22T09:30:00Z"
            }
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
                "routing_project_id": "ps_11111111111111111111111111111111",
                "status": "ready",
                "reason": "ready",
                "updated_at": "2026-07-13T19:00:00Z",
                "provider": "codex"
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
                "local_progress": {"state": "not_applicable", "unsettled": false, "dirty": null, "authored_commits": null, "recovery_required": null, "reason": null},
                "active_pr_phase": null
              },
              "prs": [],
              "active_pr": null
            }
          ]
        }
      ],
      "metric_portfolio": {
        "metrics": [
          {
            "identity": {"wave_id": "wave-1", "metric_id": "task-loop-trust"},
            "contract_revision": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "name": "Task loops earn trust",
            "description": "Fraction of Tasks settled during the trailing seven days that either completed with every PR landed through Loopflow auto-merge or stopped with a non-resumable failure receipt. Open Tasks are excluded. A user-landed PR or manual Git repair inside the Task fails the metric.",
            "project_id": "project-1",
            "stage": "graduated",
            "instrumented": true,
            "instrument": "lifecycle-scorecard",
            "unit": "ratio",
            "target": {"kind": "at_least", "value": 1.0},
            "window": "7d",
            "freshness_policy": "30h",
            "freshness": {"kind": "fresh", "source_time": "2026-08-20T18:00:00Z", "expires_at": "2026-08-22T00:00:00Z"},
            "evidence": {"kind": "met", "value": 1.0, "source_window_start": "2026-08-13T18:00:00Z", "source_window_end": "2026-08-20T18:00:00Z"}
          },
          {
            "identity": {"wave_id": "wave-1", "metric_id": "failure-ownership"},
            "contract_revision": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "name": "Failures leave with an owner",
            "description": "Share of failed release runs assigned to one accountable Project before the next scheduled run.",
            "project_id": "project-1",
            "stage": "graduated",
            "instrumented": true,
            "instrument": "release-ledger",
            "unit": "ratio",
            "target": {"kind": "at_least", "value": 0.95},
            "window": "30d",
            "freshness_policy": "24h",
            "freshness": {"kind": "fresh", "source_time": "2026-08-20T18:00:00Z", "expires_at": "2026-08-21T18:00:00Z"},
            "evidence": {"kind": "missed", "value": 0.82, "source_window_start": "2026-07-21T18:00:00Z", "source_window_end": "2026-08-20T18:00:00Z"}
          },
          {
            "identity": {"wave_id": "wave-1", "metric_id": "diagnosis-latency"},
            "contract_revision": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "name": "Diagnosis stays fast",
            "description": "Median elapsed time from a failed release check to an actionable diagnosis.",
            "project_id": "project-1",
            "stage": "installed",
            "instrumented": false,
            "instrument": "incident-timeline",
            "unit": "minutes",
            "target": {"kind": "at_most", "value": 30.0},
            "window": "14d",
            "freshness_policy": "24h",
            "freshness": {"kind": "never"},
            "evidence": {"kind": "unknown", "cause": {"kind": "never"}}
          }
        ],
        "contract_issues": [
          {
            "kind": "instrument_mismatch",
            "wave_id": "wave-1",
            "metric_id": "diagnosis-latency",
            "contract_instrument": "incident-timeline",
            "registered_instrument": "release-events-v1"
          }
        ]
      },
      "unavailable_projects": [],
      "runs": {
        "state": "ok",
        "truncated": false,
        "items": [
          {
            "id": "run_00000000000000000000000000000001",
            "parent_run_id": null,
            "repo": "/src/loopflow",
            "worktree": "/src/loopflow.task",
            "subjects": [
              {"selector": "wave:infrastructure", "source": "declared"},
              {"selector": "task:INF-123", "source": "declared"}
            ],
            "skill": "task/pursue",
            "outcome": "completed",
            "started": 1784052000,
            "ended": 1784052600,
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
            "surface": "headless"
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
