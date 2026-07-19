//! Host-mediated AI interpretation for the `ai` capability (ADR 0017 §4).
//!
//! A provider is resolved from client-scope config (`[ai.providers.<name>]`, [`AiProvider`]) and run
//! by the host — the guest never spawns a process or reaches the network. Two kinds:
//!
//! - `command`: a local executable run with an **explicit argv vector, no shell**. `{prompt}` and
//!   `{media}` are substituted as whole argv elements, so plugin-authored prompt text can never inject
//!   arguments or shell syntax. The cwd is the workspace directory so a relative `{media}` path
//!   resolves; the run is bounded by the provider timeout with `kill_on_drop`.
//! - `vision-api`: an OpenAI-compatible `POST {url}/chat/completions` with the media base64-encoded as
//!   an `image_url` data URI. The API key is read from the configured env var **at call time** and
//!   never logged; the endpoint must be HTTPS.
//!
//! The return is the model's **raw text**; the guest owns any JSON extraction (the host stays
//! schema-opaque, consistent with every other payload). Errors distinguish caller faults
//! ([`AiError::InvalidInput`] — unknown provider handled by the caller, bad url, missing key) from
//! provider/transport failures ([`AiError::Backend`]).

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use base64::Engine;
use genealogy_app::AiProvider;

use crate::net;

/// How many characters of a failing provider's stderr / API error body to include in an error.
const ERROR_EXCERPT_CHARS: usize = 500;

/// Why an AI interpretation failed. Policy/caller faults map to `invalid-input`, provider process and
/// transport failures to `backend` (the caller does the mapping).
#[derive(Debug)]
pub enum AiError {
    /// A caller fault: a bad media path, an unsupported provider kind, or a missing API-key env var.
    InvalidInput(String),
    /// A provider process, network, or protocol failure.
    Backend(String),
}

/// Runs the resolved `provider` over the media file, returning the model's raw text.
///
/// `media_abs` is the file's absolute path (read for the vision API); `media_rel` is the
/// workspace-relative path handed to a command provider as `{media}` (resolved against
/// `workspace_dir`, the command's cwd). `require_https` gates the vision-api endpoint scheme
/// (production always requires HTTPS; the capability tests relax it to reach a local mock server).
///
/// # Errors
///
/// [`AiError::InvalidInput`] for the reserved `plugin` kind, a missing API-key env var, or a non-HTTPS
/// endpoint; [`AiError::Backend`] for a provider process/transport failure, a timeout, or an
/// unreadable response.
pub async fn interpret(
    provider: &AiProvider,
    workspace_dir: &Path,
    media_abs: &Path,
    media_rel: &str,
    prompt: &str,
    require_https: bool,
) -> Result<String, AiError> {
    match provider {
        AiProvider::Command {
            command,
            args,
            timeout_secs,
        } => run_command(command, args, workspace_dir, media_rel, prompt, *timeout_secs).await,
        AiProvider::VisionApi {
            url,
            model,
            api_key_env,
            timeout_secs,
        } => run_vision_api(url, model, api_key_env, media_abs, prompt, *timeout_secs, require_https).await,
        AiProvider::Plugin => Err(AiError::InvalidInput(
            "the `plugin` AI provider kind is reserved and not yet supported (ADR 0017 §4)".to_owned(),
        )),
    }
}

/// Substitutes the `{prompt}`/`{media}` placeholders inside one argv element. Because each argument is
/// a separate vector element, the substituted value stays a **single** argv element — a prompt with
/// spaces, quotes, or shell metacharacters cannot split into extra arguments.
fn substitute(arg: &str, prompt: &str, media: &str) -> String {
    arg.replace("{prompt}", prompt).replace("{media}", media)
}

/// Truncates `text` to [`ERROR_EXCERPT_CHARS`] characters for inclusion in an error message.
fn excerpt(text: &str) -> String {
    text.chars().take(ERROR_EXCERPT_CHARS).collect()
}

/// Runs a `command`-kind provider: an explicit argv vector, no shell, cwd = the workspace, bounded by
/// `timeout_secs` with `kill_on_drop`. The model's text is the process stdout (UTF-8, lossy); a
/// non-zero exit is a backend error carrying a bounded stderr excerpt.
async fn run_command(
    command: &str,
    args: &[String],
    workspace_dir: &Path,
    media_rel: &str,
    prompt: &str,
    timeout_secs: u64,
) -> Result<String, AiError> {
    let substituted: Vec<String> = args.iter().map(|arg| substitute(arg, prompt, media_rel)).collect();
    let mut process = tokio::process::Command::new(command);
    process
        .args(&substituted)
        .current_dir(workspace_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let output = match tokio::time::timeout(Duration::from_secs(timeout_secs), process.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            return Err(AiError::Backend(format!("running AI provider `{command}`: {error}")));
        }
        Err(_) => {
            return Err(AiError::Backend(format!(
                "the AI provider `{command}` timed out after {timeout_secs}s"
            )));
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AiError::Backend(format!(
            "the AI provider `{command}` exited with {}: {}",
            output.status,
            excerpt(stderr.trim())
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Runs a `vision-api`-kind provider: an OpenAI-compatible chat-completions POST with the media
/// base64-encoded as an `image_url` data URI. The API key is read from `api_key_env` at call time and
/// used only as a bearer header — never logged or echoed in an error.
async fn run_vision_api(
    url: &str,
    model: &str,
    api_key_env: &str,
    media_abs: &Path,
    prompt: &str,
    timeout_secs: u64,
    require_https: bool,
) -> Result<String, AiError> {
    let key = std::env::var(api_key_env).map_err(|_| {
        AiError::InvalidInput(format!(
            "the AI provider's API-key environment variable `{api_key_env}` is not set"
        ))
    })?;
    let endpoint = format!("{}/chat/completions", url.trim_end_matches('/'));
    if require_https && !endpoint.starts_with("https://") {
        return Err(AiError::InvalidInput(
            "the AI provider url must be https (ADR 0017 §4)".to_owned(),
        ));
    }

    let bytes = tokio::fs::read(media_abs)
        .await
        .map_err(|error| AiError::Backend(format!("reading media {}: {error}", media_abs.display())))?;
    let mime = mime_guess::from_path(media_abs)
        .first_raw()
        .unwrap_or("application/octet-stream");
    let data_uri = format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    );

    let body = serde_json::json!({
        "model": model,
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": prompt },
                { "type": "image_url", "image_url": { "url": data_uri } },
            ],
        }],
    });

    let response = net::client()
        .post(&endpoint)
        .bearer_auth(&key)
        .json(&body)
        .timeout(Duration::from_secs(timeout_secs))
        .send()
        .await
        .map_err(|error| AiError::Backend(format!("calling the AI provider: {error}")))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| AiError::Backend(format!("reading the AI provider response: {error}")))?;
    if !status.is_success() {
        return Err(AiError::Backend(format!(
            "the AI provider returned {status}: {}",
            excerpt(text.trim())
        )));
    }
    extract_content(&text)
}

/// Extracts `choices[0].message.content` from an OpenAI-compatible chat-completions response. The
/// content is either a plain string or an array of content parts (`{ "type": "text", "text": … }`);
/// both are handled, the parts' text concatenated.
fn extract_content(body: &str) -> Result<String, AiError> {
    let json: serde_json::Value = serde_json::from_str(body)
        .map_err(|error| AiError::Backend(format!("the AI provider response was not JSON: {error}")))?;
    let content = json
        .pointer("/choices/0/message/content")
        .ok_or_else(|| AiError::Backend("the AI provider response had no choices[0].message.content".to_owned()))?;
    match content {
        serde_json::Value::String(text) => Ok(text.clone()),
        serde_json::Value::Array(parts) => {
            let mut out = String::new();
            for part in parts {
                if let Some(text) = part.get("text").and_then(serde_json::Value::as_str) {
                    out.push_str(text);
                }
            }
            Ok(out)
        }
        _ => Err(AiError::Backend(
            "the AI provider response content was neither a string nor content parts".to_owned(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{excerpt, extract_content, substitute};

    #[test]
    fn substitute_keeps_a_hostile_prompt_as_one_value() {
        // A prompt full of shell metacharacters is substituted verbatim into the single argv element,
        // never split — the argv vector is what makes injection inert.
        let hostile = r#""; rm -rf /tmp/x; echo pwned"#;
        assert_eq!(substitute("{prompt}", hostile, "media/scan.jpg"), hostile);
        assert_eq!(substitute("{media}", "irrelevant", "media/scan.jpg"), "media/scan.jpg");
        // A non-placeholder argument is passed through untouched.
        assert_eq!(substitute("-p", hostile, "media/scan.jpg"), "-p");
    }

    #[test]
    fn extract_content_reads_a_string_body() {
        let body = r#"{"choices":[{"message":{"content":"the transcribed text"}}]}"#;
        assert_eq!(extract_content(body).expect("string content"), "the transcribed text");
    }

    #[test]
    fn extract_content_concatenates_content_parts() {
        let body = r#"{"choices":[{"message":{"content":[
            {"type":"text","text":"part one "},
            {"type":"text","text":"part two"}
        ]}}]}"#;
        assert_eq!(extract_content(body).expect("parts content"), "part one part two");
    }

    #[test]
    fn extract_content_errors_on_a_missing_choice() {
        assert!(extract_content(r#"{"choices":[]}"#).is_err());
        assert!(extract_content("not json").is_err());
    }

    #[test]
    fn excerpt_bounds_length() {
        let long = "x".repeat(1000);
        assert_eq!(excerpt(&long).chars().count(), super::ERROR_EXCERPT_CHARS);
        assert_eq!(excerpt("short"), "short");
    }
}
