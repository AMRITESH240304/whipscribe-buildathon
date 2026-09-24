use std::collections::VecDeque;

pub const OUTPUT_RATE: u32 = 48_000;

// How far one source may run ahead before the other is treated as silent.
const MAX_LAG: usize = OUTPUT_RATE as usize / 2;

#[derive(Clone, Copy)]
pub enum Source {
    Mic,
    System,
}

struct Resampler {
    step: f64,
    pos: f64,
    prev: f32,
}

impl Resampler {
    fn new(rate: u32) -> Self {
        Self {
            step: rate as f64 / OUTPUT_RATE as f64,
            pos: 0.0,
            prev: 0.0,
        }
    }

    // Linear interpolation; `prev` carries the last sample across chunks.
    fn push(&mut self, input: &[f32], out: &mut VecDeque<f32>) {
        let prev = self.prev;
        let at = |i: f64| if i < 0.0 { prev } else { input[i as usize] };
        while self.pos < input.len() as f64 - 1.0 {
            let i = self.pos.floor();
            let (a, b) = (at(i), at(i + 1.0));
            out.push_back(a + (b - a) * (self.pos - i) as f32);
            self.pos += self.step;
        }
        self.pos -= input.len() as f64;
        self.prev = input.last().copied().unwrap_or(prev);
    }
}

struct Track {
    resampler: Resampler,
    queue: VecDeque<f32>,
}

impl Track {
    fn new(rate: u32) -> Self {
        Self {
            resampler: Resampler::new(rate),
            queue: VecDeque::new(),
        }
    }
}

pub struct Mixer {
    mic: Track,
    system: Option<Track>,
}

impl Mixer {
    pub fn new(mic_rate: u32, system_rate: Option<u32>) -> Self {
        Self {
            mic: Track::new(mic_rate),
            system: system_rate.map(Track::new),
        }
    }

    pub fn push(&mut self, source: Source, samples: &[f32]) {
        let track = match source {
            Source::Mic => Some(&mut self.mic),
            Source::System => self.system.as_mut(),
        };
        if let Some(track) = track {
            track.resampler.push(samples, &mut track.queue);
        }
    }

    // Mixes what both sources have delivered; `all` flushes everything left.
    pub fn drain(&mut self, all: bool) -> Vec<i16> {
        let mic = self.mic.queue.len();
        let n = match self.system.as_ref().map(|t| t.queue.len()) {
            Some(system) if !all => mic.min(system).max(mic.max(system).saturating_sub(MAX_LAG)),
            Some(system) => mic.max(system),
            None => mic,
        };
        (0..n)
            .map(|_| {
                let mic = self.mic.queue.pop_front().unwrap_or(0.0);
                let system = self
                    .system
                    .as_mut()
                    .and_then(|t| t.queue.pop_front())
                    .unwrap_or(0.0);
                ((mic + system).clamp(-1.0, 1.0) * i16::MAX as f32) as i16
            })
            .collect()
    }
}
