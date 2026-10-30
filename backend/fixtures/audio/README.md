# Audio fixtures

Short recorded clips that the backend tests replay instead of a live microphone or phone line. `vak-testkit` loads them (with `hound`), and the deterministic pipeline simulations, VAD and codec tests, and the end-to-end caller bot stream them as 20 ms frames.

There are no fixtures yet. They arrive with the voice core (M4).

## What goes here

| Kind | Format | Used for |
|---|---|---|
| Utterances | 16 kHz mono PCM16 WAV, 1–5 s | STT adapter tests, end-of-turn detection |
| Telephony calls | 8 kHz mono G.711 μ-law WAV | μ-law decode, resampling to 16 kHz, VAD on phone audio |
| Barge-in | 16 kHz PCM16: caller speech overlapping agent playback | Barge-in cancels TTS within one frame |
| Silence | 16 kHz PCM16, 0.5–3 s of digital silence | End-of-turn timing, no false VAD triggers |
| Noise | 16 kHz PCM16, background noise without speech | VAD false-positive rate (Earshot vs Silero A/B) |

## Rules

- **Synthetic or consented audio only.** Use TTS output, your own voice, or clips under a licence that allows redistribution (record the licence in `SOURCES.md` next to the clips). Never commit real customer calls or anyone's personal data.
- **Keep files small:** a few seconds each, a few MB in total. This repo is public and every worktree checks the files out.
- **Name files by what they test**, for example `utterance_hello_16k.wav`, `call_mulaw_8k.wav`, `bargein_overlap_16k.wav`, `silence_2s_16k.wav`.
- **Pair every clip that has speech with its expected transcript**, for example `utterance_hello_16k.txt`, so STT and pipeline tests can assert on it.
- **Treat fixtures as test inputs.** Changing one changes the expected results, so update the snapshots in the same commit.
