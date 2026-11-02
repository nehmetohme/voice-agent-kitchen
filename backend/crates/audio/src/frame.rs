//! `AudioFrame`: 20 ms of mono PCM16 in a fixed-size buffer, so frames move through
//! channels without a heap allocation each.

/// Every frame holds this much audio.
pub const FRAME_MS: u32 = 20;

/// Samples in a frame at the highest supported rate (48 kHz).
pub const MAX_FRAME_SAMPLES: usize = 960;

/// 20 ms of mono signed 16-bit PCM at one sample rate (320 samples at 16 kHz, 480 at
/// 24 kHz).
#[derive(Clone, PartialEq, Eq)]
pub struct AudioFrame {
    samples: [i16; MAX_FRAME_SAMPLES],
    len: usize,
    sample_rate: u32,
}

/// The samples don't make one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    #[error("unsupported sample rate {0} Hz")]
    UnsupportedRate(u32),
    #[error("a 20 ms frame at this rate holds {expected} samples, got {got}")]
    WrongLength { expected: usize, got: usize },
}

impl AudioFrame {
    /// A frame of `samples` at `sample_rate`, which must be exactly 20 ms long.
    pub fn new(sample_rate: u32, samples: &[i16]) -> Result<Self, FrameError> {
        let expected =
            Self::samples_per_frame(sample_rate).ok_or(FrameError::UnsupportedRate(sample_rate))?;
        if samples.len() != expected {
            return Err(FrameError::WrongLength {
                expected,
                got: samples.len(),
            });
        }
        let mut frame = Self::silent(sample_rate)?;
        frame.samples[..expected].copy_from_slice(samples);
        Ok(frame)
    }

    /// 20 ms of silence at `sample_rate`.
    pub fn silent(sample_rate: u32) -> Result<Self, FrameError> {
        let len =
            Self::samples_per_frame(sample_rate).ok_or(FrameError::UnsupportedRate(sample_rate))?;
        Ok(Self {
            samples: [0; MAX_FRAME_SAMPLES],
            len,
            sample_rate,
        })
    }

    /// A frame of the first samples of `samples` at `sample_rate`, padded with silence when
    /// there are fewer than a frame's worth (the end of a clip).
    pub fn padded(sample_rate: u32, samples: &[i16]) -> Result<Self, FrameError> {
        let mut frame = Self::silent(sample_rate)?;
        let take = samples.len().min(frame.len);
        frame.samples[..take].copy_from_slice(&samples[..take]);
        Ok(frame)
    }

    /// Samples in one frame at `sample_rate` (8, 16, 24 or 48 kHz), or `None` for another
    /// rate.
    #[must_use]
    pub const fn samples_per_frame(sample_rate: u32) -> Option<usize> {
        match sample_rate {
            8_000 => Some(160),
            16_000 => Some(320),
            24_000 => Some(480),
            48_000 => Some(960),
            _ => None,
        }
    }

    #[must_use]
    pub fn samples(&self) -> &[i16] {
        self.samples.get(..self.len).unwrap_or_default()
    }

    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The frame's size as little-endian PCM16 bytes (the wire format).
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.len * 2
    }

    /// Writes the samples as little-endian PCM16 into the start of `out` and returns how
    /// many bytes it wrote: [`byte_len`](Self::byte_len), or fewer if `out` is shorter.
    /// Never allocates.
    pub fn copy_le_bytes(&self, out: &mut [u8]) -> usize {
        let mut written = 0;
        for (bytes, sample) in out.as_chunks_mut::<2>().0.iter_mut().zip(self.samples()) {
            *bytes = sample.to_le_bytes();
            written += 2;
        }
        written
    }

    /// The root mean square of the samples, from 0 to 32768.
    #[must_use]
    pub fn rms(&self) -> f32 {
        let samples = self.samples();
        if samples.is_empty() {
            return 0.0;
        }
        let energy: f64 = samples
            .iter()
            .map(|sample| {
                let sample = f64::from(*sample);
                sample * sample
            })
            .sum();
        // At most 960 samples: the count converts to f64 exactly.
        let count = f64::from(u32::try_from(samples.len()).unwrap_or(u32::MAX));
        // An RMS of 16-bit samples is at most 32768, so it fits f32 with room to spare.
        #[expect(clippy::cast_possible_truncation, reason = "the RMS is at most 32768")]
        let rms = (energy / count).sqrt() as f32;
        rms
    }
}

impl std::fmt::Debug for AudioFrame {
    // Audio is personal data: show its shape, never its samples.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioFrame")
            .field("sample_rate", &self.sample_rate)
            .field("samples", &self.len)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_is_20_ms_at_a_supported_rate() {
        for (rate, len) in [(8_000, 160), (16_000, 320), (24_000, 480), (48_000, 960)] {
            assert_eq!(AudioFrame::samples_per_frame(rate), Some(len));
            let frame = AudioFrame::new(rate, &vec![7; len]).unwrap();
            assert_eq!(frame.sample_rate(), rate);
            assert_eq!(frame.samples(), vec![7; len].as_slice());
        }
        assert_eq!(
            AudioFrame::new(44_100, &[0; 882]),
            Err(FrameError::UnsupportedRate(44_100))
        );
        assert_eq!(
            AudioFrame::new(16_000, &[0; 319]),
            Err(FrameError::WrongLength {
                expected: 320,
                got: 319
            })
        );
    }

    #[test]
    fn a_frame_writes_little_endian_bytes_into_a_given_buffer() {
        let mut samples = [0; 480];
        samples[0] = 1;
        samples[1] = -2;
        let frame = AudioFrame::new(24_000, &samples).unwrap();
        assert_eq!(frame.byte_len(), 960);

        let mut out = [0xAA; 1_000];
        assert_eq!(frame.copy_le_bytes(&mut out), 960);
        assert_eq!(&out[..4], [1, 0, 0xFE, 0xFF].as_slice());
        assert_eq!(out[960], 0xAA, "nothing past the frame is touched");
        let mut short = [0; 3];
        assert_eq!(frame.copy_le_bytes(&mut short), 2);
    }

    #[test]
    fn a_short_clip_end_is_padded_with_silence() {
        let frame = AudioFrame::padded(24_000, &[5; 100]).unwrap();
        assert_eq!(frame.samples().len(), 480);
        assert_eq!(&frame.samples()[..100], [5; 100].as_slice());
        assert!(frame.samples()[100..].iter().all(|sample| *sample == 0));
        let long = AudioFrame::padded(16_000, &[1; 1_000]).unwrap();
        assert_eq!(long.samples(), [1; 320].as_slice());
        assert!(AudioFrame::padded(11_025, &[]).is_err());
    }

    #[test]
    fn rms_measures_loudness() {
        assert!(AudioFrame::silent(16_000).unwrap().rms().abs() < f32::EPSILON);
        let loud = AudioFrame::new(16_000, &[1_000; 320]).unwrap();
        assert!((loud.rms() - 1_000.0).abs() < 0.01);
        let square: Vec<i16> = (0..320)
            .map(|i| if i % 2 == 0 { 300 } else { -300 })
            .collect();
        assert!((AudioFrame::new(16_000, &square).unwrap().rms() - 300.0).abs() < 0.01);
    }

    #[test]
    fn debug_never_shows_samples() {
        let frame = AudioFrame::new(16_000, &[12_345; 320]).unwrap();
        let debug = format!("{frame:?}");
        assert!(debug.contains("320") && !debug.contains("12345"));
    }
}
