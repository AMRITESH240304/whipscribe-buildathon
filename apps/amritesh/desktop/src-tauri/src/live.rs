use std::{io::Cursor, time::Duration};

use reqwest::multipart::Part;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::{
    err,
    mixer::OUTPUT_RATE,
    recorder::WAV_SPEC,
    whipscribe::{self, Segment, WhipScribe},
    Result,
};

const RATE: usize = OUTPUT_RATE as usize;
const MIN_CLIP: usize = RATE * 6;
const MAX_CLIP: usize = RATE * 12;
const PAUSE: usize = RATE / 5;
const QUIET: u16 = 1000;
const POLL: Duration = Duration::from_secs(2);

struct Clip {
    recording_id: String,
    index: u32,
    offset: f64,
    samples: Vec<i16>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LiveText {
    recording_id: String,
    index: u32,
    segments: Vec<Segment>,
}

// Cuts the mixed audio into short clips at natural pauses while recording,
// and transcribes each one through the API as a rough live preview.
pub struct Chunker {
    recording_id: String,
    buf: Vec<i16>,
    offset: usize,
    index: u32,
    tx: UnboundedSender<Clip>,
}

impl Chunker {
    // None without an API key: live text is an extra, recording works without it.
    pub fn start(app: &AppHandle, recording_id: &str) -> Option<Self> {
        if !whipscribe::whipscribe_status() {
            return None;
        }
        let (tx, mut rx) = unbounded_channel();
        let app = app.clone();
        // One task per clip, so a slow job never holds back the ones after it.
        tauri::async_runtime::spawn(async move {
            while let Some(clip) = rx.recv().await {
                tauri::async_runtime::spawn(transcribe(app.clone(), clip));
            }
        });
        Some(Self {
            recording_id: recording_id.into(),
            buf: Vec::new(),
            offset: 0,
            index: 0,
            tx,
        })
    }

    pub fn push(&mut self, samples: &[i16]) {
        self.buf.extend_from_slice(samples);
        let tail = &self.buf[self.buf.len().saturating_sub(PAUSE)..];
        let paused = tail.iter().all(|s| s.unsigned_abs() < QUIET);
        if self.buf.len() >= MAX_CLIP || (self.buf.len() >= MIN_CLIP && paused) {
            let samples = std::mem::take(&mut self.buf);
            let len = samples.len();
            let _ = self.tx.send(Clip {
                recording_id: self.recording_id.clone(),
                index: self.index,
                offset: self.offset as f64 / RATE as f64,
                samples,
            });
            self.offset += len;
            self.index += 1;
        }
    }
}

fn encode(samples: &[i16]) -> Result<Vec<u8>> {
    let mut bytes = Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut bytes, WAV_SPEC).map_err(err)?;
    for &sample in samples {
        writer.write_sample(sample).map_err(err)?;
    }
    writer.finalize().map_err(err)?;
    Ok(bytes.into_inner())
}

async fn transcribe(app: AppHandle, clip: Clip) {
    let state = app.state::<WhipScribe>();
    let result = async {
        let part = Part::bytes(encode(&clip.samples)?)
            .file_name("live.wav")
            .mime_str("audio/wav")
            .map_err(err)?;
        let key = format!("live-{}-{}", clip.recording_id, clip.index);
        let job_id = state.upload(part, &key).await?;
        let segments = state.finished_segments(&job_id, POLL).await;
        // Clips are throwaway previews; keep them out of the job list.
        let _ = state.delete_job(&job_id).await;
        segments
    }
    .await;

    match result {
        Ok(segments) => {
            let segments = segments
                .into_iter()
                .map(|s| Segment {
                    start: s.start + clip.offset,
                    ..s
                })
                .collect();
            let _ = app.emit(
                "live-text",
                LiveText {
                    recording_id: clip.recording_id,
                    index: clip.index,
                    segments,
                },
            );
        }
        Err(e) => eprintln!("live clip {} failed: {e}", clip.index),
    }
}
