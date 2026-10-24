//! Procedurally synthesized sound effects (no asset files needed).

use std::num::NonZero;
use std::sync::Arc;
use std::time::Duration;

use bevy::audio::{AddAudioSource, Decodable, PlaybackMode, Sample, Source, SpatialScale, Volume};
use bevy::prelude::*;

use crate::cases::Rarity;
use crate::settings::Settings;
use crate::weapons::WeaponClass;

const RATE: u32 = 44_100;

#[derive(Asset, TypePath, Clone)]
pub struct Synth {
    samples: Arc<[f32]>,
}

pub struct SynthDecoder {
    samples: Arc<[f32]>,
    pos: usize,
}

impl Iterator for SynthDecoder {
    type Item = Sample;
    fn next(&mut self) -> Option<Sample> {
        let s = self.samples.get(self.pos).copied()?;
        self.pos += 1;
        Some(s as Sample)
    }
}

impl Source for SynthDecoder {
    fn current_span_len(&self) -> Option<usize> {
        Some(self.samples.len() - self.pos)
    }
    fn channels(&self) -> bevy::audio::ChannelCount {
        NonZero::new(1).unwrap()
    }
    fn sample_rate(&self) -> bevy::audio::SampleRate {
        NonZero::new(RATE).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(self.samples.len() as f32 / RATE as f32))
    }
}

impl Decodable for Synth {
    type Decoder = SynthDecoder;
    fn decoder(&self) -> SynthDecoder {
        SynthDecoder {
            samples: self.samples.clone(),
            pos: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sound {
    Shot(WeaponClass),
    Hit,
    Headshot,
    Kill,
    Reload,
    Empty,
    Click,
    Hover,
    Tick,
    Reveal(Rarity),
    Explosion,
    Flash,
    Smoke,
    Fire,
    Throw,
    Buy,
    Deny,
    RoundStart,
    RoundEnd,
    Footstep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Effects,
    Ui,
}

#[derive(Message, Clone, Copy)]
pub struct PlaySfx {
    pub sound: Sound,
    pub volume: f32,
    pub channel: Channel,
    /// World position for left/right panning (needs a `SpatialListener`).
    pub pos: Option<Vec3>,
}

impl PlaySfx {
    pub fn fx(sound: Sound, volume: f32) -> Self {
        Self { sound, volume, channel: Channel::Effects, pos: None }
    }
    /// A sound coming from somewhere in the world.
    pub fn fx_at(sound: Sound, volume: f32, pos: Vec3) -> Self {
        Self { pos: Some(pos), ..Self::fx(sound, volume) }
    }
    pub fn ui(sound: Sound) -> Self {
        Self { sound, volume: 1.0, channel: Channel::Ui, pos: None }
    }
}

#[derive(Resource, Default)]
pub struct SfxLibrary {
    sounds: Vec<(Sound, Handle<Synth>)>,
}

pub struct AudioFxPlugin;

impl Plugin for AudioFxPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<Synth>()
            .add_message::<PlaySfx>()
            .init_resource::<SfxLibrary>()
            .add_systems(Startup, build_library)
            .add_systems(PostUpdate, play_sfx);
    }
}

fn build_library(mut lib: ResMut<SfxLibrary>, mut assets: ResMut<Assets<Synth>>) {
    let mut all = vec![
        Sound::Hit,
        Sound::Headshot,
        Sound::Kill,
        Sound::Reload,
        Sound::Empty,
        Sound::Click,
        Sound::Hover,
        Sound::Tick,
        Sound::Explosion,
        Sound::Flash,
        Sound::Smoke,
        Sound::Fire,
        Sound::Throw,
        Sound::Buy,
        Sound::Deny,
        Sound::RoundStart,
        Sound::RoundEnd,
        Sound::Footstep,
    ];
    for c in [
        WeaponClass::Knife,
        WeaponClass::Pistol,
        WeaponClass::Smg,
        WeaponClass::Shotgun,
        WeaponClass::Rifle,
        WeaponClass::Sniper,
    ] {
        all.push(Sound::Shot(c));
    }
    for r in Rarity::ALL {
        all.push(Sound::Reveal(r));
    }
    for s in all {
        let samples = synthesize(s);
        lib.sounds.push((s, assets.add(Synth { samples: samples.into() })));
    }
}

fn play_sfx(
    mut commands: Commands,
    mut reader: MessageReader<PlaySfx>,
    lib: Res<SfxLibrary>,
    settings: Res<Settings>,
) {
    for msg in reader.read() {
        let Some((_, handle)) = lib.sounds.iter().find(|(s, _)| *s == msg.sound) else {
            continue;
        };
        let channel = match msg.channel {
            Channel::Effects => settings.effects_volume,
            Channel::Ui => settings.ui_volume,
        };
        let volume = msg.volume * channel;
        if volume <= 0.001 {
            continue;
        }
        commands.spawn((
            AudioPlayer::<Synth>(handle.clone()),
            PlaybackSettings {
                mode: PlaybackMode::Despawn,
                volume: Volume::Linear(volume),
                spatial: msg.pos.is_some(),
                // Shrink the world so rodio's 1/d^2 falloff never kicks in; callers
                // scale volume by distance themselves and we only want panning.
                spatial_scale: Some(SpatialScale::new(0.02)),
                ..PlaybackSettings::DESPAWN
            },
            Transform::from_translation(msg.pos.unwrap_or_default()),
        ));
    }
}

// ---------------------------------------------------------------- synthesis

struct Gen {
    out: Vec<f32>,
    rng: fastrand::Rng,
}

impl Gen {
    fn new(seconds: f32) -> Self {
        Self {
            out: vec![0.0; (seconds * RATE as f32) as usize],
            rng: fastrand::Rng::with_seed(7),
        }
    }

    /// Low-passed noise with exponential decay.
    fn noise(&mut self, start: f32, len: f32, amp: f32, decay: f32, lowpass: f32) {
        let s0 = (start * RATE as f32) as usize;
        let n = (len * RATE as f32) as usize;
        let mut lp = 0.0;
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let white = self.rng.f32() * 2.0 - 1.0;
            lp += (white - lp) * lowpass;
            if let Some(o) = self.out.get_mut(s0 + i) {
                *o += lp * amp * (-t * decay).exp();
            }
        }
    }

    /// Sine with optional pitch sweep and exponential decay.
    fn tone(&mut self, start: f32, len: f32, f0: f32, f1: f32, amp: f32, decay: f32) {
        let s0 = (start * RATE as f32) as usize;
        let n = (len * RATE as f32) as usize;
        let mut phase = 0.0f32;
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let f = f0 + (f1 - f0) * (i as f32 / n as f32);
            phase += f / RATE as f32 * std::f32::consts::TAU;
            let attack = (t * 400.0).min(1.0);
            if let Some(o) = self.out.get_mut(s0 + i) {
                *o += phase.sin() * amp * attack * (-t * decay).exp();
            }
        }
    }

    fn finish(mut self) -> Vec<f32> {
        let peak = self.out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        if peak > 0.95 {
            let k = 0.95 / peak;
            self.out.iter_mut().for_each(|s| *s *= k);
        }
        self.out
    }
}

fn synthesize(sound: Sound) -> Vec<f32> {
    match sound {
        Sound::Shot(class) => {
            let (len, decay, lp, thump, amp) = match class {
                WeaponClass::Knife => (0.18, 18.0, 0.25, 0.0, 0.35),
                WeaponClass::Pistol => (0.35, 16.0, 0.45, 140.0, 0.8),
                WeaponClass::Smg => (0.25, 22.0, 0.5, 160.0, 0.7),
                WeaponClass::Shotgun => (0.6, 8.0, 0.3, 90.0, 1.0),
                WeaponClass::Rifle => (0.45, 12.0, 0.4, 110.0, 0.9),
                WeaponClass::Sniper => (0.9, 6.0, 0.35, 70.0, 1.0),
            };
            let mut g = Gen::new(len + 0.05);
            if class == WeaponClass::Knife {
                g.noise(0.0, len, amp, decay, 0.6);
                g.tone(0.0, len, 900.0, 300.0, 0.1, decay);
            } else {
                g.noise(0.0, len, amp, decay, lp);
                g.noise(0.0, 0.02, amp * 0.8, 60.0, 0.9);
                g.tone(0.0, len, thump, thump * 0.5, amp * 0.8, decay * 0.8);
            }
            g.finish()
        }
        Sound::Hit => {
            let mut g = Gen::new(0.12);
            g.tone(0.0, 0.1, 1900.0, 1500.0, 0.5, 40.0);
            g.noise(0.0, 0.04, 0.2, 60.0, 0.5);
            g.finish()
        }
        Sound::Headshot => {
            let mut g = Gen::new(0.35);
            g.tone(0.0, 0.3, 2600.0, 2550.0, 0.5, 12.0);
            g.tone(0.0, 0.3, 3900.0, 3850.0, 0.3, 14.0);
            g.tone(0.0, 0.3, 5200.0, 5150.0, 0.15, 18.0);
            g.finish()
        }
        Sound::Kill => {
            let mut g = Gen::new(0.4);
            g.tone(0.0, 0.15, 880.0, 880.0, 0.4, 10.0);
            g.tone(0.1, 0.25, 1320.0, 1320.0, 0.4, 8.0);
            g.finish()
        }
        Sound::Reload => {
            let mut g = Gen::new(0.5);
            g.noise(0.0, 0.05, 0.4, 50.0, 0.7);
            g.tone(0.0, 0.05, 600.0, 400.0, 0.2, 40.0);
            g.noise(0.3, 0.06, 0.5, 50.0, 0.8);
            g.tone(0.3, 0.06, 900.0, 700.0, 0.25, 40.0);
            g.finish()
        }
        Sound::Empty => {
            let mut g = Gen::new(0.08);
            g.noise(0.0, 0.03, 0.4, 80.0, 0.8);
            g.tone(0.0, 0.04, 1200.0, 1100.0, 0.2, 60.0);
            g.finish()
        }
        Sound::Click => {
            let mut g = Gen::new(0.06);
            g.tone(0.0, 0.05, 1100.0, 900.0, 0.4, 60.0);
            g.finish()
        }
        Sound::Hover => {
            let mut g = Gen::new(0.04);
            g.tone(0.0, 0.03, 2200.0, 2000.0, 0.12, 90.0);
            g.finish()
        }
        Sound::Tick => {
            let mut g = Gen::new(0.04);
            g.tone(0.0, 0.03, 3200.0, 3000.0, 0.3, 120.0);
            g.noise(0.0, 0.01, 0.2, 200.0, 0.9);
            g.finish()
        }
        Sound::Reveal(r) => {
            let notes: &[f32] = match r {
                Rarity::MilSpec => &[523.0, 659.0],
                Rarity::Restricted => &[523.0, 659.0, 784.0],
                Rarity::Classified => &[587.0, 740.0, 880.0, 1175.0],
                Rarity::Covert => &[659.0, 831.0, 988.0, 1319.0],
                Rarity::RareSpecial => &[523.0, 659.0, 784.0, 1047.0, 1319.0, 1568.0],
            };
            let mut g = Gen::new(0.25 + notes.len() as f32 * 0.09 + 0.8);
            for (i, f) in notes.iter().enumerate() {
                g.tone(i as f32 * 0.09, 1.0, *f, *f, 0.3, 3.5);
            }
            g.finish()
        }
        Sound::Explosion => {
            let mut g = Gen::new(1.6);
            g.noise(0.0, 1.6, 1.0, 3.0, 0.08);
            g.noise(0.0, 0.1, 0.8, 30.0, 0.6);
            g.tone(0.0, 1.0, 60.0, 30.0, 0.8, 4.0);
            g.finish()
        }
        Sound::Flash => {
            let mut g = Gen::new(1.2);
            g.noise(0.0, 0.3, 1.0, 20.0, 0.9);
            g.tone(0.0, 1.2, 3800.0, 3700.0, 0.15, 2.5);
            g.finish()
        }
        Sound::Smoke => {
            let mut g = Gen::new(2.0);
            g.noise(0.0, 2.0, 0.5, 1.5, 0.7);
            g.finish()
        }
        Sound::Fire => {
            let mut g = Gen::new(1.0);
            g.noise(0.0, 1.0, 0.6, 2.0, 0.15);
            for i in 0..12 {
                let t = i as f32 * 0.08 + g.rng.f32() * 0.04;
                g.noise(t, 0.02, 0.5, 100.0, 0.9);
            }
            g.finish()
        }
        Sound::Throw => {
            let mut g = Gen::new(0.25);
            g.noise(0.0, 0.25, 0.4, 12.0, 0.2);
            g.finish()
        }
        Sound::Buy => {
            let mut g = Gen::new(0.3);
            g.tone(0.0, 0.1, 1568.0, 1568.0, 0.3, 20.0);
            g.tone(0.08, 0.2, 2093.0, 2093.0, 0.3, 12.0);
            g.finish()
        }
        Sound::Deny => {
            let mut g = Gen::new(0.25);
            g.tone(0.0, 0.22, 220.0, 180.0, 0.4, 8.0);
            g.finish()
        }
        Sound::RoundStart => {
            let mut g = Gen::new(0.7);
            g.tone(0.0, 0.3, 440.0, 440.0, 0.3, 5.0);
            g.tone(0.25, 0.45, 660.0, 660.0, 0.3, 5.0);
            g.finish()
        }
        Sound::RoundEnd => {
            let mut g = Gen::new(1.0);
            g.tone(0.0, 0.4, 660.0, 660.0, 0.3, 4.0);
            g.tone(0.3, 0.7, 440.0, 440.0, 0.3, 3.0);
            g.finish()
        }
        Sound::Footstep => {
            let mut g = Gen::new(0.12);
            g.noise(0.0, 0.1, 0.4, 35.0, 0.12);
            g.finish()
        }
    }
}
