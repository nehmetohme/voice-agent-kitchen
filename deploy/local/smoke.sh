#!/usr/bin/env bash
# End-to-end smoke test of the running local stack (`just dev-smoke`, after `just dev`).
# Each check sends one real request twice, first then warm, and prints both latencies.
# Exits 1 when any check fails. Prints no payloads beyond a short sample of model output.
set -uo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=env.sh
. "$here/env.sh"
fixture="$VAK_ROOT/backend/fixtures/audio/utterance_book_table_16k.wav"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/vak-smoke.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

failures=0
now_ms() { perl -MTime::HiRes=time -e 'printf "%d\n", time * 1000'; }
# Seconds (curl's %{time_total}) to whole milliseconds.
secs_to_ms() { awk -v s="$1" 'BEGIN { printf "%d", s * 1000 }'; }
norm() { tr '[:upper:]' '[:lower:]' | tr -cs 'a-z0-9' ' ' | sed 's/^ *//; s/ *$//'; }

# post URL BODY-FILE OUT-FILE: prints "<http code> <ms>"
post() {
  local result
  result=$(curl -sS -m 60 -o "$3" -w '%{http_code} %{time_total}' \
    -H 'content-type: application/json' -d @"$2" "$1" 2>"$3.err") || result="000 0"
  echo "${result% *} $(secs_to_ms "${result#* }")"
}

# Each check sets LAT (ms) and DETAIL, and returns non-zero on failure.
LAT=0
DETAIL=""

check_llm() {
  local start first end text thinking speed
  # Headers from a file (in the private temp dir): the key never shows in `ps`.
  printf 'Content-Type: application/json\n' >"$tmp/llm.headers"
  [ -z "$VAK_LLM_API_KEY" ] || printf 'Authorization: Bearer %s\n' "$VAK_LLM_API_KEY" >>"$tmp/llm.headers"
  jq -n --arg model "$VAK_LLM_MODEL" --argjson extra "$VAK_LLM_EXTRA_BODY" '{
      model: $model, stream: true, max_tokens: 32,
      messages: [
        {role: "system", content: "You are a restaurant phone agent. Reply in one short sentence."},
        {role: "user", content: "Hi, are you open tonight?"}
      ]
    } + $extra' >"$tmp/llm.json"
  start=$(now_ms)
  # perl stamps the first chunk that carries content: time to first token, client side.
  curl -sS -N -m 60 -H @"$tmp/llm.headers" \
    -d @"$tmp/llm.json" "$VAK_LLM_BASE_URL/chat/completions" 2>"$tmp/llm.err" |
    perl -MTime::HiRes=time -ne 'BEGIN { $| = 1 }
      if (!$seen && /"content":"[^"]/) { open(my $f, ">", $ENV{FIRST}); printf $f "%d\n", time * 1000; $seen = 1 }
      print' >"$tmp/llm.sse"
  end=$(now_ms)
  first=$(cat "$tmp/llm.first" 2>/dev/null || echo "$end")
  text=$(sed -n 's/^data: //p' "$tmp/llm.sse" | grep -v '^\[DONE\]' | jq -rj '.choices[0].delta.content // empty' 2>/dev/null)
  thinking=$(sed -n 's/^data: //p' "$tmp/llm.sse" | grep -v '^\[DONE\]' | jq -rj '.choices[0].delta.reasoning_content // empty' 2>/dev/null)
  speed=$(sed -n 's/^data: //p' "$tmp/llm.sse" | grep -v '^\[DONE\]' | jq -r 'select(.timings) | .timings.predicted_per_second | floor' 2>/dev/null | tail -1)
  rm -f "$tmp/llm.first"
  LAT=$((first - start))
  if [ -z "$text" ]; then
    DETAIL="no content from $VAK_LLM_BASE_URL: $(head -c 160 "$tmp/llm.err" "$tmp/llm.sse" | tr '\n' ' ')"
    return 1
  fi
  if [ -n "$thinking" ]; then
    DETAIL="the model is thinking: VAK_LLM_EXTRA_BODY must set chat_template_kwargs.enable_thinking=false"
    return 1
  fi
  DETAIL="TTFT; total $((end - start)) ms${speed:+, $speed tok/s}, thinking off: \"$(printf '%s' "$text" | head -c 48)…\""
}

check_stt() {
  local code expected text
  read -r code LAT < <(post "$VAK_STT_BASE_URL/serving-endpoints/$VAK_STT_ENDPOINT/invocations" "$tmp/stt.json" "$tmp/stt.out")
  if [ "$code" != 200 ]; then
    DETAIL="HTTP $code: $(head -c 200 "$tmp/stt.out" "$tmp/stt.out.err" 2>/dev/null | tr '\n' ' ')"
    return 1
  fi
  text=$(jq -r '.predictions[0].text' "$tmp/stt.out")
  expected=$(norm <"${fixture%.wav}.txt")
  if [ -z "$text" ]; then
    DETAIL="empty transcript"
    return 1
  elif [ "$(printf '%s' "$text" | norm)" = "$expected" ]; then
    DETAIL="$STT_SECONDS s fixture, transcript matches: \"$text\""
  else
    DETAIL="$STT_SECONDS s fixture, transcript differs: \"$text\""
  fi
}

check_tts() {
  local code bytes
  read -r code LAT < <(post "$VAK_TTS_BASE_URL/serving-endpoints/$VAK_TTS_ENDPOINT/invocations" "$tmp/tts.json" "$tmp/tts.out")
  if [ "$code" != 200 ]; then
    DETAIL="HTTP $code: $(head -c 200 "$tmp/tts.out" "$tmp/tts.out.err" 2>/dev/null | tr '\n' ' ')"
    return 1
  fi
  bytes=$(jq -r '.predictions[0].audio_b64' "$tmp/tts.out" | base64 -d | wc -c | tr -d ' ')
  if [ "${bytes:-0}" -lt 4800 ]; then
    DETAIL="only $bytes bytes of audio"
    return 1
  fi
  DETAIL="$(awk -v b="$bytes" 'BEGIN { printf "%.1f", b / 48000 }') s of 24 kHz audio for a 12-word clause"
}

check_decision() {
  local code
  read -r code LAT < <(post "$VAK_DECISION_BASE_URL/v1/systemone" "$tmp/decision.json" "$tmp/decision.out")
  if [ "$code" != 200 ] || ! jq -e '.answers | has("branch") and has("intent") and has("opt_out") and has("escalate")' "$tmp/decision.out" >/dev/null 2>&1; then
    DETAIL="HTTP $code: $(head -c 200 "$tmp/decision.out" "$tmp/decision.out.err" 2>/dev/null | tr '\n' ' ')"
    return 1
  fi
  DETAIL=$(jq -r '"4-question fan-out: branch=\(.answers.branch.choice), opt_out p=\(.answers.opt_out.noul * 100 | floor / 100), \(.usage.input_tokens) tokens"' "$tmp/decision.out")
}

check_postgres() {
  local start out
  start=$(now_ms)
  out=$(docker compose -f "$here/docker-compose.yml" exec -T db \
    psql --username=vak --dbname=vak --tuples-only --no-align -c '\timing on' -c 'select 1' 2>&1)
  LAT=$(($(now_ms) - start))
  if ! grep -qx 1 <<<"$out"; then
    DETAIL="select 1 failed: $(head -c 200 <<<"$out" | tr '\n' ' ')"
    return 1
  fi
  DETAIL="select 1 = 1; server $(sed -n 's/^Time: //p' <<<"$out"), the rest is docker exec"
}

check_http() { # url
  local code
  code=$(curl -sS -m 10 -o "$tmp/http.out" -w '%{http_code} %{time_total}' "$1" 2>/dev/null) || code="000 0"
  LAT=$(secs_to_ms "${code#* }")
  DETAIL="GET $1 → ${code%% *}"
  [ "${code%% *}" = 200 ]
}

run() { # name check-function [args]
  local name=$1 first_ms first_ok=0 warm_ok=0
  shift
  "$@" || first_ok=1
  first_ms=$LAT
  "$@" || warm_ok=1
  if [ "$first_ok" = 0 ] && [ "$warm_ok" = 0 ]; then
    printf '  ok    %-10s %6s ms %6s ms  %s\n' "$name" "$first_ms" "$LAT" "$DETAIL"
  else
    printf '  FAIL  %-10s %6s ms %6s ms  %s\n' "$name" "$first_ms" "$LAT" "$DETAIL"
    failures=$((failures + 1))
  fi
}

# ---- Request bodies ------------------------------------------------------------------------
# The fixture is a canonical WAV: 44-byte header, then PCM s16le mono 16 kHz.
if [ "$(dd if="$fixture" bs=1 skip=36 count=4 2>/dev/null)" != data ]; then
  echo "dev-smoke: $fixture is not a canonical 44-byte-header WAV" >&2
  exit 1
fi
STT_SECONDS=$(awk -v b="$(($(wc -c <"$fixture") - 44))" 'BEGIN { printf "%.1f", b / 32000 }')
{
  printf '{"dataframe_records":[{"audio_b64":"'
  tail -c +45 "$fixture" | base64 | tr -d '\n'
  printf '","encoding":"pcm_s16le","sample_rate":16000,"language":"en"}]}'
} >"$tmp/stt.json"
cat >"$tmp/tts.json" <<'JSON'
{"dataframe_records":[{"text":"Thanks for calling. Your table for two is booked for seven tonight.","voice":"af_heart","encoding":"pcm_s16le","sample_rate":24000}]}
JSON
jq -n --arg model "$VAK_DECISION_MODEL" '{
  model: $model,
  state: {
    plan_node: "take_booking",
    turns: [
      {speaker: "agent", text: "Would you like to book a table?"},
      {speaker: "caller", text: "No thanks, and please stop calling me."}
    ]
  },
  questions: {
    branch: {type: "choice", instructions: "Which step comes next?",
      criteria: {collect_details: null, confirm_booking: null, end_call: "the caller wants to end the call", none: null}},
    intent: {type: "choice", instructions: "What does the caller want?",
      criteria: {book: null, change: null, cancel: null, opt_out: "stop being contacted", other: null}},
    opt_out: {type: "noul", instructions: "Does the caller ask not to be contacted again?"},
    escalate: {type: "noul", instructions: "Does the caller ask for a human?"}
  }
}' >"$tmp/decision.json"
export FIRST="$tmp/llm.first"

# ---- Checks --------------------------------------------------------------------------------
echo "Local stack smoke test (first call, then warm)"
printf '  %-5s %-10s %9s %9s  %s\n' "" check first warm detail
run llm check_llm
run stt check_stt
run tts check_tts
run decision check_decision
run postgres check_postgres
run server check_http "http://$VAK_HTTP_ADDR/health"
run frontend check_http "http://127.0.0.1:5173/"

echo
if [ "$failures" -eq 0 ]; then
  echo "dev-smoke: every endpoint answered."
else
  echo "dev-smoke: $failures check(s) failed; \`just dev-doctor\` checks the setup."
  exit 1
fi
