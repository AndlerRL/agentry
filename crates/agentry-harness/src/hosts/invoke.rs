use std::process::Stdio;
use std::time::Duration;

use agentry_audit::fix::is_safe_shell_arg;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::time::timeout;

use super::HostProfile;

#[derive(Debug, thiserror::Error)]
pub enum InvokeError {
    #[error("host '{host}' has no headless command configured")]
    NoCommand { host: String },
    #[error("unsafe argument in headless command for host '{host}': {arg}")]
    UnsafeArg { host: String, arg: String },
    #[error("failed to spawn {binary}: {err}")]
    Spawn { binary: String, err: String },
    #[error("timed out after {secs}s")]
    Timeout { secs: u64 },
    #[error("command exited with {status}")]
    Exit { status: String, stderr: String },
    #[error("io error: {0}")]
    Io(String),
}

pub struct InvokeResult {
    pub stdout: String,
    pub stderr: String,
}

pub fn split_args(template: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escaped = false;
    for ch in template.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => in_quotes = !in_quotes,
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

pub fn interpolate(template: &str, model: Option<&str>) -> Result<String, String> {
    let mut result = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        result.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            return Err(format!("unterminated placeholder in template: {template}"));
        };
        let key = &after[..end];
        match key {
            "model" => {
                let value = model.ok_or_else(|| {
                    "template uses {{model}} but no model is configured".to_string()
                })?;
                if !is_safe_shell_arg(value) {
                    return Err(format!("unsafe model value: {value}"));
                }
                result.push_str(value);
            }
            other => return Err(format!("unknown template placeholder {{{other}}}")),
        }
        rest = &after[end + 1..];
    }
    result.push_str(rest);
    Ok(result)
}

pub async fn invoke_headless(
    host: &HostProfile,
    command_template: Option<&str>,
    model: Option<&str>,
    prompt: &str,
    timeout_secs: u64,
) -> Result<InvokeResult, InvokeError> {
    let template = command_template
        .or(host.headless_command.as_deref())
        .ok_or_else(|| InvokeError::NoCommand {
            host: host.id.clone(),
        })?;
    let interpolated = interpolate(template, model).map_err(|err| InvokeError::UnsafeArg {
        host: host.id.clone(),
        arg: err,
    })?;
    let args = split_args(&interpolated);
    let (binary, rest) = args.split_first().ok_or_else(|| InvokeError::NoCommand {
        host: host.id.clone(),
    })?;
    for arg in rest {
        if !is_safe_shell_arg(arg) {
            return Err(InvokeError::UnsafeArg {
                host: host.id.clone(),
                arg: arg.clone(),
            });
        }
    }
    let mut child = Command::new(binary)
        .args(rest)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| InvokeError::Spawn {
            binary: binary.clone(),
            err: err.to_string(),
        })?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| InvokeError::Io("failed to take stdin".to_string()))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| InvokeError::Io("failed to take stdout".to_string()))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| InvokeError::Io("failed to take stderr".to_string()))?;
    let prompt = prompt.to_string();
    let write_task = tokio::spawn(async move {
        let _ = stdin.write_all(prompt.as_bytes()).await;
        let _ = stdin.flush().await;
    });
    let read_task = tokio::spawn(async move {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let _ = stdout.read_to_end(&mut out).await;
        let _ = stderr.read_to_end(&mut err).await;
        (out, err)
    });
    let status = timeout(Duration::from_secs(timeout_secs), child.wait()).await;
    let (stdout_bytes, stderr_bytes) = read_task.await.unwrap_or_default();
    write_task.await.ok();
    match status {
        Err(_) => {
            let _ = child.kill().await;
            Err(InvokeError::Timeout { secs: timeout_secs })
        }
        Ok(Err(err)) => Err(InvokeError::Io(err.to_string())),
        Ok(Ok(status)) => {
            if !status.success() {
                return Err(InvokeError::Exit {
                    status: status.to_string(),
                    stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
                });
            }
            Ok(InvokeResult {
                stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
                stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
            })
        }
    }
}

pub async fn invoke_headless_suspended(
    host: &HostProfile,
    command_template: Option<&str>,
    model: Option<&str>,
    prompt: &str,
    timeout_secs: u64,
) -> Result<InvokeResult, InvokeError> {
    let suspended = suspend_terminal();
    let result = invoke_headless(host, command_template, model, prompt, timeout_secs).await;
    if suspended {
        restore_terminal();
    }
    result
}

pub fn with_suspended_terminal<F, T>(f: F) -> T
where
    F: FnOnce() -> T,
{
    let suspended = suspend_terminal();
    let result = f();
    if suspended {
        restore_terminal();
    }
    result
}

fn suspend_terminal() -> bool {
    use crossterm::cursor::Show;
    use crossterm::execute;
    use crossterm::terminal::{disable_raw_mode, is_raw_mode_enabled, LeaveAlternateScreen};
    if !is_raw_mode_enabled().unwrap_or(false) {
        return false;
    }
    let mut stdout = std::io::stdout();
    let raw_ok = disable_raw_mode().is_ok();
    let leave_ok = execute!(stdout, LeaveAlternateScreen, Show).is_ok();
    raw_ok && leave_ok
}

fn restore_terminal() {
    use crossterm::execute;
    use crossterm::terminal::{enable_raw_mode, EnterAlternateScreen};
    let mut stdout = std::io::stdout();
    let _ = execute!(stdout, EnterAlternateScreen);
    let _ = enable_raw_mode();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_args_handles_quotes_and_spaces() {
        assert_eq!(
            split_args("claude -p --output-format text"),
            vec!["claude", "-p", "--output-format", "text"]
        );
        assert_eq!(
            split_args("ollama run \"qwen2.5-coder:7b\""),
            vec!["ollama", "run", "qwen2.5-coder:7b"]
        );
    }

    #[test]
    fn interpolate_replaces_model_placeholder() {
        let template = "ollama run {model}";
        assert_eq!(
            interpolate(template, Some("qwen2.5-coder:7b")).unwrap(),
            "ollama run qwen2.5-coder:7b"
        );
    }

    #[test]
    fn interpolate_fails_closed_on_unsafe_model() {
        let template = "ollama run {model}";
        let err = interpolate(template, Some("x; rm -rf ~")).unwrap_err();
        assert!(err.contains("unsafe model value"));
    }

    #[test]
    fn interpolate_requires_model_when_placeholder_used() {
        let template = "ollama run {model}";
        assert!(interpolate(template, None).is_err());
    }

    #[test]
    fn interpolate_rejects_unknown_placeholder() {
        assert!(interpolate("run {bogus}", Some("x")).is_err());
    }

    #[test]
    fn interpolate_passthrough_without_placeholders() {
        assert_eq!(interpolate("claude -p", Some("x")).unwrap(), "claude -p");
    }
}
