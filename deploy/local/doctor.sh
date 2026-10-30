#!/usr/bin/env bash
# Checks everything the local stack needs, without starting anything (`just dev-doctor`).
# Each line is ok, warn or FAIL, with a fix hint; exits 1 when anything fails.
set -uo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=deploy/local/env.sh
. "$here/env.sh"

failures=0
ok() { printf '  ok    %-14s %s\n' "$1" "$2"; }
warn() { printf '  warn  %-14s %s\n' "$1" "$2"; }
fail() {
  printf '  FAIL  %-14s %s\n        fix: %s\n' "$1" "$2" "$3"
  failures=$((failures + 1))
}
have() { command -v "$1" >/dev/null 2>&1; }

echo "Local dev stack ($VAK_ROOT)"

# ---- Machine and tools -----------------------------------------------------------------
if [ "$(uname -s)/$(uname -m)" = Darwin/arm64 ]; then
  ok machine "macOS on Apple Silicon (MLX needs Metal)"
else
  fail machine "$(uname -s)/$(uname -m)" "mlx-audio needs an Apple Silicon Mac"
fi
for tool in just curl jq perl cargo pnpm; do
  if have "$tool"; then ok "$tool" "$(command -v "$tool")"; else fail "$tool" "not on PATH" "install $tool (see README.md, Development)"; fi
done
if have uv; then ok uv "$(uv --version)"; else fail uv "not on PATH" "brew install uv"; fi
if have process-compose; then
  ok process-compose "$(process-compose version --short 2>/dev/null || echo installed)"
else
  fail process-compose "not on PATH" "brew install f1bonacc1/tap/process-compose"
fi

# ---- Docker and Postgres ----------------------------------------------------------------
if ! have docker; then
  fail docker "not on PATH" "install Docker Desktop"
elif ! docker info >/dev/null 2>&1; then
  fail docker "the daemon is not running" "start Docker Desktop"
else
  ok docker "daemon running ($(docker version --format '{{.Server.Version}}' 2>/dev/null))"
fi
if [ -n "$VAK_PG_PASSWORD" ]; then
  ok postgres "password set; host port $VAK_PG_PORT"
else
  fail postgres "VAK_PG_PASSWORD is empty" "set it in .env (cp .env.example .env)"
fi

# ---- .env and the LLM on the LAN GPU box -------------------------------------------------
main_env="$(git -C "$VAK_ROOT" rev-parse --path-format=absolute --git-common-dir 2>/dev/null)/../.env"
if [ -f "$VAK_ROOT/.env" ]; then
  ok .env "$VAK_ROOT/.env"
elif [ -f "$main_env" ]; then
  # Claude Code worktrees get it through .worktreeinclude; `just task` worktrees don't.
  fail .env "missing in this worktree" "cp \"$(cd "$(dirname "$main_env")" && pwd)/.env\" \"$VAK_ROOT/\""
else
  fail .env "missing" "cp .env.example .env, then set VAK_LLM_BASE_URL and VAK_PG_PASSWORD"
fi
case "$VAK_LLM_BASE_URL" in
  "" | *"<"*)
    fail llm "VAK_LLM_BASE_URL is not set" "put the GPU box's URL in .env, e.g. http://<host>:<port>/v1"
    ;;
  *)
    auth=()
    [ -z "$VAK_LLM_API_KEY" ] || auth=(-H "Authorization: Bearer $VAK_LLM_API_KEY")
    if models=$(curl -fsS -m 3 ${auth[@]+"${auth[@]}"} "$VAK_LLM_BASE_URL/models" 2>&1); then
      if jq -e --arg m "$VAK_LLM_MODEL" '[.data[].id] | index($m)' >/dev/null 2>&1 <<<"$models"; then
        ok llm "$VAK_LLM_MODEL answers at $VAK_LLM_BASE_URL"
      else
        fail llm "$VAK_LLM_BASE_URL serves $(jq -rc '[.data[].id]' <<<"$models" 2>/dev/null), not $VAK_LLM_MODEL" \
          "set VAK_LLM_MODEL in .env to one of those ids"
      fi
    else
      fail llm "$VAK_LLM_BASE_URL/models did not answer in 3 s" \
        "is the GPU box on and llama-server running? same LAN or VPN? check VAK_LLM_BASE_URL"
    fi
    ;;
esac
if jq -e 'type == "object"' >/dev/null 2>&1 <<<"$VAK_LLM_EXTRA_BODY"; then
  if jq -e '.chat_template_kwargs.enable_thinking == false' >/dev/null <<<"$VAK_LLM_EXTRA_BODY"; then
    ok llm-thinking "off (chat_template_kwargs.enable_thinking=false)"
  else
    warn llm-thinking "VAK_LLM_EXTRA_BODY doesn't turn thinking off; voice turns need it off"
  fi
else
  fail llm-thinking "VAK_LLM_EXTRA_BODY is not a JSON object" "quote it in .env: VAK_LLM_EXTRA_BODY='{...}'"
fi

# ---- Models, decisions and speech ---------------------------------------------------------
if "$here/models.sh" --check >/dev/null 2>&1; then
  ok models "all present and verified in $VAK_MODELS_DIR"
else
  fail models "missing or unverified in $VAK_MODELS_DIR" "just models-pull"
fi
if have llama-server; then
  bin=$(command -v llama-server)
  real=$(cd "$(dirname "$bin")" && cd "$(dirname "$(readlink "$bin" || echo "$bin")")" && pwd)
  if grep -aqs '/v1/systemone' "$bin" "$real"/../lib/libllama-server* 2>/dev/null; then
    ok llama-server "$(llama-server --version 2>&1 | head -1); has /v1/systemone"
  else
    fail llama-server "$(llama-server --version 2>&1 | head -1); no /v1/systemone" \
      "brew upgrade llama.cpp (needs ggml-org/llama.cpp#29818, merged 2026-10-02)"
  fi
else
  fail llama-server "not on PATH" "brew install llama.cpp"
fi
if [ -x "$here/mlx-audio/.venv/bin/mlx_audio.server" ]; then
  ok mlx-audio "environment synced (deploy/local/mlx-audio/.venv)"
else
  fail mlx-audio "environment not synced" "just dev-setup"
fi
if [ -d "$VAK_ROOT/frontend/node_modules" ]; then
  ok frontend "dependencies installed"
else
  fail frontend "node_modules missing" "just frontend-install"
fi

# ---- Ports ----------------------------------------------------------------------------------
for entry in "$VAK_PG_PORT:postgres" 8081:decisions 8000:mlx-audio 8001:model-shim 3000:vak-server 5173:frontend; do
  port=${entry%%:*}
  name=${entry#*:}
  holder=$(lsof +c 0 -nP -iTCP:"$port" -sTCP:LISTEN 2>/dev/null | awk 'NR == 2 { print $1 " (pid " $2 ")" }')
  if [ -z "$holder" ]; then
    ok "port $port" "free for $name"
  else
    warn "port $port" "in use by $holder: fine if the stack is already up, else stop it"
  fi
done

echo
if [ "$failures" -eq 0 ]; then
  echo "dev-doctor: ready. Start the stack with \`just dev\`."
else
  echo "dev-doctor: $failures problem(s) above."
  exit 1
fi
