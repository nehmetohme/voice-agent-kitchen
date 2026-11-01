#!/usr/bin/env bash
# Download the local dev models into VAK_MODELS_DIR (default ~/.cache/vak/models), outside the
# repo, so every worktree shares one copy. Each file is pinned to a Hugging Face commit and
# verified by size and SHA-256; a verified file is skipped on the next run (idempotent).
#
#   deploy/local/models.sh            download what's missing (`just models-pull`)
#   deploy/local/models.sh --verify   also re-hash files that were verified before
#   deploy/local/models.sh --check    download nothing; exit 1 if a file is missing or unverified
#
# Layout: $VAK_MODELS_DIR/<org>/<repo>/<file>, so a model's local path is
# $VAK_MODELS_DIR/<org>/<repo> (mlx-audio takes it as the model name).
set -euo pipefail

models_dir=${VAK_MODELS_DIR:-$HOME/.cache/vak/models}
mode=${1:-pull}

# repo  revision  file  bytes  sha256
# - STT: Parakeet TDT 0.6B v2 (CC-BY-4.0), MLX build.
# - TTS: Kokoro-82M (Apache-2.0), MLX bf16 build, with two voices (US female, US male).
# - Decisions: Kev-4B Q4_K_M GGUF (Apache-2.0) for llama-server's /v1/systemone.
manifest='
mlx-community/parakeet-tdt-0.6b-v2 8ae155301e23d820d82aa60d24817c900e69e487 config.json 36176 9bd323e60afe2615c983a5d9fc3a2c0470df2a03edf90c0f861bd59509d07264
mlx-community/parakeet-tdt-0.6b-v2 8ae155301e23d820d82aa60d24817c900e69e487 model.safetensors 2471559904 b958c37a6baa6874a279108755c8f2818e27bf647d72d54800a234a421341dfe
mlx-community/Kokoro-82M-bf16 a71e4d38b236d968966a2002c4c895dbd12b1c3c config.json 2351 5abb01e2403b072bf03d04fde160443e209d7a0dad49a423be15196b9b43c17f
mlx-community/Kokoro-82M-bf16 a71e4d38b236d968966a2002c4c895dbd12b1c3c kokoro-v1_0.safetensors 327115152 4e9ecdf03b8b6cf906070390237feda473dc13327cb8d56a43deaa374c02acd8
mlx-community/Kokoro-82M-bf16 a71e4d38b236d968966a2002c4c895dbd12b1c3c voices/af_heart.safetensors 522320 2c1c733b0e6576c810e268d3e440c21dea4e0f0131a3ba4cfc98d7fe6136d094
mlx-community/Kokoro-82M-bf16 a71e4d38b236d968966a2002c4c895dbd12b1c3c voices/am_michael.safetensors 522320 3940147ded35deba0bb52e8132f89b719298e0520258c34584358aa5a24da2ea
ggml-org/Kev-4B-GGUF f4624b6bb7072f1bab726a622c1cad3f96030b9e Kev-4B-Q4_K_M.gguf 3033489824 33ae6b18926502b2209a1bf7d3b61a350d65938441515c686bb87d226a19eff9
'

die() {
  echo "models: $*" >&2
  exit 1
}

case "$mode" in
  pull | --verify | --check) ;;
  *) die "unknown argument '$mode' (use --verify or --check)" ;;
esac

file_size() { stat -f %z "$1" 2>/dev/null || stat -c %s "$1"; }
sha256_of() { shasum -a 256 "$1" | cut -d' ' -f1; }

# A file counts as verified when its size matches and its .sha256 stamp holds the pinned hash.
is_verified() { # dest bytes sha
  [ -f "$1" ] && [ "$(file_size "$1")" = "$2" ] && [ -f "$1.sha256" ] && [ "$(cat "$1.sha256")" = "$3" ]
}

verify_into_place() { # part dest bytes sha
  local actual
  actual=$(file_size "$1")
  [ "$actual" = "$3" ] || {
    rm -f "$1"
    die "$2: size $actual, expected $3 (partial download removed; run again)"
  }
  actual=$(sha256_of "$1")
  [ "$actual" = "$4" ] || {
    rm -f "$1"
    die "$2: SHA-256 $actual, expected $4 (download removed)"
  }
  mv -f "$1" "$2"
  printf '%s' "$4" >"$2.sha256"
}

missing=0
total=0
while read -r repo revision file bytes sha; do
  [ -n "$repo" ] || continue
  # openjev/openjev is CC-BY-NC (non-commercial): never fetch it (docs/research/08 §5).
  case "$repo" in openjev/*) die "refusing $repo: non-commercial licence" ;; esac
  dest="$models_dir/$repo/$file"
  total=$((total + bytes))

  if [ "$mode" = --verify ] && [ -f "$dest" ]; then
    echo "models: re-hashing $repo/$file"
    rm -f "$dest.sha256"
    mv -f "$dest" "$dest.part"
    verify_into_place "$dest.part" "$dest" "$bytes" "$sha"
    continue
  fi
  if is_verified "$dest" "$bytes" "$sha"; then
    echo "models: ok       $repo/$file"
    continue
  fi
  if [ "$mode" = --check ]; then
    echo "models: missing  $repo/$file" >&2
    missing=$((missing + 1))
    continue
  fi

  mkdir -p "$(dirname "$dest")"
  if [ -f "$dest" ] && [ "$(file_size "$dest")" = "$bytes" ]; then
    # Present but never stamped (e.g. copied in by hand): verify it instead of downloading.
    mv -f "$dest" "$dest.part"
  else
    echo "models: fetching $repo/$file ($((bytes / 1048576)) MiB)"
    progress=(-sS)
    [ -t 2 ] && progress=(--progress-bar)
    # -C -: resume a .part left by an interrupted run. Timeouts: 20 s to connect; abort if the
    # transfer stays under 10 KB/s for 60 s.
    curl -fL "${progress[@]}" --retry 3 --retry-delay 2 -C - \
      --connect-timeout 20 --speed-limit 10240 --speed-time 60 \
      -o "$dest.part" "https://huggingface.co/$repo/resolve/$revision/$file"
  fi
  verify_into_place "$dest.part" "$dest" "$bytes" "$sha"
  echo "models: verified $repo/$file"
done <<<"$manifest"

if [ "$missing" -gt 0 ]; then
  die "$missing file(s) missing or unverified in $models_dir: run \`just models-pull\`"
fi
echo "models: all present in $models_dir ($((total / 1048576)) MiB)"
