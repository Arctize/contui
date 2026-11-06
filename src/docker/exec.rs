//! Exec helper utilities

use std::pin::Pin;

use bollard::container::LogOutput;
use bollard::exec::{CreateExecOptions, ResizeExecOptions, StartExecOptions, StartExecResults};
use bollard::query_parameters::{AttachContainerOptionsBuilder, ResizeContainerTTYOptionsBuilder};
use futures::Stream;
use futures::StreamExt;
use tokio::io::AsyncWrite;

use crate::core::{DockerError, Result};
use crate::docker::DockerClient;

/// Default exec information from container inspect
#[derive(Debug, Clone)]
pub struct ExecDefaults {
    pub container_id: String,
    pub container_name: String,
    pub entrypoint: Vec<String>,
    pub cmd: Vec<String>,
    pub running: bool,
}

/// Active exec session handles
pub struct ExecStart {
    /// Exec id, or the container id for an attach session
    pub exec_id: String,
    /// True for an attach to the container's main process, false for an exec
    pub attach: bool,
    pub output: Pin<Box<dyn Stream<Item = Result<LogOutput>> + Send>>,
    pub input: Pin<Box<dyn AsyncWrite + Send>>,
}

/// Detect whether a command vector looks like a shell invocation.
pub fn looks_like_shell(cmd: &[String]) -> bool {
    if cmd.is_empty() {
        return false;
    }
    let first = cmd[0].rsplit('/').next().unwrap_or(&cmd[0]);
    matches!(first, "sh" | "bash" | "zsh" | "ash" | "dash")
        || cmd.iter().any(|c| c == "-lc" || c == "-c")
}

/// Select the command to run for exec based on entrypoint/cmd and shell detection.
pub fn select_exec_command(entrypoint: &[String], cmd: &[String]) -> Vec<String> {
    if looks_like_shell(entrypoint) || looks_like_shell(cmd) {
        let mut out = Vec::new();
        out.extend_from_slice(entrypoint);
        out.extend_from_slice(cmd);
        if out.is_empty() {
            return vec!["/bin/sh".to_string(), "-lc".to_string()];
        }
        // Ensure interactive shell if no arguments provided
        if out.len() == 1 && !out.iter().any(|c| c == "-i" || c == "-c" || c == "-lc") {
            out.push("-i".to_string());
        }
        out
    } else {
        vec!["/bin/sh".to_string(), "-lc".to_string()]
    }
}

impl DockerClient {
    /// Fetch default exec command information from container inspect
    pub async fn exec_defaults(&self, id: &str) -> Result<ExecDefaults> {
        let inspect = self
            .inner()
            .inspect_container(id, None)
            .await
            .map_err(|e| DockerError::Container(format!("Failed to inspect: {e}")))?;

        let config = inspect.config.unwrap_or_default();
        let entrypoint = config
            .entrypoint
            .unwrap_or_default()
            .into_iter()
            .map(Into::into)
            .collect::<Vec<String>>();
        let cmd = config
            .cmd
            .unwrap_or_default()
            .into_iter()
            .map(Into::into)
            .collect::<Vec<String>>();
        let name = inspect
            .name
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_string();
        let running = inspect.state.and_then(|s| s.running).unwrap_or(false);

        Ok(ExecDefaults {
            container_id: id.to_string(),
            container_name: name,
            entrypoint,
            cmd,
            running,
        })
    }

    /// Create and start an exec session with TTY enabled
    pub async fn start_exec_session(
        &self,
        container_id: &str,
        cmd: Vec<String>,
        cols: u16,
        rows: u16,
    ) -> Result<ExecStart> {
        let create = CreateExecOptions {
            attach_stdin: Some(true),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            tty: Some(true),
            cmd: Some(cmd),
            ..Default::default()
        };

        let exec = self
            .inner()
            .create_exec(container_id, create)
            .await
            .map_err(|e| DockerError::Container(format!("Failed to create exec: {e}")))?;

        let _ = self
            .inner()
            .resize_exec(
                &exec.id,
                ResizeExecOptions {
                    width: cols,
                    height: rows,
                },
            )
            .await;

        let started = self
            .inner()
            .start_exec(
                &exec.id,
                Some(StartExecOptions {
                    detach: false,
                    tty: true,
                    output_capacity: None,
                }),
            )
            .await
            .map_err(|e| DockerError::Container(format!("Failed to start exec: {e}")))?;

        match started {
            StartExecResults::Attached { output, input } => Ok(ExecStart {
                exec_id: exec.id,
                attach: false,
                output: Box::pin(
                    output
                        .map(|item| item.map_err(|e| DockerError::Container(e.to_string()).into())),
                ),
                input,
            }),
            StartExecResults::Detached => {
                Err(DockerError::Container("Exec detached unexpectedly".to_string()).into())
            }
        }
    }

    /// Attach to the container's main process (stdin, stdout and stderr).
    ///
    /// Detach with ctrl-p ctrl-q, as `docker attach` does.
    pub async fn start_attach_session(
        &self,
        container_id: &str,
        cols: u16,
        rows: u16,
    ) -> Result<ExecStart> {
        let options = AttachContainerOptionsBuilder::new()
            .stdin(true)
            .stdout(true)
            .stderr(true)
            .stream(true)
            .logs(false)
            .detach_keys("ctrl-p,ctrl-q")
            .build();

        let attached = self
            .inner()
            .attach_container(container_id, Some(options))
            .await
            .map_err(|e| DockerError::Container(format!("Failed to attach: {e}")))?;

        // Best effort: only meaningful for containers with a TTY
        let _ = self.resize_attach_session(container_id, cols, rows).await;

        Ok(ExecStart {
            exec_id: container_id.to_string(),
            attach: true,
            output: Box::pin(
                attached
                    .output
                    .map(|item| item.map_err(|e| DockerError::Container(e.to_string()).into())),
            ),
            input: attached.input,
        })
    }

    /// Resize the TTY of an attached container
    pub async fn resize_attach_session(
        &self,
        container_id: &str,
        cols: u16,
        rows: u16,
    ) -> Result<()> {
        self.inner()
            .resize_container_tty(
                container_id,
                ResizeContainerTTYOptionsBuilder::new()
                    .w(i32::from(cols))
                    .h(i32::from(rows))
                    .build(),
            )
            .await
            .map_err(|e| DockerError::Container(format!("Failed to resize: {e}")))?;
        Ok(())
    }

    /// Resize an exec TTY session
    pub async fn resize_exec_session(&self, exec_id: &str, cols: u16, rows: u16) -> Result<()> {
        self.inner()
            .resize_exec(
                exec_id,
                ResizeExecOptions {
                    width: cols,
                    height: rows,
                },
            )
            .await
            .map_err(|e| DockerError::Container(format!("Failed to resize exec: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_shell_entrypoint_with_cmd() {
        let entrypoint = vec!["/bin/sh".to_string(), "-lc".to_string()];
        let cmd = vec!["echo".to_string(), "hi".to_string()];
        let selected = select_exec_command(&entrypoint, &cmd);
        assert_eq!(selected, vec!["/bin/sh", "-lc", "echo", "hi"]);
    }

    #[test]
    fn adds_interactive_flag_when_only_shell() {
        let entrypoint = vec!["/bin/sh".to_string()];
        let selected = select_exec_command(&entrypoint, &[]);
        assert_eq!(selected, vec!["/bin/sh", "-i"]);
    }

    #[test]
    fn falls_back_to_sh_when_not_shell() {
        let entrypoint = vec!["/usr/bin/myapp".to_string()];
        let cmd = vec!["--port".to_string(), "8080".to_string()];
        let selected = select_exec_command(&entrypoint, &cmd);
        assert_eq!(selected, vec!["/bin/sh", "-lc"]);
    }

    #[test]
    fn detects_shell_by_basename() {
        assert!(looks_like_shell(&["/bin/bash".to_string()]));
        assert!(!looks_like_shell(&["/usr/bin/python".to_string()]));
    }

    #[test]
    fn exec_defaults_struct_is_cloneable() {
        let d = ExecDefaults {
            container_id: "id".into(),
            container_name: "name".into(),
            entrypoint: vec!["/bin/sh".into()],
            cmd: vec!["-lc".into()],
            running: true,
        };
        let _ = d.clone();
    }
}
