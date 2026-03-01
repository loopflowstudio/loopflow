use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Result};
use bollard::container::LogOutput;
use bollard::models::{ContainerCreateBody, Mount, MountTypeEnum};
use bollard::query_parameters::{
    CreateContainerOptions, InspectContainerOptions, StartContainerOptions, WaitContainerOptions,
};
use bollard::Docker;
use futures_util::StreamExt;
use tracing::warn;

use crate::engine::stream::StreamParser;

use super::{
    container_host_config, handle_output_line, logs_options, remove_container_options,
    sanitize_token, stop_container_options, DockerCredentialMount, DockerExecutor, DockerWorkspace,
    OutputContext, AGENT_USER, CONTAINER_PREFIX_AGENT, CONTAINER_PREFIX_PREP, CONTAINER_WORKSPACE,
    LABEL_AGENT_ID, LABEL_MANAGED, LABEL_WAVE_ID, LABEL_WAVE_RUN_ID,
};

impl DockerExecutor {
    pub(super) fn build_container_name(agent_id: &str) -> String {
        format!("{CONTAINER_PREFIX_AGENT}{}", agent_id.replace('_', "-"))
    }

    pub(super) fn build_helper_container_name(label: &str) -> String {
        format!(
            "{CONTAINER_PREFIX_PREP}{}-{}",
            sanitize_token(label),
            uuid::Uuid::new_v4().simple()
        )
    }

    pub(super) fn build_agent_labels(
        agent_id: &str,
        wave_id: &str,
        wave_run_id: &str,
    ) -> HashMap<String, String> {
        HashMap::from([
            (LABEL_MANAGED.to_string(), "true".to_string()),
            (LABEL_AGENT_ID.to_string(), agent_id.to_string()),
            (LABEL_WAVE_ID.to_string(), wave_id.to_string()),
            (LABEL_WAVE_RUN_ID.to_string(), wave_run_id.to_string()),
        ])
    }

    pub(super) fn rewrite_command_paths(
        cmd: Vec<String>,
        host_root: &Path,
        container_root: &str,
    ) -> Vec<String> {
        let host_root = host_root.to_string_lossy();
        let host_root = if host_root == "/" {
            "/".to_string()
        } else {
            host_root.trim_end_matches('/').to_string()
        };
        cmd.into_iter()
            .map(|arg| Self::rewrite_command_arg_path(&arg, &host_root, container_root))
            .collect()
    }

    pub(super) fn rewrite_command_arg_path(
        arg: &str,
        host_root: &str,
        container_root: &str,
    ) -> String {
        if arg == host_root {
            return container_root.to_string();
        }

        match arg.strip_prefix(host_root) {
            Some(suffix) if suffix.starts_with('/') => format!("{container_root}{suffix}"),
            _ => arg.to_string(),
        }
    }

    pub(super) async fn collect_env(&self, program: Option<&str>) -> Vec<String> {
        let mut env: Vec<String> = Vec::new();

        // Inject provider tokens from the DB first so OAuth credentials win.
        for (env_var, value) in crate::lfd::provider_auth::provider_env_vars(&self.store).await {
            if let Some(program) = program {
                if !crate::lfd::provider_auth::provider_env_allowed_for_program(program, &env_var) {
                    continue;
                }
            }
            if env
                .iter()
                .any(|entry| entry.starts_with(&format!("{env_var}=")))
            {
                continue;
            }
            env.push(format!("{env_var}={value}"));
        }

        for name in &self.credential_env {
            if crate::lfd::provider_auth::is_api_key_env_name(name) {
                match program {
                    Some(program) => {
                        if !crate::lfd::provider_auth::api_key_env_allowed_for_program(
                            program, name,
                        ) {
                            continue;
                        }
                    }
                    None => continue,
                }
            }
            if env
                .iter()
                .any(|entry| entry.starts_with(&format!("{name}=")))
            {
                continue;
            }
            if let Ok(value) = std::env::var(name) {
                env.push(format!("{name}={value}"));
            }
        }

        // Inject provider tokens from the DB. One query, then use the
        // canonical env_var_for_token() mapping for each.
        if let Ok(tokens) = self.store.list_provider_tokens().await {
            for token in tokens {
                if let Some((env_var, value)) = crate::lfd::provider_auth::env_var_for_token(&token)
                {
                    if !env.iter().any(|e| e.starts_with(&format!("{env_var}="))) {
                        env.push(format!("{env_var}={value}"));
                    }
                }
            }
        }

        // When SSH credentials are mounted, build a GIT_SSH_COMMAND that:
        // - Bypasses the host SSH config (may have macOS-only options like UseKeychain)
        // - Accepts new host keys so first-contact clones succeed
        // - Explicitly lists available private keys from the mounted .ssh dir
        if let Some(ssh_mount) = self
            .credential_mounts
            .iter()
            .find(|m| m.container_path.ends_with(".ssh"))
        {
            let key_args = Self::discover_ssh_keys(&ssh_mount.host_path, &ssh_mount.container_path);
            env.push(format!(
                "GIT_SSH_COMMAND=ssh -F /dev/null \
                     -o StrictHostKeyChecking=accept-new \
                     -o UserKnownHostsFile=/tmp/.ssh_known_hosts{}",
                key_args,
            ));
        }

        env
    }

    pub(super) fn discover_ssh_keys(host_ssh_dir: &Path, container_ssh_dir: &str) -> String {
        let mut key_args = String::new();
        if let Ok(entries) = std::fs::read_dir(host_ssh_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                // Skip public keys, config, known_hosts, and other non-key files
                if name_str.ends_with(".pub")
                    || name_str.starts_with("known_hosts")
                    || name_str == "config"
                    || name_str == "authorized_keys"
                    || name_str.starts_with('.')
                {
                    continue;
                }
                // Only include regular files
                if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    key_args.push_str(&format!(" -i {container_ssh_dir}/{name_str}"));
                }
            }
        }
        key_args
    }

    pub(super) fn build_mounts_for(
        volume_name: &str,
        credential_mounts: &[DockerCredentialMount],
    ) -> Vec<Mount> {
        let mut mounts = vec![Mount {
            target: Some(CONTAINER_WORKSPACE.to_string()),
            source: Some(volume_name.to_string()),
            typ: Some(MountTypeEnum::VOLUME),
            read_only: Some(false),
            ..Default::default()
        }];

        for credential_mount in credential_mounts {
            mounts.push(Mount {
                target: Some(credential_mount.container_path.clone()),
                source: Some(credential_mount.host_path.to_string_lossy().to_string()),
                typ: Some(MountTypeEnum::BIND),
                read_only: Some(credential_mount.read_only),
                ..Default::default()
            });
        }

        mounts
    }

    pub(super) fn build_mounts(&self, volume_name: &str) -> Vec<Mount> {
        Self::build_mounts_for(volume_name, &self.credential_mounts)
    }

    pub(super) fn bind_mount(path: &Path, target: &str, read_only: bool) -> Mount {
        Mount {
            target: Some(target.to_string()),
            source: Some(path.to_string_lossy().to_string()),
            typ: Some(MountTypeEnum::BIND),
            read_only: Some(read_only),
            ..Default::default()
        }
    }

    pub(super) fn helper_mounts(
        &self,
        workspace: &DockerWorkspace,
        extra: Vec<Mount>,
    ) -> Vec<Mount> {
        let mut mounts = self.build_mounts(&workspace.volume.volume_name);
        mounts.extend(extra);
        mounts
    }

    pub(super) async fn remove_container(&self, container_id: &str) {
        if let Err(err) = self
            .docker
            .remove_container(container_id, Some(remove_container_options()))
            .await
        {
            warn!(container_id, error = %err, "failed to remove container");
        }
    }

    pub(super) async fn wait_for_container_with_logs(
        &self,
        container_id: &str,
        context: OutputContext,
    ) -> Result<i32> {
        let logs_task = tokio::spawn(Self::stream_logs(
            self.docker.clone(),
            container_id.to_string(),
            context,
        ));

        let mut wait_stream = self
            .docker
            .wait_container(container_id, None::<WaitContainerOptions>);
        let wait_result = tokio::time::timeout(self.agent_timeout, wait_stream.next()).await;

        match wait_result {
            Ok(Some(result)) => {
                let tail_lines = logs_task.await.unwrap_or_default();
                match result {
                    Ok(status) => {
                        let code = status.status_code as i32;
                        if code != 0 && !tail_lines.is_empty() {
                            let tail = tail_lines.join("\n");
                            warn!(
                                container_id,
                                exit_code = code,
                                tail = %tail,
                                "agent container exited with non-zero code"
                            );
                        }
                        Ok(code)
                    }
                    Err(err) => {
                        // Log tail output for diagnostics
                        if !tail_lines.is_empty() {
                            let tail = tail_lines.join("\n");
                            warn!(
                                container_id,
                                tail = %tail,
                                "agent container output before failure"
                            );
                        }
                        // Inspect the container for more context on failure
                        let inspect = self
                            .docker
                            .inspect_container(container_id, None::<InspectContainerOptions>)
                            .await;
                        let detail = match inspect {
                            Ok(info) => {
                                let state = info.state.as_ref();
                                format!(
                                    "status={} exit_code={} error={} oom={}",
                                    state
                                        .and_then(|s| s.status.as_ref().map(|v| format!("{v:?}")))
                                        .unwrap_or_default(),
                                    state.and_then(|s| s.exit_code).unwrap_or(-1),
                                    state.and_then(|s| s.error.as_deref()).unwrap_or(""),
                                    state.and_then(|s| s.oom_killed).unwrap_or(false),
                                )
                            }
                            Err(inspect_err) => format!("(inspect failed: {inspect_err})"),
                        };
                        Err(anyhow!("Docker container wait error: {err} [{detail}]"))
                    }
                }
            }
            Ok(None) => {
                let _tail = logs_task.await;
                Err(anyhow!("docker wait stream ended without status"))
            }
            Err(_) => {
                let _ = self
                    .docker
                    .stop_container(container_id, Some(stop_container_options()))
                    .await;
                let _tail = logs_task.await;
                Err(anyhow!(
                    "agent execution timed out after {}",
                    humantime::format_duration(self.agent_timeout)
                ))
            }
        }
    }

    pub(super) async fn stream_logs(
        docker: Docker,
        container_id: String,
        context: OutputContext,
    ) -> Vec<String> {
        let mut logs = docker.logs(&container_id, Some(logs_options(true)));
        const TAIL_SIZE: usize = 20;
        let mut tail: std::collections::VecDeque<String> = std::collections::VecDeque::new();

        let mut parser = StreamParser::new();
        let mut pending = String::new();
        while let Some(entry) = logs.next().await {
            match entry {
                Ok(LogOutput::StdOut { message })
                | Ok(LogOutput::StdErr { message })
                | Ok(LogOutput::Console { message }) => {
                    pending.push_str(&String::from_utf8_lossy(&message));
                    while let Some(newline) = pending.find('\n') {
                        let mut line = pending.drain(..=newline).collect::<String>();
                        if line.ends_with('\n') {
                            line.pop();
                        }
                        if line.ends_with('\r') {
                            line.pop();
                        }
                        if tail.len() >= TAIL_SIZE {
                            tail.pop_front();
                        }
                        tail.push_back(line.clone());
                        handle_output_line(&line, &mut parser, &context);
                    }
                }
                Err(err) => {
                    warn!(container_id, error = %err, "failed streaming container logs");
                    break;
                }
                _ => {}
            }
        }

        if !pending.is_empty() {
            if tail.len() >= TAIL_SIZE {
                tail.pop_front();
            }
            tail.push_back(pending.clone());
            handle_output_line(&pending, &mut parser, &context);
        }

        tail.into()
    }

    pub(super) async fn run_helper_command(
        &self,
        label: &str,
        cmd: Vec<String>,
        mounts: Vec<Mount>,
        working_dir: Option<String>,
    ) -> Result<String> {
        let container = self
            .docker
            .create_container(
                Some(CreateContainerOptions {
                    name: Some(Self::build_helper_container_name(label)),
                    ..Default::default()
                }),
                ContainerCreateBody {
                    image: Some(self.image.clone()),
                    cmd: Some(cmd),
                    working_dir,
                    env: Some(self.collect_env(None).await),
                    user: Some(AGENT_USER.to_string()),
                    host_config: Some(container_host_config(mounts, &self.limits)),
                    labels: Some(HashMap::from([(
                        LABEL_MANAGED.to_string(),
                        "true".to_string(),
                    )])),
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    ..Default::default()
                },
            )
            .await?;
        let container_id = container.id;

        if let Err(err) = self
            .docker
            .start_container(&container_id, None::<StartContainerOptions>)
            .await
        {
            self.remove_container(&container_id).await;
            return Err(err.into());
        }

        let mut wait_stream = self
            .docker
            .wait_container(&container_id, None::<WaitContainerOptions>);
        let wait_result = wait_stream.next().await;

        let mut logs = self.docker.logs(&container_id, Some(logs_options(false)));
        let mut output = String::new();
        while let Some(entry) = logs.next().await {
            match entry {
                Ok(LogOutput::StdOut { message })
                | Ok(LogOutput::StdErr { message })
                | Ok(LogOutput::Console { message }) => {
                    output.push_str(&String::from_utf8_lossy(&message));
                }
                Err(err) => {
                    warn!(container_id, error = %err, "failed reading helper logs");
                }
                _ => {}
            }
        }

        self.remove_container(&container_id).await;

        let status = wait_result
            .ok_or_else(|| anyhow!("docker wait stream ended without status"))?
            .map_err(|err| {
                anyhow!(
                    "docker helper '{}' wait failed: {:?} output={}",
                    label,
                    err,
                    output.trim()
                )
            })?;
        if status.status_code != 0 {
            return Err(anyhow!(
                "docker helper '{}' failed (exit {}): {}",
                label,
                status.status_code,
                output.trim()
            ));
        }
        Ok(output)
    }
}
