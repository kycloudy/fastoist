//! The short chime played when a task is completed, synthesized at startup so the exe ships no audio files.

use std::sync::OnceLock;

const RATE: u32 = 44_100;

/// Two soft rising bell notes (E6 then A6) as a 16-bit mono WAV.
fn chime() -> &'static [u8] {
    static WAV: OnceLock<Vec<u8>> = OnceLock::new();
    WAV.get_or_init(|| {
        let len = (RATE as f32 * 0.45) as usize;
        let notes = [(1318.5_f32, 0.0_f32), (1760.0, 0.075)];
        let samples: Vec<i16> = (0..len)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                let mut v = 0.0;
                for (f, start) in notes {
                    let dt = t - start;
                    if dt >= 0.0 {
                        let attack = (dt / 0.004).min(1.0);
                        let env = attack * (-dt * 11.0).exp();
                        let w = std::f32::consts::TAU * f * dt;
                        v += env * (w.sin() + 0.25 * (2.0 * w).sin() + 0.08 * (3.0 * w).sin());
                    }
                }
                (v * 0.22 * i16::MAX as f32) as i16
            })
            .collect();
        let data_len = (samples.len() * 2) as u32;
        let mut b = Vec::with_capacity(44 + data_len as usize);
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data_len).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes()); // PCM
        b.extend_from_slice(&1u16.to_le_bytes()); // mono
        b.extend_from_slice(&RATE.to_le_bytes());
        b.extend_from_slice(&(RATE * 2).to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            b.extend_from_slice(&s.to_le_bytes());
        }
        b
    })
}

#[cfg(windows)]
pub fn complete() {
    use windows_sys::Win32::Media::Audio::{PlaySoundA, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
    let wav = chime();
    // SND_ASYNC needs the buffer to outlive playback; it is static.
    unsafe {
        PlaySoundA(wav.as_ptr(), std::ptr::null_mut(), SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
    }
}

#[cfg(not(windows))]
pub fn complete() {
    let _ = chime();
}

#[cfg(test)]
mod tests {
    #[test]
    fn wav_header() {
        let w = super::chime();
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(w[4..8].try_into().unwrap()) as usize, w.len() - 8);
    }
}
