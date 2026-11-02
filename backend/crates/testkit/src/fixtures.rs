//! Audio fixtures from `backend/fixtures/audio/` (see its `README.md`).

use std::path::PathBuf;

/// A recorded utterance and what it says.
#[derive(Clone, PartialEq, Eq)]
pub struct Utterance {
    samples: Vec<i16>,
    transcript: String,
}

impl Utterance {
    /// 16 kHz mono PCM16.
    #[must_use]
    pub fn samples(&self) -> &[i16] {
        &self.samples
    }

    /// The words, as written in the fixture's `.txt` file.
    #[must_use]
    pub fn transcript(&self) -> &str {
        &self.transcript
    }
}

impl std::fmt::Debug for Utterance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Utterance")
            .field("samples", &self.samples.len())
            .finish_non_exhaustive()
    }
}

/// `utterance_book_table_16k.wav`: 3.8 s, "Hi, I would like to book a table for two tomorrow
/// at seven."
///
/// # Panics
/// When the fixture is missing or is not a canonical 16 kHz mono PCM16 WAV.
#[must_use]
pub fn utterance_book_table() -> Utterance {
    load("utterance_book_table_16k")
}

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio")
}

#[expect(
    clippy::panic,
    reason = "a broken fixture fails the test that loads it"
)]
fn load(name: &str) -> Utterance {
    let wav_path = dir().join(format!("{name}.wav"));
    let txt_path = dir().join(format!("{name}.txt"));
    let wav =
        std::fs::read(&wav_path).unwrap_or_else(|e| panic!("reading {}: {e}", wav_path.display()));
    let transcript = std::fs::read_to_string(&txt_path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", txt_path.display()));
    let samples = canonical_pcm16_mono_16k(&wav)
        .unwrap_or_else(|why| panic!("{}: {why}", wav_path.display()));
    Utterance {
        samples,
        transcript: transcript.trim().to_owned(),
    }
}

/// The samples of a canonical WAV (44-byte header: `fmt ` then `data`), if it is PCM16,
/// mono, 16 kHz.
fn canonical_pcm16_mono_16k(wav: &[u8]) -> Result<Vec<i16>, &'static str> {
    let field = |at: usize, len: usize| wav.get(at..at + len).ok_or("the file is too short");
    let u16_at = |at: usize| field(at, 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32_at = |at: usize| field(at, 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    if field(0, 4)? != b"RIFF" || field(8, 4)? != b"WAVE" || field(12, 4)? != b"fmt " {
        return Err("not a RIFF WAVE file with fmt first");
    }
    if u16_at(20)? != 1 || u16_at(22)? != 1 || u32_at(24)? != 16_000 || u16_at(34)? != 16 {
        return Err("not PCM16 mono at 16 kHz");
    }
    if field(36, 4)? != b"data" {
        return Err("no data chunk right after fmt");
    }
    let len = usize::try_from(u32_at(40)?).map_err(|_| "data chunk too large")?;
    let data = wav.get(44..44 + len).ok_or("the data chunk is truncated")?;
    Ok(data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_book_table_fixture_loads_as_16k_mono() {
        let utterance = utterance_book_table();
        // 3.8 s at 16 kHz.
        assert!((60_000..62_000).contains(&utterance.samples().len()));
        assert_eq!(
            utterance.transcript(),
            "Hi, I would like to book a table for two tomorrow at seven."
        );
    }

    #[test]
    fn a_wav_that_is_not_16k_mono_pcm16_is_refused() {
        assert!(canonical_pcm16_mono_16k(b"RIFF").is_err());
        let mut wav = std::fs::read(dir().join("utterance_book_table_16k.wav")).unwrap();
        wav[24..28].copy_from_slice(&8_000_u32.to_le_bytes());
        assert!(canonical_pcm16_mono_16k(&wav).is_err());
    }
}
