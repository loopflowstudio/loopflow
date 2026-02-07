use std::time::SystemTime;

#[derive(Debug, Clone)]
pub enum EngineEvent {
    StepStarted {
        run_id: String,
        step: String,
        timestamp: SystemTime,
    },
    StepCompleted {
        run_id: String,
        step: String,
        exit_code: i32,
        timestamp: SystemTime,
    },
    StepFailed {
        run_id: String,
        step: String,
        error: String,
        timestamp: SystemTime,
    },
    FlowCompleted {
        run_id: String,
        timestamp: SystemTime,
    },
    FlowFailed {
        run_id: String,
        error: String,
        timestamp: SystemTime,
    },
}
