//! The 00:00 alarm.
//!
//! When a phase ends, a short bell chime (embedded in the binary, see
//! `assets/chime.wav`) is played through whichever of `paplay`, `pw-play`,
//! `aplay` (Linux) or `afplay` (macOS) is available, repeating until the user
//! stops the alarm. If no player works, the terminal bell is used instead so
//! the alarm is never silent.
//!
//! The chime is played by *one* child process at a time, and that process is
//! owned by the [`Alarm`]: stopping the alarm (or dropping it, e.g. on quit)
//! kills the player immediately, so the sound ends the instant `A` is pressed.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Time between the start of one chime and the start of the next. The chime is
/// 2.7 s long, which leaves a short pause between repeats.
const PULSE_INTERVAL: Duration = Duration::from_secs(3);
/// How long the ring flashes on / off.
const FLASH_INTERVAL: Duration = Duration::from_millis(500);

/// The chime, baked into the binary so nothing needs to be installed.
const CHIME_WAV: &[u8] = include_bytes!("../assets/chime.wav");

/// Command-line players tried in order. Supports Linux (PulseAudio/PipeWire/ALSA)
/// and macOS native audio (`afplay`).
const PLAYERS: [&str; 4] = ["paplay", "pw-play", "aplay", "afplay"];

pub struct Alarm {
    enabled: bool,
    started: Option<Instant>,
    next_pulse: Instant,
    /// The chime written to disk (players need a file path).
    chime: Option<PathBuf>,
    /// The chime that is playing right now, if any.
    player: Option<Child>,
    /// Index into `PLAYERS` of `player`.
    player_index: usize,
    /// Players that are missing or exited with an error; they are skipped.
    player_failed: [bool; 4],
}

impl Alarm {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            started: None,
            next_pulse: Instant::now(),
            chime: if enabled { install_chime() } else { None },
            player: None,
            player_index: 0,
            player_failed: [false; 4],
        }
    }

    /// Begin ringing at `now` (the first chime plays on the next `pulse`).
    pub fn start(&mut self, now: Instant) {
        self.started = Some(now);
        self.next_pulse = now;
    }

    /// Stop ringing and cut off any chime that is still playing.
    pub fn stop(&mut self) {
        self.started = None;
        self.silence();
    }

    /// Play the next chime if one is due. Returns true when one was started.
    /// Chimes never overlap: while the previous one is still playing, nothing
    /// new is started.
    pub fn pulse(&mut self, now: Instant) -> bool {
        if self.started.is_none() {
            return false;
        }
        self.reap();
        if now < self.next_pulse || self.player.is_some() {
            return false;
        }
        self.next_pulse = now + PULSE_INTERVAL;
        if self.enabled && !self.play_chime() {
            terminal_bell();
        }
        true
    }

    /// Alternates every half second while ringing; drives the visual flash.
    pub fn flash_on(&self, now: Instant) -> bool {
        match self.started {
            Some(start) => {
                (now.saturating_duration_since(start).as_millis() / FLASH_INTERVAL.as_millis())
                    % 2
                    == 0
            }
            None => false,
        }
    }

    /// Kill the playing chime, if any, and reap the process.
    fn silence(&mut self) {
        if let Some(mut child) = self.player.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Notice a finished player. One that exited with an error (no sound
    /// server, say) is marked broken so the next chime tries another player.
    fn reap(&mut self) {
        let finished = match self.player.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(status)) => Some(status.success()),
                Ok(None) => None,
                Err(_) => Some(false),
            },
            None => None,
        };
        if let Some(ok) = finished {
            self.player = None;
            if !ok {
                self.player_failed[self.player_index] = true;
            }
        }
    }

    /// Start the chime with the first working player. False if none could start.
    fn play_chime(&mut self) -> bool {
        let Some(file) = self.chime.clone() else {
            return false;
        };
        for index in 0..PLAYERS.len() {
            if self.player_failed[index] {
                continue;
            }
            match spawn_player(index, &file) {
                Some(child) => {
                    self.player = Some(child);
                    self.player_index = index;
                    return true;
                }
                None => self.player_failed[index] = true,
            }
        }
        false
    }
}

impl Drop for Alarm {
    fn drop(&mut self) {
        // Quitting while the alarm rings must not leave the sound playing.
        self.silence();
    }
}

fn spawn_player(index: usize, file: &Path) -> Option<Child> {
    let mut cmd = Command::new(PLAYERS[index]);
    if PLAYERS[index] == "aplay" {
        cmd.arg("-q");
    }
    cmd.arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

/// Fallback: ring the terminal bell (works in raw mode / alternate screen and
/// draws nothing).
fn terminal_bell() {
    let mut out = std::io::stdout();
    let _ = out.write_all(b"\x07");
    let _ = out.flush();
}

/// Write the embedded chime to the cache directory (if it is not there yet)
/// and return its path.
fn install_chime() -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "devchron")?;
    let dir = dirs.cache_dir();
    std::fs::create_dir_all(dir).ok()?;
    let path = dir.join("alarm-chime.wav");
    let up_to_date = std::fs::read(&path).map(|d| d == CHIME_WAV).unwrap_or(false);
    if !up_to_date {
        std::fs::write(&path, CHIME_WAV).ok()?;
    }
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silent_until_started_and_after_stop() {
        let mut a = Alarm::new(false);
        let t = Instant::now();
        assert!(!a.pulse(t));
        a.start(t);
        assert!(a.pulse(t));
        a.stop();
        assert!(!a.pulse(t + Duration::from_secs(5)));
        assert!(!a.flash_on(t));
    }

    #[test]
    fn chimes_repeat_every_three_seconds_and_flash_alternates() {
        let mut a = Alarm::new(false);
        let t = Instant::now();
        a.start(t);
        assert!(a.pulse(t));
        assert!(!a.pulse(t + Duration::from_millis(2_900)));
        assert!(a.pulse(t + Duration::from_millis(3_000)));
        assert!(a.flash_on(t));
        assert!(!a.flash_on(t + Duration::from_millis(500)));
        assert!(a.flash_on(t + Duration::from_millis(1_000)));
    }

    #[test]
    fn the_embedded_chime_is_a_wav_file() {
        assert_eq!(&CHIME_WAV[0..4], b"RIFF");
        assert_eq!(&CHIME_WAV[8..12], b"WAVE");
    }

    #[cfg(unix)]
    #[test]
    fn stopping_kills_the_chime_that_is_still_playing() {
        let mut a = Alarm::new(false);
        let t = Instant::now();
        a.start(t);
        let child = Command::new("sleep").arg("30").spawn().expect("sleep");
        let pid = child.id();
        a.player = Some(child);

        // A chime is still playing, so no second one may start on top of it.
        assert!(!a.pulse(t + Duration::from_secs(10)));

        a.stop();
        assert!(a.player.is_none());
        assert!(
            !Path::new(&format!("/proc/{pid}")).exists(),
            "the player must be killed and reaped when the alarm stops"
        );
    }

    #[cfg(unix)]
    #[test]
    fn dropping_the_alarm_kills_the_chime_too() {
        let child = Command::new("sleep").arg("30").spawn().expect("sleep");
        let pid = child.id();
        {
            let mut a = Alarm::new(false);
            a.player = Some(child);
        }
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
    }
}
