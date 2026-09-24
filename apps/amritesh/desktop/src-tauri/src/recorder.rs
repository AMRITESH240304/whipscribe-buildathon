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
    FromSample, SampleFormat, SizedSample, StreamConfig, SupportedStreamConfig,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::mixer::{Mixer, Source, OUTPUT_RATE};

type Result<T> = std::result::Result<T, String>;
type Wav = hound::WavWriter<BufWriter<File>>;
type Chunks = mpsc::Sender<(Source, Vec<f32>)>;

const WAV_SPEC: hound::WavSpec = hound::WavSpec {
    channels: 1,
    sample_rate: OUTPUT_RATE,
    bits_per_sample: 16,
    sample_format: hound::SampleFormat::Int,
};

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
    system_error: Option<String>,
    shared: Arc<Shared>,
    thread: JoinHandle<Result<()>>,
}

#[derive(Default)]
struct Shared {
    stop: AtomicBool,
    paused: AtomicBool,
    frames: AtomicU64,
    peaks: [AtomicU32; 2],
}

impl Shared {
    fn take_level(&self, source: Source) -> f32 {
        self.peaks[source as usize].swap(0, Relaxed) as f32 / i16::MAX as f32
    }
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
    mic_level: f32,
    system_level: f32,
    system_error: Option<String>,
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
    source: Source,
    tx: Chunks,
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
                let mono: Vec<f32> = data
                    .chunks(channels)
                    .map(|frame| {
                        frame.iter().map(|&s| s.to_sample::<f32>()).sum::<f32>() / channels as f32
                    })
                    .collect();
                let peak = mono.iter().fold(0f32, |m, s| m.max(s.abs())).min(1.0);
                shared.peaks[source as usize].fetch_max((peak * i16::MAX as f32) as u32, Relaxed);
                let _ = tx.send((source, mono));
            },
            |e| eprintln!("audio input error: {e}"),
            None,
        )
        .map_err(err)
}

fn open_stream(
    device: &cpal::Device,
    supported: SupportedStreamConfig,
    source: Source,
    tx: Chunks,
    shared: &Arc<Shared>,
) -> Result<(cpal::Stream, u32)> {
    let config = supported.config();
    let shared = shared.clone();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build_stream::<f32>(device, config, source, tx, shared),
        SampleFormat::I16 => build_stream::<i16>(device, config, source, tx, shared),
        SampleFormat::I32 => build_stream::<i32>(device, config, source, tx, shared),
        SampleFormat::U16 => build_stream::<u16>(device, config, source, tx, shared),
        other => Err(format!("Unsupported audio format: {other}")),
    }?;
    stream.play().map_err(err)?;
    Ok((stream, supported.sample_rate()))
}

fn open_microphone(
    host: &cpal::Host,
    tx: Chunks,
    shared: &Arc<Shared>,
) -> Result<(cpal::Stream, u32)> {
    let device = host.default_input_device().ok_or("No microphone found.")?;
    let config = device.default_input_config().map_err(err)?;
    open_stream(&device, config, Source::Mic, tx, shared)
}

// An input stream on an output device is a Core Audio tap on macOS 14.2+
// and WASAPI loopback on Windows.
fn open_system_audio(
    host: &cpal::Host,
    tx: Chunks,
    shared: &Arc<Shared>,
) -> Result<(cpal::Stream, u32)> {
    let device = host
        .default_output_device()
        .ok_or("No speakers or headphones found.")?;
    // cpal only taps devices without inputs; otherwise it would record this device's mic.
    if cfg!(target_os = "macos") && device.supports_input() {
        return Err(
            "Your output device has its own microphone (like a USB headset). \
                    Switch sound output to speakers or Bluetooth headphones to record the call."
                .into(),
        );
    }
    let config = device.default_output_config().map_err(err)?;
    open_stream(&device, config, Source::System, tx, shared)
}

fn write_samples(writer: &mut Wav, shared: &Shared, samples: &[i16]) -> Result<()> {
    for &sample in samples {
        writer.write_sample(sample).map_err(err)?;
    }
    shared.frames.fetch_add(samples.len() as u64, Relaxed);
    Ok(())
}

// Owns the audio streams (not Send on every platform) and the file writer.
// Flushing every second keeps the file on disk if the process is killed.
// Sends back why system audio is unavailable, if it is; the mic alone is enough to record.
fn record(
    path: PathBuf,
    shared: Arc<Shared>,
    ready: mpsc::Sender<Result<Option<String>>>,
) -> Result<()> {
    let host = cpal::default_host();
    let (tx, rx) = mpsc::channel();
    let setup = || -> Result<_> {
        let mic = open_microphone(&host, tx.clone(), &shared)?;
        let system = open_system_audio(&host, tx, &shared);
        let writer = hound::WavWriter::create(&path, WAV_SPEC).map_err(err)?;
        Ok((mic, system, writer))
    };
    let ((mic, mic_rate), system, mut writer) = match setup() {
        Ok(opened) => opened,
        Err(e) => {
            let _ = ready.send(Err(e.clone()));
            return Err(e);
        }
    };
    let (system, system_rate) = match system {
        Ok((stream, rate)) => {
            let _ = ready.send(Ok(None));
            (Some(stream), Some(rate))
        }
        Err(e) => {
            let _ = ready.send(Ok(Some(e)));
            (None, None)
        }
    };

    let mut mixer = Mixer::new(mic_rate, system_rate);
    let mut last_flush = Instant::now();
    while !shared.stop.load(Relaxed) {
        let received = rx.recv_timeout(Duration::from_millis(100)).into_iter();
        for (source, chunk) in received.chain(rx.try_iter()) {
            mixer.push(source, &chunk);
        }
        write_samples(&mut writer, &shared, &mixer.drain(false))?;
        if last_flush.elapsed() >= Duration::from_secs(1) {
            writer.flush().map_err(err)?;
            last_flush = Instant::now();
        }
    }

    drop((mic, system));
    for (source, chunk) in rx.try_iter() {
        mixer.push(source, &chunk);
    }
    write_samples(&mut writer, &shared, &mixer.drain(true))?;
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
    let system_error = ready_rx
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
        system_error,
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
        elapsed_secs: s.shared.frames.load(Relaxed) as f64 / OUTPUT_RATE as f64,
        paused: s.shared.paused.load(Relaxed),
        mic_level: s.shared.take_level(Source::Mic),
        system_level: s.shared.take_level(Source::System),
        system_error: s.system_error.clone(),
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
                title: meta
                    .as_ref()
                    .map_or("Untitled recording".into(), |m| m.title.clone()),
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
