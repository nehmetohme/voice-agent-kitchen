# Audio fixture sources

| File | Source | Licence |
|---|---|---|
| `utterance_book_table_16k.wav` | Synthetic. Kokoro-82M (`mlx-community/Kokoro-82M-bf16`, voice `af_heart`, speed 1.0) through `vak-model-shim` and mlx-audio 0.6.3 on 2026-10-30, text in `utterance_book_table_16k.txt`. Resampled from 24 kHz with `ffmpeg -ar 16000 -c:a pcm_s16le -bitexact -map_metadata -1` | Kokoro-82M weights: Apache-2.0. The generated audio contains no personal data |
