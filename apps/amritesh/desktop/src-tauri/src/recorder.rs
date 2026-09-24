use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed},
        mpsc, Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, SampleFormat, SizedSample, StreamConfig,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

type Result<T> = std::result::Result<T, String>;
type Wav = hound::WavWriter<BufWriter<File>>;

// Recordings are written as `<id>.partial` and renamed to `<id>.wav` on stop.
// A `.partial` left behind means the app died mid-recording.
const PARTIAL: &str = "partial";

#[derive(Default)]
pub struct Recorder {
    session: Mutex<Option<Session>>,
}

struct Session {
    id: String,
    title: String,
    dir: PathBuf,
    sample_rate: u32,
    shared: Arc<Shared>,
    thread: JoinHandle<Result<()>>,
}

#[derive(Default)]
struct Shared {
    stop: AtomicBool,
    paused: AtomicBool,
    frames: AtomicU64,
    peak: AtomicU32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    title: String,
    started_at: u64,
    #[serde(default)]
    recovered: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    title: String,
    elapsed_secs: f64,
    paused: bool,
    level: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    id: String,
    title: String,
    started_at: u64,
    duration_secs: f64,
    recovered: bool,
    path: PathBuf,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn recordings_dir(app: &AppHandle) -> Result<PathBuf> {
    let dir = app.path().app_data_dir().map_err(err)?.join("recordings");
    fs::create_dir_all(&dir).map_err(err)?;
    Ok(dir)
}

fn write_meta(dir: &Path, id: &str, meta: &Meta) -> Result<()> {
    let json = serde_json::to_vec_pretty(meta).map_err(err)?;
    fs::write(dir.join(format!("{id}.json")), json).map_err(err)
}

fn read_meta(dir: &Path, id: &str) -> Option<Meta> {
    serde_json::from_slice(&fs::read(dir.join(format!("{id}.json"))).ok()?).ok()
}

fn build_stream<T>(
    device: &cpal::Device,
    config: StreamConfig,
    tx: mpsc::Sender<Vec<i16>>,
    shared: Arc<Shared>,
) -> Result<cpal::Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels as usize;
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                if shared.paused.load(Relaxed) {
                    return;
                }
                let mono = data
                    .chunks(channels)
                    .map(|frame| {
                        let sum: f32 = frame.iter().map(|&s| s.to_sample::<f32>()).sum();
                        ((sum / channels as f32).clamp(-1.0, 1.0) * i16::MAX as f32) as i16
                    })
                    .collect();
                let _ = tx.send(mono);
            },
            |e| eprintln!("microphone error: {e}"),
            None,
        )
        .map_err(err)
}

fn open_microphone(
    path: &Path,
    shared: &Arc<Shared>,
) -> Result<(cpal::Stream, Wav, mpsc::Receiver<Vec<i16>>, u32)> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or("No microphone found.")?;
    let supported = device.default_input_config().map_err(err)?;
    let sample_rate = supported.sample_rate();
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let writer = hound::WavWriter::create(path, spec).map_err(err)?;

    let (tx, rx) = mpsc::channel();
    let config = supported.config();
    let shared = shared.clone();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build_stream::<f32>(&device, config, tx, shared),
        SampleFormat::I16 => build_stream::<i16>(&device, config, tx, shared),
        SampleFormat::I32 => build_stream::<i32>(&device, config, tx, shared),
        SampleFormat::U16 => build_stream::<u16>(&device, config, tx, shared),
        other => Err(format!("Unsupported microphone format: {other}")),
    }?;
    stream.play().map_err(err)?;
    Ok((stream, writer, rx, sample_rate))
}

fn write_chunk(writer: &mut Wav, shared: &Shared, chunk: &[i16]) -> Result<()> {
    for &sample in chunk {
        writer.write_sample(sample).map_err(err)?;
    }
    shared.frames.fetch_add(chunk.len() as u64, Relaxed);
    let peak = chunk.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
    shared.peak.fetch_max(peak as u32, Relaxed);
    Ok(())
}

// Owns the audio stream (not Send on every platform) and the file writer.
// Flushing every second keeps the file on disk if the process is killed.
fn record(
    path: PathBuf,
    shared: Arc<Shared>,
    ready: mpsc::Sender<Result<u32>>,
) -> Result<()> {
    let (stream, mut writer, rx) = match open_microphone(&path, &shared) {
        Ok((stream, writer, rx, sample_rate)) => {
            let _ = ready.send(Ok(sample_rate));
            (stream, writer, rx)
        }
        Err(e) => {
            let _ = fs::remove_file(&path);
            let _ = ready.send(Err(e.clone()));
            return Err(e);
        }
    };

    let mut last_flush = Instant::now();
    while !shared.stop.load(Relaxed) {
        if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(100)) {
            write_chunk(&mut writer, &shared, &chunk)?;
        }
        if last_flush.elapsed() >= Duration::from_secs(1) {
            writer.flush().map_err(err)?;
            last_flush = Instant::now();
        }
    }

    drop(stream);
    for chunk in rx.try_iter() {
        write_chunk(&mut writer, &shared, &chunk)?;
    }
    writer.finalize().map_err(err)
}

// Rewrites the RIFF and data sizes from the real file length, so a
// recording cut off by a crash plays back up to its last written sample.
fn repair_wav(path: &Path) -> io::Result<()> {
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;
    let mut header = [0u8; 256];
    let n = file.read(&mut header)?;
    let data = header[..n]
        .windows(4)
        .position(|w| w == b"data")
        .ok_or(io::ErrorKind::InvalidData)? as u64;
    let data_len = (file.metadata()?.len().saturating_sub(data + 8)) & !1;
    file.set_len(data + 8 + data_len)?;
    file.seek(SeekFrom::Start(4))?;
    file.write_all(&((data + data_len) as u32).to_le_bytes())?;
    file.seek(SeekFrom::Start(data + 4))?;
    file.write_all(&(data_len as u32).to_le_bytes())
}

pub fn recover_interrupted(app: &AppHandle) -> Result<()> {
    let dir = recordings_dir(app)?;
    for entry in fs::read_dir(&dir).map_err(err)?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some(PARTIAL) {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()).map(String::from) else {
            continue;
        };
        if let Err(e) = repair_wav(&path) {
            eprintln!("could not recover {}: {e}", path.display());
            continue;
        }
        fs::rename(&path, dir.join(format!("{id}.wav"))).map_err(err)?;
        if let Some(mut meta) = read_meta(&dir, &id) {
            meta.recovered = true;
            write_meta(&dir, &id, &meta)?;
        }
    }
    Ok(())
}

pub fn finish(recorder: &Recorder) -> Result<()> {
    let session = recorder
        .session
        .lock()
        .unwrap()
        .take()
        .ok_or("Nothing is recording.")?;
    session.shared.stop.store(true, Relaxed);
    let result = session
        .thread
        .join()
        .map_err(|_| "The recorder stopped unexpectedly.".to_string())?;
    let partial = session.dir.join(format!("{}.{PARTIAL}", session.id));
    fs::rename(&partial, session.dir.join(format!("{}.wav", session.id))).map_err(err)?;
    result
}

#[tauri::command]
pub fn start_recording(app: AppHandle, state: State<'_, Recorder>, title: String) -> Result<()> {
    let mut slot = state.session.lock().unwrap();
    if slot.is_some() {
        return Err("Already recording.".into());
    }

    let dir = recordings_dir(&app)?;
    let started_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(err)?
        .as_millis() as u64;
    let id = started_at.to_string();
    let shared = Arc::new(Shared::default());
    let (ready_tx, ready_rx) = mpsc::channel();

    let path = dir.join(format!("{id}.{PARTIAL}"));
    let thread_shared = shared.clone();
    let thread = std::thread::spawn(move || record(path, thread_shared, ready_tx));
    let sample_rate = ready_rx
        .recv()
        .map_err(|_| "The recorder stopped unexpectedly.".to_string())??;

    write_meta(
        &dir,
        &id,
        &Meta {
            title: title.clone(),
            started_at,
            recovered: false,
        },
    )?;
    *slot = Some(Session {
        id,
        title,
        dir,
        sample_rate,
        shared,
        thread,
    });
    Ok(())
}

#[tauri::command]
pub fn stop_recording(state: State<'_, Recorder>) -> Result<()> {
    finish(&state)
}

#[tauri::command]
pub fn set_paused(state: State<'_, Recorder>, paused: bool) -> Result<()> {
    let slot = state.session.lock().unwrap();
    let session = slot.as_ref().ok_or("Nothing is recording.")?;
    session.shared.paused.store(paused, Relaxed);
    Ok(())
}

#[tauri::command]
pub fn recording_status(state: State<'_, Recorder>) -> Option<Status> {
    let slot = state.session.lock().unwrap();
    slot.as_ref().map(|s| Status {
        title: s.title.clone(),
        elapsed_secs: s.shared.frames.load(Relaxed) as f64 / s.sample_rate as f64,
        paused: s.shared.paused.load(Relaxed),
        level: s.shared.peak.swap(0, Relaxed) as f32 / i16::MAX as f32,
    })
}

#[tauri::command]
pub fn list_recordings(app: AppHandle) -> Result<Vec<Recording>> {
    let dir = recordings_dir(&app)?;
    let mut recordings: Vec<Recording> = fs::read_dir(&dir)
        .map_err(err)?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("wav"))
        .filter_map(|path| {
            let id = path.file_stem()?.to_str()?.to_string();
            let meta = read_meta(&dir, &id);
            let duration_secs = hound::WavReader::open(&path)
                .map(|r| r.duration() as f64 / r.spec().sample_rate as f64)
                .unwrap_or(0.0);
            Some(Recording {
                title: meta.as_ref().map_or("Untitled recording".into(), |m| m.title.clone()),
                started_at: meta.as_ref().map_or(0, |m| m.started_at),
                recovered: meta.is_some_and(|m| m.recovered),
                duration_secs,
                path,
                id,
            })
        })
        .collect();
    recordings.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(recordings)
}
