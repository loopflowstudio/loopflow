//! One headless provider turn from an already admitted launch.
use super::{ApprovalPolicy, Launch};
use crate::agent::{AgentFailure, AgentProcessResult, ProcessConfig};
use crate::error::CoreError;
use loopflow_model::chat::types::{ConversationEvent, Lifecycle};
use std::io::Write;
use std::time::Duration;

pub fn run(launch: &Launch<'_>, process: &ProcessConfig) -> Result<AgentProcessResult, CoreError> {
    let runtime = process.runtime.as_ref();
    let provider = launch.harness.kind().as_str();
    let mut config = launch.config.clone();
    let prompt = std::mem::take(&mut config.task_prompt);
    let launch_worktree = config.cwd.clone().or_else(|| std::env::current_dir().ok());
    if let Some(cwd) = launch_worktree.as_deref() {
        crate::ops::git_operation::prepare_agent_process(cwd)
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
    }
    crate::runtime::block_on(async {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let launch = Launch {
            config: &config,
            ..*launch
        };
        let mut harness = launch
            .harness
            .driver(&launch, event_tx, ApprovalPolicy::AutoApprove);
        harness.set_runtime(process.runtime.clone());
        harness
            .start()
            .await
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
        let agent_session = harness.agent_session();
        if let Some(runtime) = runtime {
            runtime.observe_provider(agent_session.clone());
        }

        let drive = async {
            harness
                .send_input(&prompt)
                .await
                .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
            let mut stdout = String::new();
            let mut stderr = String::new();
            let mut exit_code = None;
            let mut failure = None;
            let mut input_tick = tokio::time::interval(Duration::from_millis(250));
            input_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            let mut activity_tick =
                tokio::time::interval(crate::session_record::activity::SAMPLE_INTERVAL);
            activity_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            while exit_code.is_none() {
                tokio::select! {
                    _ = input_tick.tick(), if process.task_input.is_some() => {
                        process.task_input.as_ref().expect("Task input is enabled")
                            .poll(harness.as_mut()).await
                            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
                    }
                    _ = activity_tick.tick(), if runtime.is_some() => {
                        if let Some(runtime) = runtime { runtime.observe_activity(harness.pid()).await; }
                    }
                    event = event_rx.recv() => {
                        let Some(event) = event else {
                            stderr.push_str(&format!("{provider} event stream closed\n"));
                            exit_code = Some(1);
                            continue;
                        };
                        if let Some(runtime) = runtime {
                            runtime.observe_provider(harness.agent_session());
                            runtime.record_conversation(event.clone());
                        }
                        match event {
                            ConversationEvent::TextDelta { content, .. } => {
                                stdout.push_str(&content);
                                if process.stream {
                                    print!("{content}");
                                    let _ = std::io::stdout().flush();
                                }
                            }
                            ConversationEvent::Error { code, message, .. } => {
                                report(&launch, &code, &message, &mut stderr, &mut failure);
                                if launch.harness.terminal_error(&code)
                                {
                                    exit_code = Some(1);
                                }
                            }
                            ConversationEvent::TurnCompleted { status, .. } => {
                                exit_code = Some(match status {
                                    Lifecycle::Completed => 0,
                                    Lifecycle::Interrupted => 130,
                                    _ => 1,
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
            while let Ok(event) = event_rx.try_recv() {
                if let Some(runtime) = runtime {
                    runtime.record_conversation(event.clone());
                }
                if let ConversationEvent::Error { code, message, .. } = event {
                    report(&launch, &code, &message, &mut stderr, &mut failure);
                }
            }
            if process.stream && !stdout.ends_with('\n') {
                println!();
            }
            let exit_code = exit_code.expect("event loop stops with an exit code");
            Ok(AgentProcessResult {
                exit_code,
                stdout,
                stderr,
                agent_session: harness.agent_session(),
                // A turn the provider finished was served, whatever an error
                // said along the way.
                failure: failure.filter(|_| exit_code != 0),
            })
        };
        let result = match process.timeout {
            Some(timeout) => match tokio::time::timeout(timeout, drive).await {
                Ok(result) => result,
                Err(_) => Err(CoreError::ExecutionFailed(format!(
                    "agent timed out after {}ms",
                    timeout.as_millis()
                ))),
            },
            None => drive.await,
        };
        harness
            .stop()
            .await
            .map_err(|error| CoreError::ExecutionFailed(error.to_string()))?;
        result
    })
}

/// Keep an error the provider reported: its text is what a person reads as
/// the reason, and the gravest failure of the turn decides what happens next.
fn report(
    launch: &Launch<'_>,
    code: &str,
    message: &str,
    stderr: &mut String,
    failure: &mut Option<AgentFailure>,
) {
    stderr.push_str(&format!("{code}: {message}\n"));
    *failure = [*failure, launch.harness.classify_failure(code, message)]
        .into_iter()
        .flatten()
        .reduce(AgentFailure::graver);
}
