//! Conversation identity and execution history. An invocation never owns feedback.

pub use loopflow_storage::session::*;

pub fn conversation(
    cwd: &std::path::Path,
    skill: &str,
    title: String,
) -> anyhow::Result<LfSession> {
    let skill = crate::graph::load_skill(skill, cwd)?;
    let wave = crate::work::wave::context::resolve_ambient_wave_name();
    let agent = crate::ops::task::resolve_task_agent(cwd, cwd, wave.as_deref(), Some(&skill))?;
    Ok(LfSession {
        captured: None,
        id: uuid::Uuid::new_v4().simple().to_string().into(),
        artifact_key: None,
        caller_artifact_key: None,
        input_published: false,
        cwd: cwd.to_path_buf(),
        skill: Some(skill.name),
        provider: Some(agent.harness.as_str().to_owned()),
        model: agent.model,
        node: None,
        iterations: None,
        task_id: None,
        wave_id: None,
        graph_lf_process_id: None,
        work_source: None,
        bound_at: None,
        interactive: true,
        repo: None,
        title,
        title_source: TitleSource::Generated,
        request: None,
        completed_at: None,
        created_at: loopflow_storage::rows::now_unix(),
    })
}

#[cfg(test)]
mod title_tests {
    #[test]
    fn generated_task_title_repairs_tokens_and_bounds_unicode_purpose() {
        let task =
            loopflow_model::durable::TaskId::from_raw("task_abcdef10000000000000000000000001");
        for title in [
            "abcdef Fix login",
            "lf-abcdef1 Fix login",
            "task_abcdef10000000000000000000000001 Fix login",
            "ENG-1 Fix login",
        ] {
            assert_eq!(
                super::task_session_title(title, &task, "ENG-2", Some("ENG-1")),
                "ENG-2 Fix login"
            );
        }
        let title = super::task_session_title(&"λ".repeat(100), &task, "ENG-2", None);
        assert!(title.starts_with("ENG-2 λ"));
        assert_eq!(title.chars().count(), 80);
    }
}
