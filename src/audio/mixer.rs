//! The real-time mixer running on the audio thread.
//!
//! The game thread never touches mixer state directly. It sends [`Command`]s through a
//! channel which the mixer drains at the start of every audio block. Per-voice status flows
//! back through atomics in [`VoiceShared`].

use std::f32::consts::FRAC_PI_4;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Receiver;

use glam::Vec3;

use super::data::SoundSlot;
use crate::math;

/// Upper limit of simultaneously mixed voices. Older voices are dropped beyond this.
const MAX_VOICES: usize = 512;

/// Volume group a voice belongs to. Each group has its own volume control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Group {
    /// Sound effects (the default).
    Sound,
    /// Background music.
    Music,
    /// Menu and interface sounds.
    Ui,
}

impl Group {
    fn index(self) -> usize {
        match self {
            Group::Sound => 0,
            Group::Music => 1,
            Group::Ui => 2,
        }
    }
}

/// Distance model of a positioned sound.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Spatial {
    pub position: Vec3,
    pub min_distance: f32,
    pub max_distance: f32,
    pub rolloff: f32,
}

/// Status shared between a voice in the mixer and its [`Sound`](super::Sound) handles.
#[derive(Debug, Default)]
pub(crate) struct VoiceShared {
    pub finished: AtomicBool,
    /// Playback position in seconds, stored as `f64` bits.
    pub position: AtomicU64,
}

pub(crate) enum Change {
    Volume { target: f32, fade: f32 },
    Stop { fade: f32 },
    Pitch(f32),
    Pan(f32),
    Position(Vec3),
    Paused(bool),
    Looping(bool),
    Distance { min: f32, max: f32 },
    Seek(f32),
}

pub(crate) enum Command {
    Play(Box<Voice>),
    Change(u64, Change),
    StopAll { group: Option<Group>, fade: f32 },
    Listener { position: Vec3, yaw: f32, pitch: f32 },
    GroupVolume(Group, f32),
    MasterVolume(f32),
}

pub(crate) struct Voice {
    pub id: u64,
    pub slot: Arc<SoundSlot>,
    pub shared: Arc<VoiceShared>,
    pub group: Group,
    pub looping: bool,
    pub paused: bool,
    pub pitch: f32,
    pub pan: f32,
    pub spatial: Option<Spatial>,
    /// Current volume and where it is fading to.
    pub volume: f32,
    pub target_volume: f32,
    pub fade_step: f32,
    pub stop_after_fade: bool,

    pos: f64,
    gains: [f32; 2],
    lowpass: f32,
    lp_state: [f32; 2],
    fresh: bool,
}

impl Voice {
    pub(crate) fn new(id: u64, slot: Arc<SoundSlot>, shared: Arc<VoiceShared>, group: Group) -> Self {
        Self {
            id,
            slot,
            shared,
            group,
            looping: false,
            paused: false,
            pitch: 1.0,
            pan: 0.0,
            spatial: None,
            volume: 1.0,
            target_volume: 1.0,
            fade_step: 0.0,
            stop_after_fade: false,
            pos: 0.0,
            gains: [1.0, 1.0],
            lowpass: 1.0,
            lp_state: [0.0; 2],
            fresh: true,
        }
    }

    /// Starts the voice silent and fades it in.
    pub(crate) fn fade_in(&mut self, seconds: f32, rate: f32) {
        self.target_volume = self.volume;
        self.volume = 0.0;
        self.fade_step = fade_step(self.target_volume, seconds, rate);
    }

    fn finish(&mut self) {
        self.shared.finished.store(true, Ordering::Release);
    }

    fn is_finished(&self) -> bool {
        self.shared.finished.load(Ordering::Acquire)
    }

    fn apply(&mut self, change: Change, rate: f32) {
        match change {
            Change::Volume { target, fade } => {
                self.target_volume = target.max(0.0);
                self.stop_after_fade = false;
                self.fade_step = fade_step((self.target_volume - self.volume).abs(), fade, rate);
            }
            Change::Stop { fade } => {
                if fade <= 0.0 {
                    self.finish();
                } else {
                    self.target_volume = 0.0;
                    self.stop_after_fade = true;
                    self.fade_step = fade_step(self.volume, fade, rate);
                }
            }
            Change::Pitch(p) => self.pitch = p.clamp(0.01, 16.0),
            Change::Pan(p) => self.pan = p.clamp(-1.0, 1.0),
            Change::Position(p) => {
                if let Some(s) = &mut self.spatial {
                    s.position = p;
                }
            }
            Change::Paused(p) => self.paused = p,
            Change::Looping(l) => self.looping = l,
            Change::Distance { min, max } => {
                if let Some(s) = &mut self.spatial {
                    s.min_distance = min.max(0.01);
                    s.max_distance = max.max(s.min_distance + 0.01);
                }
            }
            Change::Seek(seconds) => {
                if let Some(data) = self.slot.get() {
                    self.pos = (seconds.max(0.0) as f64 * data.sample_rate as f64)
                        .min(data.frames().saturating_sub(1) as f64);
                }
            }
        }
    }

    /// Computes the left/right gains and low-pass coefficient for the current state.
    fn targets(&self, listener: &Listener, rate: f32, source_channels: usize) -> ([f32; 2], f32) {
        let Some(s) = &self.spatial else {
            // Plain stereo balance; centered sounds play at full volume in both ears.
            let p = self.pan;
            let gains = if source_channels == 1 && p == 0.0 {
                [1.0, 1.0]
            } else {
                [(1.0 - p).min(1.0), (1.0 + p).min(1.0)]
            };
            return (gains, 1.0);
        };

        // The offset in the listener's own frame: right, ahead and above the head.
        let rel = s.position - listener.position;
        let local_x = rel.dot(math::right(listener.yaw));
        let local_y = rel.dot(math::forward_3d(listener.yaw, listener.pitch));
        let local_z = rel.dot(math::up_3d(listener.yaw, listener.pitch));
        let distance = rel.length();

        if distance >= s.max_distance {
            return ([0.0, 0.0], 1.0);
        }
        let near = s.min_distance / distance.max(s.min_distance);
        let t = ((distance - s.min_distance) / (s.max_distance - s.min_distance)).clamp(0.0, 1.0);
        let mut gain = near.powf(s.rolloff) * (1.0 - t * t);

        let (pan, behind, below) = if distance > 1e-4 {
            ((local_x / distance) * 0.9, (-local_y / distance).max(0.0), (-local_z / distance).max(0.0))
        } else {
            (0.0, 0.0, 0.0)
        };
        // Sounds behind the listener are a bit quieter and duller, which helps telling
        // front from back on headphones. Sounds below are dulled less, so above (clear),
        // below (slightly dull) and behind (dull) can be told apart. Sounds straight above or
        // below are centered, because the pan only uses the sideways part of the offset.
        gain *= 1.0 - 0.3 * behind - 0.15 * below * (1.0 - behind);
        let muffle = behind.max(0.6 * below);
        let cutoff = math::lerp(20_000.0, 2_500.0, muffle);
        let lowpass = 1.0 - (-std::f32::consts::TAU * cutoff / rate).exp();

        let angle = (pan + 1.0) * FRAC_PI_4;
        ([angle.cos() * gain, angle.sin() * gain], lowpass)
    }

    /// Mixes this voice into `out` (stereo interleaved). Returns `false` once finished.
    fn render(&mut self, out: &mut [f32], listener: &Listener, group_gain: f32, rate: f32) -> bool {
        if self.is_finished() {
            return false;
        }
        let slot = self.slot.clone();
        let Some(data) = slot.get() else {
            if slot.has_failed() {
                self.finish();
                return false;
            }
            return true; // Still loading.
        };
        if self.paused {
            return true;
        }

        let frames = out.len() / 2;
        let total = data.frames();
        if total == 0 {
            self.finish();
            return false;
        }
        let ch = data.channels;
        let (target_gains, lowpass) = self.targets(listener, rate, ch);
        if self.fresh {
            self.gains = target_gains;
            self.lowpass = lowpass;
            self.fresh = false;
        }
        let start_gains = self.gains;
        let gain_step = [
            (target_gains[0] - start_gains[0]) / frames as f32,
            (target_gains[1] - start_gains[1]) / frames as f32,
        ];
        let lp_start = self.lowpass;
        let lp_step = (lowpass - lp_start) / frames as f32;
        let mono = self.spatial.is_some() || ch == 1;
        let step = data.sample_rate as f64 / rate as f64 * self.pitch as f64;
        let s = &data.samples;

        let read = |frame: usize, c: usize| s[frame * ch + c.min(ch - 1)];

        for i in 0..frames {
            // Volume fades.
            if self.volume != self.target_volume {
                if self.volume < self.target_volume {
                    self.volume = (self.volume + self.fade_step).min(self.target_volume);
                } else {
                    self.volume = (self.volume - self.fade_step).max(self.target_volume);
                }
            } else if self.stop_after_fade {
                self.finish();
                break;
            }

            let idx = self.pos as usize;
            let frac = (self.pos - idx as f64) as f32;
            let next = if idx + 1 < total {
                Some(idx + 1)
            } else if self.looping {
                Some(0)
            } else {
                None
            };
            let sample = |c: usize| {
                let a = read(idx, c);
                let b = next.map_or(0.0, |n| read(n, c));
                a + (b - a) * frac
            };
            let (mut l, mut r) = if mono {
                let m = if ch == 1 {
                    sample(0)
                } else {
                    (0..ch).map(sample).sum::<f32>() / ch as f32
                };
                (m, m)
            } else {
                (sample(0), sample(1))
            };

            let fi = i as f32;
            let lp = lp_start + lp_step * fi;
            if lp < 0.999 {
                self.lp_state[0] += lp * (l - self.lp_state[0]);
                self.lp_state[1] += lp * (r - self.lp_state[1]);
                l = self.lp_state[0];
                r = self.lp_state[1];
            } else {
                self.lp_state = [l, r];
            }

            let v = self.volume * group_gain;
            out[i * 2] += l * (start_gains[0] + gain_step[0] * fi) * v;
            out[i * 2 + 1] += r * (start_gains[1] + gain_step[1] * fi) * v;

            self.pos += step;
            if self.pos >= total as f64 {
                if self.looping {
                    self.pos %= total as f64;
                } else {
                    self.finish();
                    break;
                }
            }
        }
        self.gains = target_gains;
        self.lowpass = lowpass;
        let seconds = self.pos / data.sample_rate as f64;
        self.shared.position.store(seconds.to_bits(), Ordering::Relaxed);
        !self.is_finished()
    }
}

fn fade_step(distance: f32, seconds: f32, rate: f32) -> f32 {
    if seconds <= 0.0 { f32::INFINITY } else { distance / (seconds * rate) }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Listener {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

pub(crate) struct Mixer {
    rx: Receiver<Command>,
    voices: Vec<Voice>,
    listener: Listener,
    groups: [f32; 3],
    master: f32,
    rate: f32,
}

impl Mixer {
    pub(crate) fn new(rx: Receiver<Command>, rate: u32) -> Self {
        Self {
            rx,
            voices: Vec::with_capacity(64),
            listener: Listener { position: Vec3::ZERO, yaw: 0.0, pitch: 0.0 },
            groups: [1.0; 3],
            master: 1.0,
            rate: rate as f32,
        }
    }

    fn handle_commands(&mut self) {
        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                Command::Play(mut voice) => {
                    if self.voices.len() >= MAX_VOICES {
                        let oldest = self.voices.remove(0);
                        oldest.shared.finished.store(true, Ordering::Release);
                    }
                    if voice.fade_step == 0.0 {
                        voice.fade_step = f32::INFINITY;
                    }
                    self.voices.push(*voice);
                }
                Command::Change(id, change) => {
                    let rate = self.rate;
                    if let Some(v) = self.voices.iter_mut().find(|v| v.id == id) {
                        v.apply(change, rate);
                    }
                }
                Command::StopAll { group, fade } => {
                    let rate = self.rate;
                    for v in self.voices.iter_mut().filter(|v| group.is_none_or(|g| v.group == g)) {
                        v.apply(Change::Stop { fade }, rate);
                    }
                }
                Command::Listener { position, yaw, pitch } => self.listener = Listener { position, yaw, pitch },
                Command::GroupVolume(group, volume) => self.groups[group.index()] = volume.max(0.0),
                Command::MasterVolume(volume) => self.master = volume.max(0.0),
            }
        }
    }

    /// Renders stereo interleaved audio into `out`.
    pub(crate) fn render(&mut self, out: &mut [f32]) {
        self.handle_commands();
        out.fill(0.0);
        let (listener, rate, master, groups) = (self.listener, self.rate, self.master, self.groups);
        self.voices.retain_mut(|v| v.render(out, &listener, master * groups[v.group.index()], rate));
        for s in out.iter_mut() {
            *s = s.clamp(-1.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::data::SoundData;
    use super::*;
    use std::sync::mpsc::channel;

    fn voice(id: u64, data: SoundData) -> (Box<Voice>, Arc<VoiceShared>) {
        let shared = Arc::new(VoiceShared::default());
        let v = Voice::new(id, Arc::new(SoundSlot::ready(data)), shared.clone(), Group::Sound);
        (Box::new(v), shared)
    }

    #[test]
    fn plays_and_finishes() {
        let (tx, rx) = channel();
        let mut mixer = Mixer::new(rx, 44_100);
        let (v, shared) = voice(1, SoundData::from_samples(vec![0.5; 100], 1, 44_100));
        tx.send(Command::Play(v)).unwrap();
        let mut out = vec![0.0; 128];
        mixer.render(&mut out);
        assert!((out[0] - 0.5).abs() < 1e-4 && (out[1] - 0.5).abs() < 1e-4);
        assert!(!shared.finished.load(Ordering::Acquire));
        mixer.render(&mut out);
        assert!(shared.finished.load(Ordering::Acquire));
        assert!(mixer.voices.is_empty());
    }

    #[test]
    fn spatial_left_is_louder_in_left_ear() {
        let (tx, rx) = channel();
        let mut mixer = Mixer::new(rx, 44_100);
        let (mut v, _) = voice(1, SoundData::from_samples(vec![0.5; 1000], 1, 44_100));
        v.spatial = Some(Spatial { position: Vec3::new(-3.0, 0.0, 0.0), min_distance: 1.0, max_distance: 30.0, rolloff: 1.0 });
        tx.send(Command::Play(v)).unwrap();
        let mut out = vec![0.0; 64];
        mixer.render(&mut out);
        assert!(out[0] > out[1] * 3.0);
    }

    #[test]
    fn height_and_pitch_change_the_sound() {
        let (_, rx) = channel();
        let mixer = Mixer::new(rx, 44_100);
        let at = |position: Vec3, pitch: f32| {
            let (mut v, _) = voice(1, SoundData::from_samples(vec![0.5; 10], 1, 44_100));
            v.spatial = Some(Spatial { position, min_distance: 1.0, max_distance: 30.0, rolloff: 1.0 });
            let listener = Listener { position: Vec3::ZERO, yaw: 0.0, pitch };
            let ([l, r], lowpass) = v.targets(&listener, mixer.rate, 1);
            (l + r, lowpass)
        };
        let (above, above_lp) = at(Vec3::new(0.0, 0.0, 3.0), 0.0);
        let (below, below_lp) = at(Vec3::new(0.0, 0.0, -3.0), 0.0);
        let (behind, behind_lp) = at(Vec3::new(0.0, -3.0, 0.0), 0.0);
        assert!(above > below && below > behind);
        assert!(above_lp > below_lp && below_lp > behind_lp);
        // Looking straight down, a sound above is behind the head.
        let (_, looking_down_lp) = at(Vec3::new(0.0, 0.0, 3.0), -90.0);
        assert!((looking_down_lp - behind_lp).abs() < 1e-4);
    }
}
