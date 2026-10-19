//! High level sound: one-liners for effects, 3D positioned sounds and music.
//! Available as `ctx.audio`.
//!
//! ```no_run
//! # use audio_code::prelude::*;
//! # fn f(ctx: &mut Context) {
//! // Fire and forget.
//! ctx.audio.play("door_open");
//!
//! // A looping sound placed in the world. Keep the handle to move or stop it later.
//! let engine = ctx.audio.play_looped_at("car_engine", vec3(10.0, 5.0, 0.0));
//! engine.set_pitch(1.3);
//!
//! // Full control with the builder.
//! ctx.audio.sound("explosion").volume(0.8).pitch(0.9).at(vec3(3.0, 0.0, 0.0)).play();
//!
//! // Background music with automatic cross fading.
//! ctx.audio.play_music("music/level1");
//! # }
//! ```
//!
//! ## Finding files
//!
//! Sound names are resolved like this:
//!
//! 1. The name as given (`"sounds/step.ogg"`).
//! 2. Inside the sound directory set with [`Engine::sound_dir`](crate::Engine::sound_dir)
//!    (default `"sounds"`).
//! 3. In a macOS app bundle, both of the above inside `Contents/Resources`.
//! 4. If the name has no extension, `.ogg`, `.wav`, `.mp3`, `.flac`, `.opus` and `.m4a` are tried.
//!
//! So with the default settings `ctx.audio.play("step")` finds `sounds/step.ogg`.
//!
//! Loaded files are cached, the second play of the same name costs nothing.
//! Missing or broken files print a warning once and return a silent handle, so a typo never
//! crashes your game.

mod data;
mod mixer;
mod synth;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Sender, channel};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use glam::Vec3;

pub use data::SoundData;
pub use mixer::Group;
pub use synth::{Synth, Wave};

use crate::error::{Error, Result};
use data::SoundSlot;
use mixer::{Change, Command, Mixer, Spatial, Voice, VoiceShared};

const EXTENSIONS: [&str; 6] = ["ogg", "wav", "mp3", "flac", "opus", "m4a"];

/// The audio system. Reach it through `ctx.audio`.
pub struct Audio {
    tx: Sender<Command>,
    _stream: Option<cpal::Stream>,
    sample_rate: u32,
    sound_dir: PathBuf,
    cache: HashMap<String, Arc<SoundSlot>>,
    warned: HashSet<String>,
    next_id: u64,
    music: Option<Sound>,
    music_name: Option<String>,
    listener: (Vec3, f32, f32),
    master: f32,
    groups: HashMap<Group, f32>,
    default_distance: (f32, f32),
    default_rolloff: f32,
}

impl std::fmt::Debug for Audio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Audio")
            .field("sample_rate", &self.sample_rate)
            .field("output", &self._stream.is_some())
            .field("cached_sounds", &self.cache.len())
            .finish()
    }
}

impl Audio {
    /// Opens the default output device. If that fails, a silent audio system is returned
    /// and a warning is printed, so the game still runs.
    pub(crate) fn new(sound_dir: PathBuf) -> Self {
        match Self::open(sound_dir.clone()) {
            Ok(audio) => audio,
            Err(e) => {
                eprintln!("audio-code: {e}; continuing without sound");
                Self::silent(sound_dir)
            }
        }
    }

    /// An audio system without an output device. Everything works, nothing is heard.
    pub fn silent(sound_dir: impl Into<PathBuf>) -> Self {
        let (tx, _) = channel();
        Self::with_parts(tx, None, 44_100, sound_dir.into())
    }

    fn with_parts(tx: Sender<Command>, stream: Option<cpal::Stream>, rate: u32, sound_dir: PathBuf) -> Self {
        Self {
            tx,
            _stream: stream,
            sample_rate: rate,
            sound_dir,
            cache: HashMap::new(),
            warned: HashSet::new(),
            next_id: 1,
            music: None,
            music_name: None,
            listener: (Vec3::ZERO, 0.0, 0.0),
            master: 1.0,
            groups: HashMap::new(),
            default_distance: (1.0, 30.0),
            default_rolloff: 0.7,
        }
    }

    fn open(sound_dir: PathBuf) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| Error::Audio("no output device found".into()))?;
        let supported = device.default_output_config().map_err(|e| Error::Audio(e.to_string()))?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let rate = config.sample_rate;
        let (tx, rx) = channel();
        let mixer = Mixer::new(rx, rate);
        let stream = match format {
            SampleFormat::F32 => build_stream::<f32>(&device, config, mixer),
            SampleFormat::F64 => build_stream::<f64>(&device, config, mixer),
            SampleFormat::I16 => build_stream::<i16>(&device, config, mixer),
            SampleFormat::I32 => build_stream::<i32>(&device, config, mixer),
            SampleFormat::U16 => build_stream::<u16>(&device, config, mixer),
            SampleFormat::U8 => build_stream::<u8>(&device, config, mixer),
            SampleFormat::I8 => build_stream::<i8>(&device, config, mixer),
            other => return Err(Error::Audio(format!("unsupported sample format {other}"))),
        }?;
        stream.play().map_err(|e| Error::Audio(e.to_string()))?;
        Ok(Self::with_parts(tx, Some(stream), rate, sound_dir))
    }

    /// Sample rate of the output device in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Whether an output device is open (`false` means sounds are silently ignored).
    pub fn has_output(&self) -> bool {
        self._stream.is_some()
    }

    /// Changes the directory searched for sounds. See the [module docs](self).
    pub fn set_sound_dir(&mut self, dir: impl Into<PathBuf>) {
        self.sound_dir = dir.into();
    }

    // ----------------------------------------------------------------- loading

    fn resolve(&self, name: &str) -> Option<PathBuf> {
        let given = Path::new(name);
        let mut bases = vec![given.to_path_buf(), self.sound_dir.join(given)];
        // A macOS app bundle ships its files in `Contents/Resources`.
        if given.is_relative()
            && let Some(resources) = crate::bundle::resources()
        {
            bases.extend([resources.join(&bases[0]), resources.join(&bases[1])]);
        }
        for base in &bases {
            if base.is_file() {
                return Some(base.clone());
            }
            if base.extension().is_none() {
                for ext in EXTENSIONS {
                    let candidate = base.with_extension(ext);
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
        None
    }

    fn slot(&mut self, name: &str) -> Result<Arc<SoundSlot>> {
        if let Some(slot) = self.cache.get(name) {
            return Ok(slot.clone());
        }
        let path = self
            .resolve(name)
            .ok_or_else(|| Error::Io(std::io::Error::new(std::io::ErrorKind::NotFound, format!("sound \"{name}\" not found"))))?;
        let slot = Arc::new(SoundSlot::ready(SoundData::load(path)?));
        self.cache.insert(name.to_string(), slot.clone());
        Ok(slot)
    }

    fn slot_or_warn(&mut self, name: &str) -> Option<Arc<SoundSlot>> {
        match self.slot(name) {
            Ok(slot) => Some(slot),
            Err(e) => {
                if self.warned.insert(name.to_string()) {
                    eprintln!("audio-code: {e}");
                }
                None
            }
        }
    }

    /// Loads a sound into the cache now, so the first play has no delay.
    pub fn preload(&mut self, name: &str) -> Result<()> {
        self.slot(name).map(|_| ())
    }

    /// Loads every sound file found in `dir` (not recursive). Names are the file paths
    /// relative to the sound directory without extension, e.g. `"step"`.
    /// Returns how many files were loaded.
    pub fn preload_dir(&mut self, dir: impl AsRef<Path>) -> Result<usize> {
        let mut root = self.sound_dir.clone();
        if !root.join(dir.as_ref()).is_dir()
            && root.is_relative()
            && let Some(resources) = crate::bundle::resources()
        {
            root = resources.join(root);
        }
        let dir = root.join(dir.as_ref());
        let mut count = 0;
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            let is_sound = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()));
            if !is_sound {
                continue;
            }
            let rel = path.strip_prefix(&root).unwrap_or(&path).with_extension("");
            let name = rel.to_string_lossy().replace('\\', "/");
            if let std::collections::hash_map::Entry::Vacant(entry) = self.cache.entry(name) {
                entry.insert(Arc::new(SoundSlot::ready(SoundData::load(&path)?)));
                count += 1;
            }
        }
        Ok(count)
    }

    /// Registers sound data generated in code (or a [`Synth`]) under a name, so it can be
    /// played like a file. Registering the same name again replaces the sound.
    ///
    /// ```no_run
    /// # use audio_code::prelude::*;
    /// # fn f(ctx: &mut Context) {
    /// ctx.audio.register("beep", SoundData::tone(880.0, 0.1, 0.5));
    /// ctx.audio.play("beep");
    /// # }
    /// ```
    pub fn register(&mut self, name: impl Into<String>, data: impl Into<SoundData>) {
        self.cache.insert(name.into(), Arc::new(SoundSlot::ready(data.into())));
    }

    /// Whether a sound with this name is loaded or registered.
    pub fn is_loaded(&self, name: &str) -> bool {
        self.cache.contains_key(name)
    }

    /// Removes a sound from the cache to free memory. Playing sounds are not affected.
    pub fn unload(&mut self, name: &str) {
        self.cache.remove(name);
    }

    /// Length of a sound in seconds (loads it if needed).
    pub fn duration(&mut self, name: &str) -> Option<f32> {
        self.slot_or_warn(name).and_then(|s| s.get().map(SoundData::duration))
    }

    // ----------------------------------------------------------------- playing

    /// Starts building a sound with full control over its settings. Finish with
    /// [`SoundBuilder::play`].
    pub fn sound(&mut self, name: &str) -> SoundBuilder<'_> {
        SoundBuilder::new(self, name)
    }

    /// Plays a sound once, centered. Returns a handle you may keep or ignore.
    pub fn play(&mut self, name: &str) -> Sound {
        self.sound(name).play()
    }

    /// Plays a sound once at a position in the world.
    pub fn play_at(&mut self, name: &str, position: Vec3) -> Sound {
        self.sound(name).at(position).play()
    }

    /// Plays a sound in an endless loop. Stop it with [`Sound::stop`].
    pub fn play_looped(&mut self, name: &str) -> Sound {
        self.sound(name).looped().play()
    }

    /// Plays a looping sound at a position in the world.
    pub fn play_looped_at(&mut self, name: &str, position: Vec3) -> Sound {
        self.sound(name).looped().at(position).play()
    }

    /// Plays one of several sounds picked at random, e.g. footstep variations.
    pub fn play_random(&mut self, names: &[&str]) -> Sound {
        match crate::math::random::pick(names) {
            Some(name) => self.play(name),
            None => Sound::none(),
        }
    }

    /// Plays one of several sounds picked at random at a position.
    pub fn play_random_at(&mut self, names: &[&str], position: Vec3) -> Sound {
        match crate::math::random::pick(names) {
            Some(name) => self.play_at(name, position),
            None => Sound::none(),
        }
    }

    /// Plays a generated sine beep of `frequency` hertz (e.g. 880.0) for `seconds`. Great
    /// for prototyping before you have sound files.
    pub fn play_tone(&mut self, frequency: f32, seconds: f32) -> Sound {
        let name = format!("tone:{frequency}:{seconds}");
        if !self.cache.contains_key(&name) {
            self.register(name.clone(), SoundData::tone(frequency, seconds, 0.4));
        }
        self.play(&name)
    }

    fn start(&mut self, b: Settings, slot: Arc<SoundSlot>) -> Sound {
        let id = self.next_id;
        self.next_id += 1;
        let shared = Arc::new(VoiceShared::default());
        let mut voice = Voice::new(id, slot, shared.clone(), b.group);
        voice.looping = b.looping;
        voice.paused = b.paused;
        voice.pitch = b.pitch.clamp(0.01, 16.0);
        voice.pan = b.pan.clamp(-1.0, 1.0);
        voice.volume = b.volume.max(0.0);
        voice.target_volume = voice.volume;
        voice.spatial = b.position.map(|position| Spatial {
            position,
            min_distance: b.distance.0.max(0.01),
            max_distance: b.distance.1.max(b.distance.0 + 0.01),
            rolloff: b.rolloff,
        });
        if b.fade_in > 0.0 {
            voice.fade_in(b.fade_in, self.sample_rate as f32);
        }
        let _ = self.tx.send(Command::Play(Box::new(voice)));
        let sound = Sound { id, shared: Some(shared), tx: self.tx.clone() };
        if b.start_at > 0.0 {
            sound.seek(b.start_at);
        }
        sound
    }

    // ----------------------------------------------------------------- music

    /// Plays background music in a loop. The file is decoded in the background so the game
    /// does not stutter. Any previous music fades out while the new one fades in.
    /// Calling it again with the music that is already playing does nothing.
    pub fn play_music(&mut self, name: &str) -> Sound {
        self.play_music_with_fade(name, 1.0)
    }

    /// Like [`Audio::play_music`] with a custom cross fade time in seconds.
    pub fn play_music_with_fade(&mut self, name: &str, fade: f32) -> Sound {
        if let (Some(current), Some(music)) = (&self.music_name, &self.music)
            && current == name
            && music.is_playing()
        {
            return music.clone();
        }
        self.stop_music(fade);

        let slot = match self.cache.get(name) {
            Some(slot) => slot.clone(),
            None => {
                let Some(path) = self.resolve(name) else {
                    if self.warned.insert(name.to_string()) {
                        eprintln!("audio-code: music \"{name}\" not found");
                    }
                    return Sound::none();
                };
                let slot = Arc::new(SoundSlot::default());
                let loader = slot.clone();
                std::thread::spawn(move || match SoundData::load(&path) {
                    Ok(data) => loader.fill(data),
                    Err(e) => {
                        eprintln!("audio-code: {e}");
                        loader.fail();
                    }
                });
                self.cache.insert(name.to_string(), slot.clone());
                slot
            }
        };
        let settings = Settings { looping: true, group: Group::Music, fade_in: fade, ..Settings::new(self) };
        let sound = self.start(settings, slot);
        self.music = Some(sound.clone());
        self.music_name = Some(name.to_string());
        sound
    }

    /// Fades out the current music over `fade` seconds.
    pub fn stop_music(&mut self, fade: f32) {
        if let Some(music) = self.music.take() {
            music.fade_out(fade);
        }
        self.music_name = None;
    }

    /// Handle of the current music, if any.
    pub fn music(&self) -> Option<&Sound> {
        self.music.as_ref()
    }

    /// Name of the current music, as passed to [`Audio::play_music`].
    pub fn music_name(&self) -> Option<&str> {
        self.music_name.as_deref()
    }

    // ----------------------------------------------------------------- listener & volume

    /// Places the listener (the player's ears) in the world. `yaw` is the compass direction
    /// the player faces (0 = north, 90 = east). Call this whenever the player moves or turns.
    /// The listener looks level with the horizon; use [`Audio::set_listener_3d`] to also
    /// look up or down.
    pub fn set_listener(&mut self, position: Vec3, yaw: f32) {
        self.set_listener_3d(position, yaw, 0.0);
    }

    /// Like [`Audio::set_listener`], plus `pitch`: how far the player looks up (positive,
    /// 90 = straight up) or down (negative, -90 = straight down), in degrees. For flying,
    /// swimming or space games.
    pub fn set_listener_3d(&mut self, position: Vec3, yaw: f32, pitch: f32) {
        let pitch = pitch.clamp(-90.0, 90.0);
        self.listener = (position, yaw, pitch);
        let _ = self.tx.send(Command::Listener { position, yaw, pitch });
    }

    /// Current listener position.
    pub fn listener_position(&self) -> Vec3 {
        self.listener.0
    }

    /// Current listener facing in compass degrees.
    pub fn listener_yaw(&self) -> f32 {
        self.listener.1
    }

    /// Current listener pitch in degrees (0 = level, 90 = straight up).
    pub fn listener_pitch(&self) -> f32 {
        self.listener.2
    }

    /// Default audible range for positioned sounds in meters: full volume up to `min`,
    /// silent from `max` on. The default is 1.0 and 30.0.
    pub fn set_default_distance(&mut self, min: f32, max: f32) {
        self.default_distance = (min, max);
    }

    /// Default rolloff for positioned sounds: how fast the volume drops between `min` and
    /// `max` distance. 1.0 is physically accurate (twice the distance, half the volume),
    /// lower values keep distant sounds louder, 0.0 keeps full volume until shortly before
    /// `max`. Default is 0.7, which is at 32 % volume at 5 m and 18 % at 10 m.
    pub fn set_default_rolloff(&mut self, rolloff: f32) {
        self.default_rolloff = rolloff.max(0.0);
    }

    /// Sets the overall volume as a linear factor: 1.0 = unchanged (default), 0.5 = half,
    /// 0.0 = silent. Higher values amplify and may distort.
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master = volume.max(0.0);
        let _ = self.tx.send(Command::MasterVolume(self.master));
    }

    /// Overall volume.
    pub fn master_volume(&self) -> f32 {
        self.master
    }

    /// Sets the volume of one group (sound effects, music or UI) as a linear factor,
    /// 1.0 = unchanged (default). It is multiplied with each sound's own volume and the
    /// master volume.
    pub fn set_group_volume(&mut self, group: Group, volume: f32) {
        self.groups.insert(group, volume.max(0.0));
        let _ = self.tx.send(Command::GroupVolume(group, volume));
    }

    /// Volume of one group.
    pub fn group_volume(&self, group: Group) -> f32 {
        self.groups.get(&group).copied().unwrap_or(1.0)
    }

    /// Shortcut for `set_group_volume(Group::Music, volume)`.
    pub fn set_music_volume(&mut self, volume: f32) {
        self.set_group_volume(Group::Music, volume);
    }

    /// Shortcut for `set_group_volume(Group::Sound, volume)`.
    pub fn set_sound_volume(&mut self, volume: f32) {
        self.set_group_volume(Group::Sound, volume);
    }

    /// Stops every sound immediately (including music).
    pub fn stop_all(&mut self) {
        self.music = None;
        self.music_name = None;
        let _ = self.tx.send(Command::StopAll { group: None, fade: 0.0 });
    }

    /// Fades out all sounds of a group over `fade` seconds and stops them.
    pub fn stop_group(&mut self, group: Group, fade: f32) {
        if group == Group::Music {
            self.music = None;
            self.music_name = None;
        }
        let _ = self.tx.send(Command::StopAll { group: Some(group), fade });
    }
}

fn build_stream<T>(device: &cpal::Device, config: cpal::StreamConfig, mut mixer: Mixer) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut scratch: Vec<f32> = Vec::new();
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let frames = data.len() / channels.max(1);
                scratch.resize(frames * 2, 0.0);
                mixer.render(&mut scratch);
                for (frame, stereo) in data.chunks_mut(channels).zip(scratch.chunks(2)) {
                    match frame.len() {
                        1 => frame[0] = T::from_sample((stereo[0] + stereo[1]) * 0.5),
                        _ => {
                            frame[0] = T::from_sample(stereo[0]);
                            frame[1] = T::from_sample(stereo[1]);
                            for extra in &mut frame[2..] {
                                *extra = T::from_sample(0.0f32);
                            }
                        }
                    }
                }
            },
            |err| eprintln!("audio-code: audio stream error: {err}"),
            None,
        )
        .map_err(|e| Error::Audio(e.to_string()))
}

/// Configures a sound before playing it. Created by [`Audio::sound`].
#[must_use = "call .play() to actually play the sound"]
pub struct SoundBuilder<'a> {
    audio: &'a mut Audio,
    name: String,
    settings: Settings,
}

#[derive(Debug, Clone)]
struct Settings {
    volume: f32,
    pitch: f32,
    pan: f32,
    looping: bool,
    paused: bool,
    position: Option<Vec3>,
    distance: (f32, f32),
    rolloff: f32,
    group: Group,
    fade_in: f32,
    start_at: f32,
}

impl Settings {
    fn new(audio: &Audio) -> Self {
        Self {
            volume: 1.0,
            pitch: 1.0,
            pan: 0.0,
            looping: false,
            paused: false,
            position: None,
            distance: audio.default_distance,
            rolloff: audio.default_rolloff,
            group: Group::Sound,
            fade_in: 0.0,
            start_at: 0.0,
        }
    }
}

impl<'a> SoundBuilder<'a> {
    fn new(audio: &'a mut Audio, name: &str) -> Self {
        let settings = Settings::new(audio);
        Self { audio, name: name.to_string(), settings }
    }

    /// Linear volume factor: 1.0 = as recorded (default), 0.5 = half (about -6 dB),
    /// 0.0 = silent. Values above 1.0 amplify and may distort.
    pub fn volume(mut self, volume: f32) -> Self {
        self.settings.volume = volume;
        self
    }

    /// Volume in decibels: 0 = original, -6 = about half, -20 = a tenth, +6 = about double.
    pub fn volume_db(mut self, db: f32) -> Self {
        self.settings.volume = crate::math::db_to_linear(db);
        self
    }

    /// Playback speed, which changes pitch and length together: 1.0 = original (default),
    /// 2.0 = one octave up and twice as fast, 0.5 = one octave down and twice as long.
    pub fn pitch(mut self, pitch: f32) -> Self {
        self.settings.pitch = pitch;
        self
    }

    /// Random pitch between `min` and `max` – makes repeated sounds less monotonous.
    /// `0.9, 1.1` is a good range for footsteps and hits.
    pub fn random_pitch(mut self, min: f32, max: f32) -> Self {
        self.settings.pitch = crate::math::random::range(min, max);
        self
    }

    /// Stereo position for non-positioned sounds: -1.0 left, 0.0 center (default), 1.0
    /// right. Ignored once the sound has a position ([`SoundBuilder::at`]).
    pub fn pan(mut self, pan: f32) -> Self {
        self.settings.pan = pan;
        self
    }

    /// Loop endlessly.
    pub fn looped(mut self) -> Self {
        self.settings.looping = true;
        self
    }

    /// Place the sound in the world. It is panned and attenuated relative to the listener.
    pub fn at(mut self, position: Vec3) -> Self {
        self.settings.position = Some(position);
        self
    }

    /// Audible range: full volume up to `min` meters, silent from `max` meters. Default
    /// 1.0 and 30.0 (see [`Audio::set_default_distance`]).
    pub fn distance(mut self, min: f32, max: f32) -> Self {
        self.settings.distance = (min, max);
        self
    }

    /// How quickly volume drops with distance (see [`Audio::set_default_rolloff`]).
    pub fn rolloff(mut self, rolloff: f32) -> Self {
        self.settings.rolloff = rolloff.max(0.0);
        self
    }

    /// Volume group (default [`Group::Sound`]).
    pub fn group(mut self, group: Group) -> Self {
        self.settings.group = group;
        self
    }

    /// Fades in from silence to the volume over the given seconds.
    pub fn fade_in(mut self, seconds: f32) -> Self {
        self.settings.fade_in = seconds;
        self
    }

    /// Start playback this many seconds into the sound.
    pub fn start_at(mut self, seconds: f32) -> Self {
        self.settings.start_at = seconds;
        self
    }

    /// Create the sound paused; call [`Sound::resume`] to start it.
    pub fn paused(mut self) -> Self {
        self.settings.paused = true;
        self
    }

    /// Plays the sound and returns its handle.
    pub fn play(self) -> Sound {
        let SoundBuilder { audio, name, settings } = self;
        match audio.slot_or_warn(&name) {
            Some(slot) => audio.start(settings, slot),
            None => Sound::none(),
        }
    }
}

/// Handle to a playing sound. Cheap to clone. Dropping it does **not** stop the sound.
///
/// All methods are safe to call after the sound finished; they simply do nothing.
#[derive(Clone)]
pub struct Sound {
    id: u64,
    shared: Option<Arc<VoiceShared>>,
    tx: Sender<Command>,
}

impl std::fmt::Debug for Sound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sound").field("id", &self.id).field("playing", &self.is_playing()).finish()
    }
}

impl Default for Sound {
    fn default() -> Self {
        Self::none()
    }
}

impl Sound {
    /// A handle that refers to no sound. Useful as a placeholder in structs.
    pub fn none() -> Self {
        let (tx, _) = channel();
        Self { id: 0, shared: None, tx }
    }

    fn send(&self, change: Change) {
        if self.shared.is_some() {
            let _ = self.tx.send(Command::Change(self.id, change));
        }
    }

    /// Whether the sound is still playing (or paused, or loading).
    pub fn is_playing(&self) -> bool {
        self.shared.as_ref().is_some_and(|s| !s.finished.load(Ordering::Acquire))
    }

    /// Whether the sound has ended or was stopped.
    pub fn is_finished(&self) -> bool {
        !self.is_playing()
    }

    /// Current playback position in seconds.
    pub fn position(&self) -> f32 {
        self.shared.as_ref().map_or(0.0, |s| f64::from_bits(s.position.load(Ordering::Relaxed)) as f32)
    }

    /// Stops immediately.
    pub fn stop(&self) {
        self.send(Change::Stop { fade: 0.0 });
    }

    /// Fades to silence over `seconds`, then stops. Good for ending loops without a click.
    pub fn fade_out(&self, seconds: f32) {
        self.send(Change::Stop { fade: seconds });
    }

    /// Sets the linear volume immediately (1.0 = original, 0.0 = silent).
    pub fn set_volume(&self, volume: f32) {
        self.send(Change::Volume { target: volume, fade: 0.0 });
    }

    /// Fades the linear volume to `volume` over `seconds`. The sound keeps playing, also
    /// at 0.0.
    pub fn fade_to(&self, volume: f32, seconds: f32) {
        self.send(Change::Volume { target: volume, fade: seconds });
    }

    /// Sets playback speed and pitch (1.0 = original, 2.0 = one octave up and twice as
    /// fast). Can be changed every frame, e.g. for an engine sound.
    pub fn set_pitch(&self, pitch: f32) {
        self.send(Change::Pitch(pitch));
    }

    /// Sets stereo panning for non-positioned sounds (-1.0 left to 1.0 right).
    pub fn set_pan(&self, pan: f32) {
        self.send(Change::Pan(pan));
    }

    /// Moves a positioned sound (created with `at` / `play_at`).
    pub fn set_position(&self, position: Vec3) {
        self.send(Change::Position(position));
    }

    /// Changes the audible range of a positioned sound.
    pub fn set_distance(&self, min: f32, max: f32) {
        self.send(Change::Distance { min, max });
    }

    /// Pauses playback.
    pub fn pause(&self) {
        self.send(Change::Paused(true));
    }

    /// Continues after [`Sound::pause`].
    pub fn resume(&self) {
        self.send(Change::Paused(false));
    }

    /// Turns looping on or off while playing.
    pub fn set_looping(&self, looping: bool) {
        self.send(Change::Looping(looping));
    }

    /// Jumps to a position in seconds.
    pub fn seek(&self, seconds: f32) {
        self.send(Change::Seek(seconds));
    }
}
