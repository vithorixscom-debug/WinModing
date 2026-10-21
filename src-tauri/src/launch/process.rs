use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::error::LauncherError;
use crate::launch::args::PlannedCommand;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub instance_id: String,
    pub stream: String,
    pub line: String,
}

#[derive(Clone, Default)]
pub struct RunningGames {
    inner: Arc<Mutex<std::collections::HashMap<String, Child>>>,
}

impl RunningGames {
    pub fn contains(&self, instance_id: &str) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains_key(instance_id)
    }

    pub async fn stop(&self, instance_id: &str) -> Result<(), LauncherError> {
        let child = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(instance_id);
        if let Some(mut child) = child {
            child
                .kill()
                .await
                .map_err(|err| crate::error::io_at(instance_id, err))?;
        }
        Ok(())
    }

    pub fn poll_exit(&self, instance_id: &str) -> Option<i32> {
        let mut map = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let child = map.get_mut(instance_id)?;
        match child.try_wait() {
            Ok(Some(status)) => {
                map.remove(instance_id);
                Some(status.code().unwrap_or(-1))
            }
            _ => None,
        }
    }
}

pub async fn spawn_game(
    games: &RunningGames,
    instance_id: &str,
    plan: &PlannedCommand,
    logs: mpsc::UnboundedSender<LogLine>,
) -> Result<u32, LauncherError> {
    if games.contains(instance_id) {
        return Err(LauncherError::AlreadyRunning {
            id: instance_id.to_string(),
        });
    }
    let mut command = Command::new(&plan.java_path);
    command
        .args(&plan.args)
        .current_dir(&plan.working_directory)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdin(std::process::Stdio::null())
        .kill_on_drop(false);
    if !plan.natives_directory.is_empty() {
        let current = std::env::var("PATH").unwrap_or_default();
        #[cfg(windows)]
        let combined = format!("{};{}", plan.natives_directory, current);
        #[cfg(not(windows))]
        let combined = format!("{}:{}", plan.natives_directory, current);
        command.env("PATH", combined);
    }
    command.env_remove("_JAVA_OPTIONS");
    for (key, value) in &plan.env_vars {
        command.env(key, value);
    }
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW — tokio::process::Command exposes creation_flags on Windows.
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|err| crate::error::io_at(&plan.java_path, err))?;
    let pid = child.id().unwrap_or(0);
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    pump(instance_id, "stdout", stdout, logs.clone());
    pump(instance_id, "stderr", stderr, logs);
    games
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(instance_id.to_string(), child);
    Ok(pid)
}

fn pump<R>(instance_id: &str, stream: &str, pipe: Option<R>, logs: mpsc::UnboundedSender<LogLine>)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    let Some(pipe) = pipe else {
        return;
    };
    let instance_id = instance_id.to_string();
    let stream = stream.to_string();
    tokio::spawn(async move {
        let mut lines = BufReader::new(pipe).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    if logs
                        .send(LogLine {
                            instance_id: instance_id.clone(),
                            stream: stream.clone(),
                            line,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(None) => break,
                Err(err) => {
                    let _ = logs.send(LogLine {
                        instance_id: instance_id.clone(),
                        stream: stream.clone(),
                        line: format!("log stream closed: {err}"),
                    });
                    break;
                }
            }
        }
    });
}

pub fn working_dir_exists(path: &Path) -> bool {
    path.is_dir()
}
