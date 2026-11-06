//! The island's equalizer: WASAPI loopback of what the PC plays, turned into
//! four band levels ([`poni_core::viz::Bands`]).
//!
//! One thread, `poni-viz`, exists only while a song plays on the island and
//! someone is at the PC ([`crate::services::Services::viz_watch`]): the
//! playback device is opened when it starts and given back when it stops, so
//! nothing is captured at rest. It reads the same loopback the Meeting notes
//! page uses (`mic::open_endpoint` with `eRender`) and the same 16 kHz
//! downmix, then keeps only four numbers.

use crate::services::mic::{self, POLL};
use poni_core::ielts::RATE;
use poni_core::viz::{Bands, BANDS};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eRender, AUDCLNT_BUFFERFLAGS_SILENT};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

struct Shared {
    stop: AtomicBool,
    /// f32 bits: 0..1, per band.
    levels: [AtomicU32; BANDS],
}

/// A running equalizer. Dropping it ends the thread and gives the device back.
pub struct Viz {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Viz {
    /// Opens the playback device on a thread of its own. A device that won't
    /// open leaves every bar at zero, so the island keeps its stand-in bars.
    pub fn start() -> Viz {
        let shared = Arc::new(Shared { stop: AtomicBool::new(false), levels: std::array::from_fn(|_| AtomicU32::new(0)) });
        let worker = shared.clone();
        let thread = std::thread::Builder::new().name("poni-viz".into()).spawn(move || run(&worker)).ok();
        Viz { shared, thread }
    }

    /// The bars' heights, 0..1, left to right; zero until sound comes.
    pub fn levels(&self) -> [f32; BANDS] {
        std::array::from_fn(|i| f32::from_bits(self.shared.levels[i].load(Ordering::Relaxed)))
    }
}

impl Drop for Viz {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Loopback carries Windows' master volume already applied. Undo it, so the
/// bars follow the track and not the Windows slider. The gain is capped: at a
/// near-silent endpoint it would otherwise lift the noise floor into the bars.
fn gain_for(level: f32, muted: bool) -> f32 {
    if muted || !level.is_finite() || level <= 0.005 {
        0.0
    } else {
        (1.0 / level).clamp(1.0, 20.0)
    }
}

fn volume_gain(volume: Option<&IAudioEndpointVolume>) -> f32 {
    let Some(volume) = volume else { return 1.0 };
    let muted = unsafe { volume.GetMute() }.map(|m| m.as_bool()).unwrap_or(false);
    let level = unsafe { volume.GetMasterVolumeLevelScalar() }.unwrap_or(1.0);
    gain_for(level, muted)
}

/// The `poni-viz` thread: open the playback device, take what it plays until
/// told to stop, and report the bars.
fn run(shared: &Shared) {
    let com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if let Ok(mut d) = mic::open_endpoint(eRender, "equalizer") {
        if unsafe { d.client.Start() }.is_ok() {
            let mut bands = Bands::new();
            let mut pcm: Vec<i16> = Vec::with_capacity(4096);
            // Loopback sends nothing while nothing plays, so a poll that
            // heard nothing pushes silence: that is what lets a bar fall.
            let quiet = vec![0i16; (RATE as f32 * POLL.as_secs_f32()) as usize];
            while !shared.stop.load(Ordering::SeqCst) {
                let gain = volume_gain(d.volume.as_ref());
                let mut heard = false;
                loop {
                    let frames = match unsafe { d.capture.GetNextPacketSize() } {
                        Ok(f) => f,
                        // The device went away: stop quietly.
                        Err(_) => break,
                    };
                    if frames == 0 {
                        break;
                    }
                    let (mut data, mut n, mut flags) = (std::ptr::null_mut::<u8>(), 0u32, 0u32);
                    if unsafe { d.capture.GetBuffer(&mut data, &mut n, &mut flags, None, None) }.is_err() {
                        break;
                    }
                    pcm.clear();
                    if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                        d.downmix.silence(n as usize, &mut pcm);
                    } else {
                        let bytes = unsafe { std::slice::from_raw_parts(data, n as usize * d.block) };
                        d.downmix.push(bytes, &mut pcm);
                    }
                    let _ = unsafe { d.capture.ReleaseBuffer(n) };
                    if gain == 0.0 {
                        pcm.fill(0);
                    } else if (gain - 1.0).abs() > 0.01 {
                        for s in &mut pcm {
                            *s = (f32::from(*s) * gain).clamp(-32_768.0, 32_767.0) as i16;
                        }
                    }
                    bands.push(&pcm);
                    heard = true;
                }
                if !heard {
                    bands.push(&quiet);
                }
                for (i, level) in bands.levels().into_iter().enumerate() {
                    shared.levels[i].store(level.to_bits(), Ordering::Relaxed);
                }
                std::thread::sleep(POLL);
            }
            unsafe {
                let _ = d.client.Stop();
            }
        }
    }
    if com.is_ok() {
        unsafe { CoUninitialize() };
    }
    // Nothing plays once this ends: every bar back down.
    for level in &shared.levels {
        level.store(0f32.to_bits(), Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::gain_for;

    #[test]
    fn the_gain_undoes_the_windows_slider() {
        assert!((gain_for(1.0, false) - 1.0).abs() < 0.001);
        assert!((gain_for(0.5, false) - 2.0).abs() < 0.001);
        assert!((gain_for(0.25, false) - 4.0).abs() < 0.001);
    }

    #[test]
    fn a_mute_or_a_dead_slider_is_silence() {
        assert_eq!(gain_for(0.8, true), 0.0);
        assert_eq!(gain_for(0.0, false), 0.0);
    }

    #[test]
    fn a_near_silent_slider_is_capped() {
        assert_eq!(gain_for(0.01, false), 20.0);
    }
}
