//! Canonical migration identities and registration, shared by build and runtime.

use std::fmt;

// -- Identity -----------------------------------------------------------------

/// A migration's identity: legacy `{major}.{minor}.{ordinal:03}` or release-scoped
/// `{major}.{minor}.{patch}.{ordinal:03}`.
///
/// New migrations carry the full package version of their release cut. Historical
/// three-part ids remain immutable and sort before release-scoped ids in the same
/// major/minor line. Ordering is numeric, never a string sort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MigrationId {
    pub major: u32,
    pub minor: u32,
    pub patch: Option<u32>,
    pub ordinal: u32,
}

impl MigrationId {
    /// The id leading a canonical version string (`0.10.001_initial`), or `None`
    /// if the string does not carry a release-scoped id at all — which is how a
    /// ledger row from the pre-namespace era is told apart from a future release.
    pub(super) fn parse_version(version: &str) -> Option<Self> {
        let (id, _name) = version.split_once('_')?;
        let numbers = id
            .split('.')
            .map(str::parse)
            .collect::<Result<Vec<u32>, _>>()
            .ok()?;
        match numbers.as_slice() {
            [major, minor, ordinal] => Some(MigrationId {
                major: *major,
                minor: *minor,
                patch: None,
                ordinal: *ordinal,
            }),
            [major, minor, patch, ordinal] => Some(MigrationId {
                major: *major,
                minor: *minor,
                patch: Some(*patch),
                ordinal: *ordinal,
            }),
            _ => None,
        }
    }
}

impl fmt::Display for MigrationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.patch {
            Some(patch) => write!(
                f,
                "{}.{}.{}.{:03}",
                self.major, self.minor, patch, self.ordinal
            ),
            None => write!(f, "{}.{}.{:03}", self.major, self.minor, self.ordinal),
        }
    }
}

/// One migration file. `version()` is the canonical string recorded in
/// `schema_migrations` and is exactly the file stem, so renaming a shipped file
/// is a schema break rather than a cosmetic edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub id: MigrationId,
    pub name: &'static str,
    pub sql: &'static str,
}

impl Migration {
    pub fn version(&self) -> String {
        format!("{}_{}", self.id, self.name)
    }
}

/// Every canonical migration, in id order. Release cuts append one generated
/// batch after topologically ordering the ordinal-free drafts.
pub(super) const MIGRATIONS: &[Migration] = &[
    Migration {
        id: MigrationId {
            major: 0,
            minor: 10,
            patch: None,
            ordinal: 1,
        },
        name: "initial",
        sql: include_str!("migrations/0.10.001_initial.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 10,
            patch: None,
            ordinal: 2,
        },
        name: "session_execution_context",
        sql: include_str!("migrations/0.10.002_session_execution_context.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 1,
        },
        name: "task_prs",
        sql: include_str!("migrations/0.11.001_task_prs.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 2,
        },
        name: "project_session_successors",
        sql: include_str!("migrations/0.11.002_project_session_successors.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 3,
        },
        name: "child_body_lease",
        sql: include_str!("migrations/0.11.003_child_body_lease.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 4,
        },
        name: "task_pr_ci_state",
        sql: include_str!("migrations/0.11.004_task_pr_ci_state.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 5,
        },
        name: "provider_accounts",
        sql: include_str!("migrations/0.11.005_provider_accounts.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 6,
        },
        name: "context_launch_work",
        sql: include_str!("migrations/0.11.006_context_launch_work.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 7,
        },
        name: "task_pr_parent",
        sql: include_str!("migrations/0.11.007_task_pr_parent.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 8,
        },
        name: "interactive_handoffs",
        sql: include_str!("migrations/0.11.008_interactive_handoffs.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 9,
        },
        name: "context_pressure",
        sql: include_str!("migrations/0.11.009_context_pressure.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 10,
        },
        name: "context_input_normalization",
        sql: include_str!("migrations/0.11.010_context_input_normalization.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 11,
        },
        name: "profiles",
        sql: include_str!("migrations/0.11.011_profiles.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 12,
        },
        name: "provider_account_lifecycle",
        sql: include_str!("migrations/0.11.012_provider_account_lifecycle.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 13,
        },
        name: "task_review_state",
        sql: include_str!("migrations/0.11.013_task_review_state.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 14,
        },
        name: "task_lifecycle",
        sql: include_str!("migrations/0.11.014_task_lifecycle.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 15,
        },
        name: "interaction_reviews",
        sql: include_str!("migrations/0.11.015_interaction_reviews.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 16,
        },
        name: "task_linear_observations",
        sql: include_str!("migrations/0.11.016_task_linear_observations.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 17,
        },
        name: "migration_provenance",
        sql: include_str!("migrations/0.11.017_migration_provenance.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 18,
        },
        name: "session_body_provenance",
        sql: include_str!("migrations/0.11.018_session_body_provenance.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 19,
        },
        name: "task_pr_github_observation",
        sql: include_str!("migrations/0.11.019_task_pr_github_observation.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 20,
        },
        name: "task_pr_linear_linkage",
        sql: include_str!("migrations/0.11.020_task_pr_linear_linkage.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 21,
        },
        name: "provider_deliveries",
        sql: include_str!("migrations/0.11.021_provider_deliveries.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 22,
        },
        name: "task_session_successors",
        sql: include_str!("migrations/0.11.022_task_session_successors.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 23,
        },
        name: "capture_pruned_state",
        sql: include_str!("migrations/0.11.023_capture_pruned_state.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 24,
        },
        name: "ci_incidents",
        sql: include_str!("migrations/0.11.024_ci_incidents.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 25,
        },
        name: "usage_deltas",
        sql: include_str!("migrations/0.11.025_usage_deltas.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 26,
        },
        name: "lineage_boundary",
        sql: include_str!("migrations/0.11.026_lineage_boundary.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 27,
        },
        name: "accounts_first",
        sql: include_str!("migrations/0.11.027_accounts_first.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 29,
        },
        name: "ci_incident_repaired_head",
        sql: include_str!("migrations/0.11.029_ci_incident_repaired_head.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 30,
        },
        name: "one_spend_grain",
        sql: include_str!("migrations/0.11.030_one_spend_grain.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 31,
        },
        name: "durable_input_spine",
        sql: include_str!("migrations/0.11.031_durable_input_spine.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 32,
        },
        name: "run_launch_attention",
        sql: include_str!("migrations/0.11.032_run_launch_attention.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 33,
        },
        name: "launch_attention_only",
        sql: include_str!("migrations/0.11.033_launch_attention_only.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 34,
        },
        name: "typed_ci_runs",
        sql: include_str!("migrations/0.11.034_typed_ci_runs.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 35,
        },
        name: "drop_child_commands",
        sql: include_str!("migrations/0.11.035_drop_child_commands.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 36,
        },
        name: "delete_sessions",
        sql: include_str!("migrations/0.11.036_delete_sessions.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 37,
        },
        name: "capture_terminal_states",
        sql: include_str!("migrations/0.11.037_capture_terminal_states.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(2),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.2.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(3),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.3.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(4),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.4.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(5),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.5.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(7),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.7.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(8),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.8.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(10),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.10.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(12),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.12.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(13),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.13.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(14),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.14.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(15),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.15.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(16),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.16.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(20),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.20.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(21),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.21.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(22),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.22.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(23),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.23.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(24),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.24.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(26),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.26.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(27),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.27.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(29),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.29.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(30),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.30.001_release.sql"),
    },
];
