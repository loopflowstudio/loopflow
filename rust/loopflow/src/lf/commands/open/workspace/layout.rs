//! One Task in a host's native panes: its conversation, beside its changes
//! when `--diff` asks. Hosts own the result; nothing here is persisted.
use crate::lf::commands::open::conversation::Attachment;
use std::path::Path;

use anyhow::Result;
use serde_json::Value;

use super::{lf_argv, tool_cwd, Native, Presentation, SqliteStore, ViewContent};
use loopflow_storage::work::selection::SplitAxis;

struct Tools<'a> {
    host: &'a Native,
    content: &'a ViewContent,
    cwd: &'a Path,
    conversations: bool,
    attachment: Option<Attachment>,
}

/// The native layout for one Task.
pub(super) fn layout(
    store: &SqliteStore,
    content: &ViewContent,
    repo: &Path,
    presentation: Presentation<'_>,
    host: &Native,
) -> Result<(Value, Option<Attachment>)> {
    let cwd = tool_cwd(content, repo);
    let mut tools = Tools {
        host,
        content,
        cwd,
        conversations: presentation.conversations,
        attachment: None,
    };
    let Some(location) = &content.location else {
        return Ok((tools.task_view()?, None));
    };
    anyhow::ensure!(
        location.machine_id
            == loopflow_execution::store::machines::local_machine(store)?
                .id
                .as_str(),
        "Task runs on Machine {}; no local tool was created",
        location.machine_id
    );
    anyhow::ensure!(
        cwd.is_dir(),
        "Recorded Task checkout is unavailable; no home-directory fallback"
    );
    let conversation = tools.conversation_pane(presentation.session)?;
    let native = if presentation.diff {
        host.split(SplitAxis::Vertical, 0.618, conversation, tools.changes()?)
    } else {
        conversation
    };
    Ok((native, tools.attachment))
}

impl Tools<'_> {
    fn task_view(&self) -> Result<Value> {
        Ok(self.host.block(
            "Task & workflow",
            self.cwd,
            lf_argv(self.cwd, &["task", "view", &self.content.work.id])?,
        ))
    }

    fn conversation(&mut self, session: Option<&str>) -> Result<Option<Value>> {
        if !self.conversations {
            return Ok(None);
        }
        let session = session.or(self.content.primary_session.as_deref());
        let task = session.is_none().then_some(self.content.work.id.as_str());
        let store = loopflow_execution::store::read_existing_registry()?;
        let context = super::super::work_context(self.cwd, task, None, session)?;
        let mut target = super::super::describe(
            store.as_ref(),
            &context,
            session.map(str::to_owned),
            false,
            None,
        )?;
        super::super::conversation::resolve(&mut target, true)?;
        let argv = if let Some(live) =
            super::super::conversation_view(&target)?.filter(|view| view.retains_attachment())
        {
            eprintln!("{}: {}", target.label, live.reason);
            vec![
                "/bin/sh".into(),
                "-c".into(),
                r#"printf '%s\n' "$1"; read -r answer"#.into(),
                "lf-open".into(),
                live.reason,
            ]
        } else {
            let attachment = Attachment::new()?;
            let lf = loopflow_execution::os_process::resolve_lf_binary()?;
            let argv = attachment.argv(target.resume_argv(&lf.to_string_lossy()));
            self.attachment = Some(attachment);
            argv
        };
        Ok(Some(self.host.block("Conversation", target.cwd(), argv)))
    }

    fn conversation_pane(&mut self, session: Option<&str>) -> Result<Value> {
        match self.conversation(session)? {
            Some(conversation) => Ok(conversation),
            None => self.task_view(),
        }
    }

    fn changes(&self) -> Result<Value> {
        let args = ["task", "diff", &self.content.work.id, "--base", "parent"];
        Ok(self
            .host
            .block("Changes", self.cwd, lf_argv(self.cwd, &args)?))
    }
}
