use rodio::{Decoder, OutputStream, Sink};
use std::io::Cursor;

// Embedded WAV tones generated at build time.
const FOCUS_COMPLETE_WAV: &[u8] = include_bytes!("../assets/sounds/focus_complete.wav");
const BREAK_COMPLETE_WAV: &[u8] = include_bytes!("../assets/sounds/break_complete.wav");

pub struct SoundPlayer {
    pub enabled: bool,
    volume: f32,
}

impl SoundPlayer {
    pub fn new(enabled: bool, volume: f32) -> Self {
        Self {
            enabled,
            // Clamp volume to [0.0, 1.0]
            volume: volume.clamp(0.0, 1.0),
        }
    }

    /// Play the focus-complete chime (warm bell, A5).
    pub fn play_focus_complete(&self) {
        self.play(FOCUS_COMPLETE_WAV);
    }

    /// Play the break-complete chime (lighter tone, C6).
    pub fn play_break_complete(&self) {
        self.play(BREAK_COMPLETE_WAV);
    }

    fn play(&self, wav_bytes: &'static [u8]) {
        if !self.enabled {
            return;
        }

        let volume = self.volume;

        // Spawn a thread so audio never blocks the TUI event loop.
        std::thread::spawn(move || {
            // IMPORTANT: `stream` must be bound to a named variable that lives
            // for the entire duration of playback. If it drops early the audio
            // device closes and playback is silently cut off.
            let stream_result = OutputStream::try_default();
            let (stream, stream_handle) = match stream_result {
                Ok(pair) => pair,
                Err(_) => return,
            };

            let sink = match Sink::try_new(&stream_handle) {
                Ok(s) => s,
                Err(_) => return,
            };

            sink.set_volume(volume);

            let cursor = Cursor::new(wav_bytes);
            let source = match Decoder::new(cursor) {
                Ok(s) => s,
                Err(_) => return,
            };

            sink.append(source);
            sink.sleep_until_end();

            // Explicitly drop stream after playback ends so the compiler
            // doesn't move it earlier.
            drop(stream);
        });
    }
}
