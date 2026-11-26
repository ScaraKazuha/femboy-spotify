//! Audio playback through librespot.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use librespot::{
    core::{session::Session, SpotifyUri},
    playback::{
        audio_backend,
        config::{AudioFormat, PlayerConfig},
        mixer::{self, Mixer, MixerConfig},
        player::{Player as LsPlayer, PlayerEvent as LsEvent},
    },
};
use tokio::sync::mpsc;

/// Simplified player events for the UI (only for the current track).
#[derive(Debug, Clone, Copy)]
pub enum Event {
    Loading,
    Playing(u32),
    Paused(u32),
    Position(u32),
    EndOfTrack,
    Unavailable,
    Stopped,
}

pub struct Player {
    inner: Arc<LsPlayer>,
    mixer: Arc<dyn Mixer>,
}

impl Player {
    pub fn new(session: Session, volume_percent: u16) -> Result<(Self, mpsc::UnboundedReceiver<Event>)> {
        let mixer_fn = mixer::find(None).ok_or_else(|| anyhow!("no audio mixer available"))?;
        let mixer = mixer_fn(MixerConfig::default()).map_err(|e| anyhow!("mixer: {e}"))?;
        let backend =
            audio_backend::find(None).ok_or_else(|| anyhow!("no audio backend available"))?;

        let inner = LsPlayer::new(
            PlayerConfig::default(),
            session,
            mixer.get_soft_volume(),
            move || backend(None, AudioFormat::default()),
        );

        let mut ls_events = inner.get_player_event_channel();
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            // Ignore stray events from tracks we already moved away from.
            let mut current: Option<u64> = None;
            while let Some(ev) = ls_events.recv().await {
                let (id, mapped) = match ev {
                    LsEvent::PlayRequestIdChanged { play_request_id } => {
                        current = Some(play_request_id);
                        continue;
                    }
                    LsEvent::Loading { play_request_id, .. } => (play_request_id, Event::Loading),
                    LsEvent::Playing { play_request_id, position_ms, .. } => {
                        (play_request_id, Event::Playing(position_ms))
                    }
                    LsEvent::Paused { play_request_id, position_ms, .. } => {
                        (play_request_id, Event::Paused(position_ms))
                    }
                    LsEvent::Seeked { play_request_id, position_ms, .. }
                    | LsEvent::PositionCorrection { play_request_id, position_ms, .. } => {
                        (play_request_id, Event::Position(position_ms))
                    }
                    LsEvent::EndOfTrack { play_request_id, .. } => {
                        (play_request_id, Event::EndOfTrack)
                    }
                    LsEvent::Unavailable { play_request_id, .. } => {
                        (play_request_id, Event::Unavailable)
                    }
                    LsEvent::Stopped { play_request_id, .. } => (play_request_id, Event::Stopped),
                    _ => continue,
                };
                if current.is_some_and(|c| c != id) {
                    continue;
                }
                if tx.send(mapped).is_err() {
                    break;
                }
            }
        });

        let player = Self { inner, mixer };
        player.set_volume(volume_percent);
        Ok((player, rx))
    }

    pub fn load(&self, uri: &str) -> Result<()> {
        let uri = SpotifyUri::from_uri(uri).map_err(|e| anyhow!("bad track uri {uri}: {e}"))?;
        self.inner.load(uri, true, 0);
        Ok(())
    }

    pub fn play(&self) {
        self.inner.play();
    }

    pub fn pause(&self) {
        self.inner.pause();
    }

    pub fn seek(&self, position_ms: u32) {
        self.inner.seek(position_ms);
    }

    /// `percent` is 0..=100.
    pub fn set_volume(&self, percent: u16) {
        let v = (percent.min(100) as u32 * u16::MAX as u32 / 100) as u16;
        self.mixer.set_volume(v);
    }
}
