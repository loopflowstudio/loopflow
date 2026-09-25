use loopflow::id::WaveId;
use loopflow::work::chapter::{Chapter, ChapterId, ChapterPhase};
use time::OffsetDateTime;

pub fn current_chapter(wave_id: &WaveId, wave: &str, project_id: &str) -> Chapter {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    Chapter {
        id: ChapterId::parse("current").unwrap(),
        wave_id: wave_id.clone(),
        wave: wave.to_string(),
        project_id: project_id.to_string(),
        content: loopflow::ops::chapter::empty_plan(),
        predecessors: Vec::new(),
        predecessor_metrics: Vec::new(),
        tasks: Vec::new(),
        phase: ChapterPhase::Complete,
        created_at: now,
        activated_at: Some(now),
        completed_at: Some(now),
        error: None,
    }
}
