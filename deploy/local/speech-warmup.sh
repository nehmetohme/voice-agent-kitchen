#!/usr/bin/env bash
# Warms mlx-audio once it answers: loads Parakeet and Kokoro, then runs one tiny
# transcription and one synthesis (Kokoro builds its G2P pipeline on first use, ~4 s).
# process-compose runs it before vak-model-shim, so the first real call is warm.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=env.sh
. "$here/env.sh"
mlx=$VAK_SHIM_MLX_AUDIO_URL
fixture="$VAK_ROOT/backend/fixtures/audio/utterance_book_table_16k.wav"

for model in "$VAK_SHIM_STT_MODEL" "$VAK_SHIM_TTS_MODEL"; do
  [ -d "$model" ] || {
    echo "speech-warmup: no model at $model: run \`just models-pull\`" >&2
    exit 1
  }
  curl -fsS -m 120 -o /dev/null -X POST -G --data-urlencode "model_name=$model" "$mlx/v1/models"
done
curl -fsS -m 60 -o /dev/null "$mlx/v1/audio/transcriptions" \
  -F "file=@$fixture" -F "model=$VAK_SHIM_STT_MODEL" -F response_format=json
curl -fsS -m 60 -o /dev/null "$mlx/v1/audio/speech" -H 'content-type: application/json' \
  -d "{\"model\":\"$VAK_SHIM_TTS_MODEL\",\"input\":\"Ready.\",\"voice\":\"af_heart\",\"response_format\":\"wav\"}"
echo "speech-warmup: Parakeet and Kokoro are loaded and warm"
