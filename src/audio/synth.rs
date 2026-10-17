//! A tiny synthesizer for designing clicks, beeps and effects in code.

use std::f32::consts::TAU;

use super::SoundData;

const RATE: u32 = 44_100;

/// Basic wave shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wave {
    /// Smooth and pure.
    Sine,
    /// Hollow, retro.
    Square,
    /// Soft, between sine and square.
    Triangle,
    /// Bright and buzzy.
    Saw,
    /// White noise: clicks, steps, hiss, explosions.
    Noise,
}

/// Builds a sound from waves and envelopes. Pass it to
/// [`Audio::register`](super::Audio::register) or
/// [`Ui::set_sound_data`](crate::ui::Ui::set_sound_data), or call [`Synth::build`].
///
/// Settings like `volume` and `decay` apply to the part they are called on; parts added
/// with [`Synth::then`] and [`Synth::with`] keep their own settings.
///
/// ```
/// use audio_code::audio::Synth;
///
/// // A short, soft click.
/// let click = Synth::sine(1500.0).length(0.03).decay(120.0).volume(0.4).build();
///
/// // A rising "pling" made of two notes.
/// let pling = Synth::triangle(660.0).length(0.06)
///     .then(Synth::triangle(990.0).length(0.12).decay(25.0))
///     .build();
///
/// // A noisy thump: noise layered with a low sine sweep.
/// let thump = Synth::sine(150.0).sweep_to(50.0).length(0.15).decay(30.0)
///     .with(Synth::noise().length(0.05).decay(80.0).volume(0.3))
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct Synth {
    wave: Wave,
    from: f32,
    to: f32,
    length: f32,
    attack: f32,
    decay: f32,
    release: f32,
    volume: f32,
    layers: Vec<Synth>,
    next: Option<Box<Synth>>,
}

impl Synth {
    /// A synth with the given wave and frequency, 0.1 seconds long.
    pub fn new(wave: Wave, frequency: f32) -> Self {
        Self {
            wave,
            from: frequency,
            to: frequency,
            length: 0.1,
            attack: 0.002,
            decay: 0.0,
            release: 0.005,
            volume: 0.5,
            layers: Vec::new(),
            next: None,
        }
    }

    /// Sine wave.
    pub fn sine(frequency: f32) -> Self {
        Self::new(Wave::Sine, frequency)
    }

    /// Square wave.
    pub fn square(frequency: f32) -> Self {
        Self::new(Wave::Square, frequency)
    }

    /// Triangle wave.
    pub fn triangle(frequency: f32) -> Self {
        Self::new(Wave::Triangle, frequency)
    }

    /// Sawtooth wave.
    pub fn saw(frequency: f32) -> Self {
        Self::new(Wave::Saw, frequency)
    }

    /// White noise.
    pub fn noise() -> Self {
        Self::new(Wave::Noise, 0.0)
    }

    /// Silence, useful as a pause between parts: `a.then(Synth::silence(0.1)).then(b)`.
    pub fn silence(seconds: f32) -> Self {
        Self::sine(0.0).volume(0.0).length(seconds)
    }

    /// Length in seconds.
    pub fn length(mut self, seconds: f32) -> Self {
        self.length = seconds.max(0.0);
        self
    }

    /// Glide the frequency linearly to `frequency` (in hertz) over the whole length.
    pub fn sweep_to(mut self, frequency: f32) -> Self {
        self.to = frequency;
        self
    }

    /// Fade-in time in seconds (default 2 ms).
    pub fn attack(mut self, seconds: f32) -> Self {
        self.attack = seconds.max(0.0);
        self
    }

    /// Exponential fade out speed: the volume is multiplied by `e^(-rate * seconds)`.
    /// 0 = none (default), 10 rings for about 0.7 s, 30 is a pluck (0.2 s), 120 a click
    /// (60 ms), 300 a tick (20 ms).
    pub fn decay(mut self, rate: f32) -> Self {
        self.decay = rate.max(0.0);
        self
    }

    /// Linear fade-out at the end in seconds (default 5 ms, prevents clicks).
    pub fn release(mut self, seconds: f32) -> Self {
        self.release = seconds.max(0.0);
        self
    }

    /// Linear loudness, 0.0 (silent) to 1.0 (full scale), default 0.5.
    pub fn volume(mut self, volume: f32) -> Self {
        self.volume = volume;
        self
    }

    /// Plays `other` at the same time (mixed in).
    pub fn with(mut self, other: Synth) -> Self {
        self.layers.push(other);
        self
    }

    /// Plays `other` after this one.
    pub fn then(mut self, other: Synth) -> Self {
        let next = match self.next.take() {
            Some(next) => next.then(other),
            None => other,
        };
        self.next = Some(Box::new(next));
        self
    }

    fn render(&self) -> Vec<f32> {
        let frames = (self.length * RATE as f32) as usize;
        let mut out = Vec::with_capacity(frames);
        let mut phase = 0.0f32;
        for i in 0..frames {
            let t = i as f32 / RATE as f32;
            let progress = if frames > 1 { i as f32 / (frames - 1) as f32 } else { 0.0 };
            let freq = self.from + (self.to - self.from) * progress;
            phase = (phase + freq / RATE as f32).fract();
            let raw = match self.wave {
                Wave::Sine => (phase * TAU).sin(),
                Wave::Square => {
                    if phase < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                Wave::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
                Wave::Saw => 2.0 * phase - 1.0,
                Wave::Noise => fastrand::f32() * 2.0 - 1.0,
            };
            let mut env = 1.0;
            if self.attack > 0.0 {
                env *= (t / self.attack).min(1.0);
            }
            if self.decay > 0.0 {
                env *= (-t * self.decay).exp();
            }
            // Time until the last sample, so the release reaches exactly zero.
            let left = (frames - 1 - i) as f32 / RATE as f32;
            if self.release > 0.0 && left < self.release {
                env *= (left / self.release).max(0.0);
            }
            out.push(raw * env * self.volume);
        }
        for layer in &self.layers {
            let other = layer.render();
            if other.len() > out.len() {
                out.resize(other.len(), 0.0);
            }
            for (o, s) in out.iter_mut().zip(other) {
                *o += s;
            }
        }
        if let Some(next) = &self.next {
            out.extend(next.render());
        }
        out
    }

    /// Renders the sound.
    pub fn build(&self) -> SoundData {
        SoundData::from_samples(self.render(), 1, RATE)
    }
}

impl From<Synth> for SoundData {
    fn from(synth: Synth) -> Self {
        synth.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_add_up() {
        let s = Synth::sine(440.0).length(0.1).then(Synth::noise().length(0.05)).then(Synth::saw(100.0).length(0.05));
        assert!((s.build().duration() - 0.2).abs() < 0.001);
        let layered = Synth::sine(440.0).length(0.1).with(Synth::sine(880.0).length(0.3));
        assert!((layered.build().duration() - 0.3).abs() < 0.001);
    }

    #[test]
    fn stays_in_range_and_fades() {
        let data = Synth::square(300.0).volume(1.0).length(0.05).build();
        assert!(data.samples.iter().all(|s| s.abs() <= 1.0));
        assert!(data.samples.last().unwrap().abs() < 1e-3);
        assert!(data.samples[0].abs() < 1e-3);
    }
}
