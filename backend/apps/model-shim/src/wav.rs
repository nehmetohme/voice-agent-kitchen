//! Just enough RIFF/WAVE: wrap PCM s16le mono in a header for mlx-audio, and read the
//! samples and format back out of the WAV it answers.

/// PCM samples and their format, borrowed from a WAV file.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Wav<'a> {
    /// 1 is integer PCM.
    pub(crate) format_tag: u16,
    pub(crate) channels: u16,
    pub(crate) sample_rate: u32,
    pub(crate) bits_per_sample: u16,
    pub(crate) data: &'a [u8],
}

const PCM: u16 = 1;

/// A canonical 44-byte header followed by `pcm` (16-bit little-endian mono samples).
/// `None` if `pcm` is too long for a WAV file (4 GiB).
pub(crate) fn pcm16_mono(sample_rate: u32, pcm: &[u8]) -> Option<Vec<u8>> {
    let data_len = u32::try_from(pcm.len()).ok()?;
    let riff_len = data_len.checked_add(36)?;
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_len.to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&PCM.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&sample_rate.checked_mul(2)?.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(pcm);
    Some(out)
}

/// Reads the `fmt ` and `data` chunks. A `data` chunk that claims more bytes than the file
/// has (what streaming writers leave) gets the rest of the file.
pub(crate) fn parse(bytes: &[u8]) -> Result<Wav<'_>, &'static str> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE file");
    }
    let mut format = None;
    let mut at = 12;
    while let Some(header) = bytes.get(at..at + 8) {
        let id = &header[0..4];
        let len = usize::try_from(u32_le(&header[4..8])).map_err(|_| "chunk too large")?;
        let body_start = at + 8;
        let body_end = body_start.saturating_add(len);
        if id == b"data" {
            let data = bytes
                .get(body_start..body_end)
                .unwrap_or(&bytes[body_start..]);
            let (format_tag, channels, sample_rate, bits_per_sample) =
                format.ok_or("no fmt chunk before the data chunk")?;
            return Ok(Wav {
                format_tag,
                channels,
                sample_rate,
                bits_per_sample,
                data,
            });
        }
        let body = bytes.get(body_start..body_end).ok_or("truncated chunk")?;
        if id == b"fmt " {
            if body.len() < 16 {
                return Err("fmt chunk too short");
            }
            format = Some((
                u16_le(&body[0..2]),
                u16_le(&body[2..4]),
                u32_le(&body[4..8]),
                u16_le(&body[14..16]),
            ));
        }
        at = body_end + len % 2;
    }
    Err("no data chunk")
}

fn u16_le(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn u32_le(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_written_header_parses_back() {
        let pcm = [1u8, 0, 2, 0, 255, 255];

        let file = pcm16_mono(16_000, &pcm).expect("small");

        assert_eq!(file.len(), 44 + pcm.len());
        assert_eq!(
            parse(&file),
            Ok(Wav {
                format_tag: 1,
                channels: 1,
                sample_rate: 16_000,
                bits_per_sample: 16,
                data: &pcm,
            })
        );
    }

    #[test]
    fn extra_chunks_are_skipped_including_odd_padding() {
        let mut file = pcm16_mono(24_000, &[7, 0]).expect("small");
        // Insert a 3-byte LIST chunk (padded to 4) between fmt and data.
        let list = [b"LIST".as_slice(), &3u32.to_le_bytes(), b"abc\0"].concat();
        file.splice(36..36, list);

        let wav = parse(&file).expect("parses");

        assert_eq!((wav.sample_rate, wav.data), (24_000, [7u8, 0].as_slice()));
    }

    #[test]
    fn an_oversized_data_chunk_takes_the_rest_of_the_file() {
        let mut file = pcm16_mono(24_000, &[1, 0, 2, 0]).expect("small");
        file[40..44].copy_from_slice(&u32::MAX.to_le_bytes());

        assert_eq!(
            parse(&file).expect("parses").data,
            [1u8, 0, 2, 0].as_slice()
        );
    }

    #[test]
    fn malformed_files_are_rejected() {
        let good = pcm16_mono(16_000, &[0, 0]).expect("small");
        let mut no_fmt = good.clone();
        no_fmt[12..16].copy_from_slice(b"junk");
        let cases: [(&[u8], &str); 4] = [
            (b"<html>", "not a RIFF/WAVE file"),
            (&good[..36], "no data chunk"),
            (&good[..30], "truncated chunk"),
            (&no_fmt, "no fmt chunk before the data chunk"),
        ];
        for (bytes, reason) in cases {
            assert_eq!(parse(bytes), Err(reason));
        }
    }
}
