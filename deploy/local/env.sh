# shellcheck shell=bash
# Sourced by the local dev scripts and recipes: loads the repo's .env (gitignored; copy it
# from .env.example) and fills in the defaults of the local stack. Prints nothing, so no
# secret reaches a log.
#
#   . deploy/local/env.sh

VAK_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
if [ -f "$VAK_ROOT/.env" ]; then
  set -a
  # shellcheck source=/dev/null
  . "$VAK_ROOT/.env"
  set +a
fi

# LLM: no default. It runs on a LAN GPU box, configured in .env (see .env.example).
: "${VAK_LLM_BASE_URL:=}"
: "${VAK_LLM_MODEL:=}"
: "${VAK_LLM_API_KEY:=}"
[ -n "${VAK_LLM_EXTRA_BODY:-}" ] || VAK_LLM_EXTRA_BODY='{}'

# Models on disk, shared by every worktree (deploy/local/models.sh).
: "${VAK_MODELS_DIR:=$HOME/.cache/vak/models}"

# Speech: mlx-audio on :8000 behind vak-model-shim on :8001 (Databricks invocation shapes).
# The ports are fixed in process-compose.yml.
: "${VAK_STT_BASE_URL:=http://127.0.0.1:8001}"
: "${VAK_STT_ENDPOINT:=stt}"
: "${VAK_TTS_BASE_URL:=http://127.0.0.1:8001}"
: "${VAK_TTS_ENDPOINT:=tts}"
: "${VAK_SHIM_ADDR:=127.0.0.1:8001}"
: "${VAK_SHIM_MLX_AUDIO_URL:=http://127.0.0.1:8000}"
: "${VAK_SHIM_STT_MODEL:=$VAK_MODELS_DIR/mlx-community/parakeet-tdt-0.6b-v2}"
: "${VAK_SHIM_TTS_MODEL:=$VAK_MODELS_DIR/mlx-community/Kokoro-82M-bf16}"

# Decisions: llama-server /v1/systemone with Kev-4B on :8081.
: "${VAK_DECISION_BASE_URL:=http://127.0.0.1:8081}"
: "${VAK_DECISION_MODEL:=Kev-4B}"
: "${VAK_DECISION_GGUF:=$VAK_MODELS_DIR/ggml-org/Kev-4B-GGUF/Kev-4B-Q4_K_M.gguf}"

# Postgres 18 in Docker. Host port 5433: a Homebrew Postgres often holds 5432.
: "${VAK_PG_PORT:=5433}"
: "${VAK_PG_PASSWORD:=}"
: "${DATABASE_URL:=postgres://vak:${VAK_PG_PASSWORD}@127.0.0.1:${VAK_PG_PORT}/vak}"

# vak-server and the Vite dev server (which proxies /api to it).
: "${VAK_HTTP_ADDR:=127.0.0.1:3000}"
: "${VAK_LOG:=info}"

export VAK_ROOT VAK_LLM_BASE_URL VAK_LLM_MODEL VAK_LLM_API_KEY VAK_LLM_EXTRA_BODY \
  VAK_MODELS_DIR VAK_STT_BASE_URL VAK_STT_ENDPOINT VAK_TTS_BASE_URL VAK_TTS_ENDPOINT \
  VAK_SHIM_ADDR VAK_SHIM_MLX_AUDIO_URL VAK_SHIM_STT_MODEL VAK_SHIM_TTS_MODEL \
  VAK_DECISION_BASE_URL VAK_DECISION_MODEL VAK_DECISION_GGUF VAK_PG_PORT VAK_PG_PASSWORD \
  DATABASE_URL VAK_HTTP_ADDR VAK_LOG
