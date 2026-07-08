use std::time::SystemTime;

#[derive(Debug, Clone)]
pub enum EngineEvent {
    SkillStarted {
        run_id: String,
        skill: String,
        timestamp: SystemTime,
    },
    SkillCompleted {
        run_id: String,
        skill: String,
        exit_code: i32,
        timestamp: SystemTime,
    },
    SkillFailed {
        run_id: String,
        skill: String,
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
