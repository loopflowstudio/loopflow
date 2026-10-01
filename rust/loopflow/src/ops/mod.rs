mod abandon;
pub mod chapter;
pub mod checkout;
pub(crate) mod child;
mod commit;
pub mod cron;
mod error;
mod flow;
pub(crate) mod flow_run;
pub(crate) mod flow_session;
pub(crate) mod git_operation;
pub(crate) mod human_session;
mod land;
pub mod linear_observe;
pub(crate) mod metrics;
pub mod pm;
mod pr;
pub mod pr_landing;
mod present;
mod progress;
pub mod project;
mod release;
mod run;
mod sync;
pub mod task;
pub mod task_actions;
pub mod task_execution;
pub mod task_flow;
#[doc(hidden)]
pub mod task_input;
pub(crate) mod task_pm;
pub(crate) mod telemetry;
pub mod trace;
pub(crate) mod util;
pub mod wt;

pub use abandon::{abandon_branch, AbandonOptions};
pub(crate) use commit::{checkpoint_task_restart, checkpoint_task_worktree};
pub use commit::{commit_workflow, commit_workflow_traced, CommitOptions};
pub use cron::{
    add_cron, daily_time_of, default_launch_agents_dir, latest_cron_receipt, list_cron_receipts,
    list_crons, parse_schedule, parse_wait_duration, receipt_is_stale, receipt_root,
    record_cron_preflight_failure, remove_cron, resolve_lf_path, run_cron, schedule_from_cron,
    sync_crons, trigger_cron, validate_cron_specs, wait_for_cron_receipt, CronHost, CronOutcome,
    CronReceipt, CronSchedule, CronSource, CronSpec, CronSyncResult, CronTargetKind, InstalledCron,
    SystemLaunchctl,
};
pub(crate) use cron::{cron_receipt_ids, list_cron_obligations, CronObligation};
pub use error::{OpsError, OpsResult};
pub use flow::execute_flow_command;
pub use land::{arm, mark_ready, submit, LandOptions};
pub(crate) use land::{finish_arm_after_sync, finish_submit_after_sync};
pub use pr::{create_or_update_pr, current_pr, PrInfo, PrOptions, PrResult};
pub use present::{present_pr_review, ReviewSurface};
pub use progress::{NullProgress, Progress};
pub use release::{
    bump_version, generate_release, preview_release_notes, release_bump, release_check,
    release_notes, release_publish, release_run, release_status, release_tag, MergedPr,
    ReleaseNotesDegradation, ReleaseNotesStatus, ReleaseReceipt, ReleaseRunOutcome,
    ReleaseStatusResult,
};
pub(crate) use run::render_task_context;
pub(crate) use run::{exec_task_worker, TaskWorkerExec};
#[doc(hidden)]
pub use run::{
    resolve_checkout_binding, resolve_execution_binding, resolve_work_binding,
    resolve_work_selection, WorkBinding, WorkSelection,
};
pub(crate) use sync::{abort_sync_after_authorization, continue_sync_after_authorization};
pub use sync::{
    abort_sync_for_resolution, continue_sync_for_resolution, plan_sync, recover_sync,
    sync_class_name, sync_strategy_name, sync_with_recovery, SyncClass, SyncOptions, SyncPlan,
    SyncRecovery, SyncStrategy, SyncVerification,
};
pub use trace::{hash_prompt, trace_enabled, MockResponses, OpTrace, Tracer};
pub use util::normalize_wave_name;

pub mod task_automation;
