#!/usr/bin/env bash
# Runs the mlx-audio speech server (Parakeet STT, Kokoro TTS on MLX) from the project-local
# uv environment in deploy/local/mlx-audio (pinned by its uv.lock). process-compose starts it.
#
#   deploy/local/mlx-audio.sh           sync the environment if needed, then serve on :8000
#   deploy/local/mlx-audio.sh --sync    only sync the environment
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=deploy/local/env.sh
. "$here/env.sh"
project="$here/mlx-audio"

uv sync --project "$project" --locked --quiet
[ "${1:-}" = --sync ] && exit 0

# espeak-ng (Kokoro's fallback for unknown words) keeps its data path in a 160-byte buffer;
# a worktree's venv path is longer, and espeak then exits the whole server. A short
# symlink in TMPDIR, passed as ESPEAK_DATA_PATH, keeps it under the limit.
loader=$(echo "$project"/.venv/lib/python3.*/site-packages/espeakng_loader)
[ -d "$loader/espeak-ng-data" ] || {
  echo "mlx-audio: espeak-ng data not found under $loader" >&2
  exit 1
}
short="${TMPDIR:-/tmp}"
short="${short%/}/vak-espeak-ng"
ln -sfn "$loader" "$short"
export ESPEAK_DATA_PATH="$short"

# Models come from VAK_MODELS_DIR only (`just models-pull`); never download at run time.
export HF_HUB_OFFLINE=1
exec uv run --project "$project" --frozen --no-sync mlx_audio.server \
  --host 127.0.0.1 --port 8000 \
  --log-dir "${TMPDIR:-/tmp}/vak-mlx-audio-logs"
