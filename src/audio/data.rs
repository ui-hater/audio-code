//! Decoded sound data and file loading.

use std::fs::File;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use symphonia::core::audio::sample::Sample;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::error::{Error, Result};

/// Decoded PCM audio kept in memory: interleaved `f32` samples.
///
/// You normally never touch this directly; the engine loads files for you.
/// It is public so you can generate sounds in code and register them with
/// [`Audio::register`](crate::audio::Audio::register).
#[derive(Debug, Clone)]
pub struct SoundData {
    pub(crate) samples: Vec<f32>,
    pub(crate) channels: usize,
    pub(crate) sample_rate: u32,
}

impl SoundData {
    /// Creates sound data from interleaved samples in the range `-1.0..=1.0`.
    ///
    /// # Panics
    /// If `channels` is 0 or `sample_rate` is 0.
    pub fn from_samples(samples: Vec<f32>, channels: usize, sample_rate: u32) -> Self {
        assert!(channels > 0 && sample_rate > 0, "channels and sample rate must be positive");
        Self { samples, channels, sample_rate }
    }

    /// Generates a sine tone with short fades, handy for prototypes and beeps.
    /// `frequency` is the pitch in hertz (440 = A, 880 = one octave higher), `seconds` the
    /// length and `volume` the linear loudness from 0.0 to 1.0.
    pub fn tone(frequency: f32, seconds: f32, volume: f32) -> Self {
        Self::generate(seconds, |t| (t * frequency * std::f32::consts::TAU).sin() * volume)
    }

    /// Generates mono sound from a function of time in seconds.
    /// Adds 5 ms fades at both ends to avoid clicks.
    pub fn generate(seconds: f32, mut f: impl FnMut(f32) -> f32) -> Self {
        const RATE: u32 = 44_100;
        let frames = (seconds.max(0.0) * RATE as f32) as usize;
        let fade = (RATE as usize / 200).min(frames / 2).max(1);
        let samples = (0..frames)
            .map(|i| {
                let env = (i.min(frames - 1 - i) as f32 / fade as f32).min(1.0);
                f(i as f32 / RATE as f32) * env
            })
            .collect();
        Self { samples, channels: 1, sample_rate: RATE }
    }

    /// Length in seconds.
    pub fn duration(&self) -> f32 {
        self.frames() as f32 / self.sample_rate as f32
    }

    /// Number of channels (1 = mono, 2 = stereo).
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Sample rate in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub(crate) fn frames(&self) -> usize {
        self.samples.len() / self.channels
    }

    /// Loads and decodes a sound file (wav, ogg, mp3, flac, aiff, m4a/aac, ...).
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let file = File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let decode_err = |e: SymphoniaError| Error::Decode(format!("{}: {e}", path.display()));
        let mut format = symphonia::default::get_probe()
            .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
            .map_err(decode_err)?;
        let track = format
            .default_track(TrackType::Audio)
            .ok_or_else(|| Error::Decode(format!("{}: no audio track", path.display())))?;
        let track_id = track.id;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .ok_or_else(|| Error::Decode(format!("{}: unsupported codec", path.display())))?;
        let mut decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(decode_err)?;

        let mut samples = Vec::new();
        let mut chunk = Vec::new();
        let mut channels = 0;
        let mut sample_rate = 0;
        loop {
            let packet = match format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => break,
                // Some files end with a truncated packet; keep what we have.
                Err(SymphoniaError::IoError(_)) => break,
                Err(e) => return Err(decode_err(e)),
            };
            if packet.track_id != track_id {
                continue;
            }
            match decoder.decode(&packet) {
                Ok(buf) => {
                    channels = buf.spec().channels().count();
                    sample_rate = buf.spec().rate();
                    chunk.resize(buf.samples_interleaved(), f32::MID);
                    buf.copy_to_slice_interleaved(&mut chunk);
                    samples.extend_from_slice(&chunk);
                }
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(e) => return Err(decode_err(e)),
            }
        }
        if channels == 0 || sample_rate == 0 {
            return Err(Error::Decode(format!("{}: file contains no audio", path.display())));
        }
        Ok(Self { samples, channels, sample_rate })
    }
}

/// A slot that receives sound data, possibly later (music loads in the background).
#[derive(Debug, Default)]
pub(crate) struct SoundSlot {
    data: OnceLock<SoundData>,
    failed: AtomicBool,
}

impl SoundSlot {
    pub(crate) fn ready(data: SoundData) -> Self {
        let slot = Self::default();
        let _ = slot.data.set(data);
        slot
    }

    pub(crate) fn fill(&self, data: SoundData) {
        let _ = self.data.set(data);
    }

    pub(crate) fn fail(&self) {
        self.failed.store(true, Ordering::Release);
    }

    pub(crate) fn get(&self) -> Option<&SoundData> {
        self.data.get()
    }

    pub(crate) fn has_failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
}
