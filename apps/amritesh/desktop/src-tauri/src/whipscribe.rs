use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use reqwest::{multipart, RequestBuilder, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, State};

use crate::{
    err, net_err,
    recorder::{read_meta, recordings_dir, write_meta},
    secret, Result, NOT_CONNECTED,
};

const API: &str = "https://whipscribe.com/api/v1";
const KEYRING_USER: &str = "whipscribe-api-key";
const TIMEOUT: Duration = Duration::from_secs(20);

pub struct WhipScribe {
    http: reqwest::Client,
}

impl WhipScribe {
    pub fn new() -> Self {
        // No overall timeout: uploads of long meetings take as long as they take.
        Self {
            http: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(15))
                .build()
                .expect("http client"),
        }
    }

    fn get(&self, path: &str) -> Result<RequestBuilder> {
        Ok(self
            .http
            .get(format!("{API}{path}"))
            .header("X-API-Key", api_key()?)
            .timeout(TIMEOUT))
    }
}

#[derive(Deserialize)]
struct ApiError {
    error: String,
}

#[derive(Deserialize)]
struct Submitted {
    job_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct JobStatus {
    status: String,
    #[serde(default)]
    progress: f64,
    error: Option<String>,
    #[serde(default)]
    locked: bool,
}

fn api_key() -> Result<String> {
    secret(KEYRING_USER)?
        .get_password()
        .map_err(|_| NOT_CONNECTED.to_string())
}

pub(crate) fn transcript_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.transcript.json"))
}

async fn check(res: reqwest::Response) -> Result<reqwest::Response> {
    let status = res.status();
    if status.is_success() {
        return Ok(res);
    }
    if status == StatusCode::UNAUTHORIZED {
        return Err(NOT_CONNECTED.into());
    }
    match res.json::<ApiError>().await {
        Ok(body) => Err(body.error),
        Err(_) => Err(format!("WhipScribe returned an error ({status}).")),
    }
}

async fn send<T: DeserializeOwned>(request: RequestBuilder) -> Result<T> {
    let res = check(request.send().await.map_err(net_err)?).await?;
    res.json().await.map_err(err)
}

#[tauri::command]
pub fn whipscribe_status() -> bool {
    api_key().is_ok()
}

#[tauri::command]
pub async fn whipscribe_connect(state: State<'_, WhipScribe>, key: String) -> Result<()> {
    let key = key.trim();
    let check = state
        .http
        .get(format!("{API}/me"))
        .header("X-API-Key", key)
        .timeout(TIMEOUT);
    match send::<Value>(check).await {
        Err(e) if e == NOT_CONNECTED => {
            Err("That API key wasn't accepted. Check it and try again.".into())
        }
        other => other.map(drop),
    }?;
    secret(KEYRING_USER)?.set_password(key).map_err(err)
}

// Returns the job for this recording, uploading it first if needed.
// The Idempotency-Key makes a repeated upload of the same attempt reuse the job.
#[tauri::command]
pub async fn transcribe(
    app: AppHandle,
    state: State<'_, WhipScribe>,
    id: String,
    retry: bool,
) -> Result<String> {
    let dir = recordings_dir(&app)?;
    let mut meta = read_meta(&dir, &id).unwrap_or_default();
    match (&meta.job_id, retry) {
        (Some(job_id), false) => return Ok(job_id.clone()),
        (Some(_), true) => meta.attempt += 1,
        _ => {}
    }

    let file = tokio::fs::File::open(dir.join(format!("{id}.wav")))
        .await
        .map_err(err)?;
    let len = file.metadata().await.map_err(err)?.len();
    let file_name = format!("{}.wav", meta.title.replace('/', "-"));
    let part = multipart::Part::stream_with_length(file, len)
        .file_name(file_name)
        .mime_str("audio/wav")
        .map_err(err)?;
    let form = multipart::Form::new()
        .part("file", part)
        .text("source", "recording");

    let job: Submitted = send(
        state
            .http
            .post(format!("{API}/transcribe"))
            .header("X-API-Key", api_key()?)
            .header(
                "Idempotency-Key",
                format!("recording-{id}-{}", meta.attempt),
            )
            .multipart(form),
    )
    .await?;
    meta.job_id = Some(job.job_id.clone());
    write_meta(&dir, &id, &meta)?;
    Ok(job.job_id)
}

#[tauri::command]
pub async fn job_status(state: State<'_, WhipScribe>, job_id: String) -> Result<JobStatus> {
    let mut status: JobStatus = send(state.get(&format!("/jobs/{job_id}"))?).await?;
    // The docs say 0.0–1.0; the live API returns 0–100.
    if status.progress > 1.0 {
        status.progress /= 100.0;
    }
    Ok(status)
}

// Cached on disk after the first fetch, so finished transcripts open offline.
#[tauri::command]
pub async fn transcript(app: AppHandle, state: State<'_, WhipScribe>, id: String) -> Result<Value> {
    let dir = recordings_dir(&app)?;
    let path = transcript_path(&dir, &id);
    if let Ok(bytes) = tokio::fs::read(&path).await {
        return serde_json::from_slice(&bytes).map_err(err);
    }
    let job_id = read_meta(&dir, &id)
        .and_then(|m| m.job_id)
        .ok_or("This recording hasn't been transcribed yet.")?;
    let result: Value = send(state.get(&format!("/jobs/{job_id}/result?format=json"))?).await?;
    tokio::fs::write(&path, serde_json::to_vec(&result).map_err(err)?)
        .await
        .map_err(err)?;
    Ok(result)
}

// Removes the job from WhipScribe first, so a failed request never leaves
// a transcript on the server that the app no longer knows about.
#[tauri::command]
pub async fn delete_recording(
    app: AppHandle,
    state: State<'_, WhipScribe>,
    id: String,
) -> Result<()> {
    let dir = recordings_dir(&app)?;
    if let Some(job_id) = read_meta(&dir, &id).and_then(|m| m.job_id) {
        let res = state
            .http
            .delete(format!("{API}/jobs/{job_id}"))
            .header("X-API-Key", api_key()?)
            .timeout(TIMEOUT)
            .send()
            .await
            .map_err(net_err)?;
        if res.status() != StatusCode::NOT_FOUND {
            check(res).await?;
        }
    }
    for path in [
        dir.join(format!("{id}.wav")),
        dir.join(format!("{id}.json")),
        transcript_path(&dir, &id),
    ] {
        match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(err(e)),
            _ => {}
        }
    }
    Ok(())
}

#[derive(Deserialize)]
struct Stored {
    #[serde(default)]
    segments: Vec<StoredSegment>,
}

#[derive(Deserialize)]
struct StoredSegment {
    start: f64,
    text: String,
}

#[derive(Serialize)]
pub struct Match {
    id: String,
    start: f64,
    text: String,
}

// Local, over the transcripts saved on disk: instant and works offline.
// The MCP server only searches inside one transcript at a time.
#[tauri::command]
pub fn search_transcripts(app: AppHandle, query: String) -> Result<Vec<Match>> {
    let dir = recordings_dir(&app)?;
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let mut matches = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(err)?.flatten() {
        let path = entry.path();
        let Some(id) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".transcript.json"))
        else {
            continue;
        };
        let Some(stored) = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Stored>(&bytes).ok())
        else {
            continue;
        };
        matches.extend(
            stored
                .segments
                .into_iter()
                .filter(|s| s.text.to_lowercase().contains(&query))
                .map(|s| Match {
                    id: id.to_string(),
                    start: s.start,
                    text: s.text,
                }),
        );
    }
    Ok(matches)
}
