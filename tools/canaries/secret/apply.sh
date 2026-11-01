#!/usr/bin/env bash
# Runs from the canary worktree's root. Writes a config file holding a made-up AWS access key
# ID (AKIA + 16 base-32 characters). The key is assembled here at run time, so no file in this
# repository holds it and gitleaks never flags the canary itself. AWS's documented example key
# (…EXAMPLE) would prove nothing: gitleaks allowlists IDs that end in EXAMPLE.
set -euo pipefail
key_id="AKIA""LALEMEL33243OLIA"
mkdir -p deploy
cat >deploy/aws-credentials.toml <<EOF
[aws]
region = "eu-west-1"
access_key_id = "$key_id"
EOF
