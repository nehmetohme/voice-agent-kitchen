# shellcheck shell=bash
# Static checks of the gate configuration: the reviewer agents, the verdict schema and
# .claude/settings.json agree with each other and with stop-gate.sh.

CLAUDE_DIR="$REPO_ROOT/.claude"

front_matter() { # file key : value from the YAML front matter
  awk -v key="$2" '/^---[[:space:]]*$/ { n++; next }
    n == 1 && index($0, key ":") == 1 { sub(/^[^:]*:[[:space:]]*/, ""); print; exit }' "$1"
}

gate_reviewers() {
  sed -n 's/^ALL_REVIEWERS="\(.*\)"$/\1/p' "$HOOKS_DIR/stop-gate.sh"
}

test_config_every_gate_reviewer_has_a_read_only_agent() {
  local name file count=0
  for name in $(gate_reviewers); do
    count=$((count + 1))
    file="$CLAUDE_DIR/agents/$name.md"
    assert_file "$file"
    assert_eq "$name" "$(front_matter "$file" name)" "$name: name"
    assert_contains "$(front_matter "$file" description)" "verdict JSON" "$name: description"
    assert_eq "Read, Grep, Glob, Bash" "$(front_matter "$file" tools)" "$name: tools"
    assert_eq "Edit, Write, NotebookEdit, Agent" "$(front_matter "$file" disallowedTools)" "$name: disallowedTools"
    assert_eq "sonnet" "$(front_matter "$file" model)" "$name: model"
    case "$(front_matter "$file" maxTurns)" in
      [1-9] | [1-9][0-9]) ;;
      *) fail "$name: maxTurns must be a number" ;;
    esac
    assert_contains "$(cat "$file")" '"verdict"' "$name: output contract"
  done
  assert_eq 14 "$count" "fourteen reviewers"
}

test_config_no_agent_outside_the_gate() {
  local file name
  for file in "$CLAUDE_DIR"/agents/*.md; do
    name=$(basename "$file" .md)
    assert_contains " $(gate_reviewers) " " $name " "agent $name is not in stop-gate.sh ALL_REVIEWERS"
  done
}

test_config_ux_reviewers_read_their_guardrail_checklist() {
  local group
  for group in cognitive interaction perception memory complexity; do
    assert_contains "$(cat "$CLAUDE_DIR/agents/ux-$group.md")" "docs/ux-guardrails/$group.md"
    assert_contains "$(cat "$CLAUDE_DIR/agents/ux-$group.md")" "auditScreen"
  done
}

test_config_verdict_schema_is_valid_json() {
  assert_eq '["findings","verdict"]' \
    "$(jq -c '.required | sort' "$CLAUDE_DIR/schemas/review-verdict.json")"
  assert_eq '["fail","pass"]' \
    "$(jq -c '.properties.verdict.enum | sort' "$CLAUDE_DIR/schemas/review-verdict.json")"
}

test_config_settings_wire_executable_hooks() {
  local settings="$CLAUDE_DIR/settings.json" script
  assert_eq "Edit|Write" "$(jq -r '.hooks.PreToolUse[0].matcher' "$settings")"
  assert_contains "$(jq -r '.hooks.PreToolUse[0].hooks[0].command' "$settings")" "pre-edit-guard.sh"
  assert_eq "post-edit-lint.sh false" \
    "$(jq -r '.hooks.PostToolUse[0].hooks[0] | "\(.command | split("/") | last) \(.async // false)"' "$settings")"
  assert_eq "graph-update.sh true" \
    "$(jq -r '.hooks.PostToolUse[1].hooks[0] | "\(.command | split("/") | last) \(.async // false)"' "$settings")"
  assert_contains "$(jq -r '.hooks.Stop[0].hooks[0].command' "$settings")" "stop-gate.sh"
  [ "$(jq -r '.hooks.Stop[0].hooks[0].timeout' "$settings")" -gt 1700 ] \
    || fail "the Stop hook timeout must exceed the gate's 1700 s deadline"
  for script in $(jq -r '.. | .command? // empty' "$settings" | sed 's#.*/\.claude/hooks/##'); do
    [ -x "$HOOKS_DIR/$script" ] || fail "hook script not executable: $script"
  done
}

test_config_settings_deny_tags_and_generated_edits() {
  local deny
  deny=$(jq -r '.permissions.deny[]' "$CLAUDE_DIR/settings.json")
  assert_contains "$deny" "Bash(git tag *)"
  assert_contains "$deny" "Bash(git push --tags *)"
  assert_contains "$deny" "Bash(git push * --tags)"
  assert_contains "$deny" "Bash(git push * refs/tags/*)"
  assert_contains "$deny" "Edit(/contracts/openapi.json)"
  assert_contains "$deny" "Edit(/frontend/src/shared/api/generated/**)"
  assert_contains "$deny" "Edit(/frontend/src/app/routeTree.gen.ts)"
  assert_contains "$deny" "Edit(/frontend/src/shared/ui/tokens.css)"
  assert_contains "$deny" "Edit(/backend/**/.sqlx/**)"
}

test_config_rules_are_path_scoped() {
  local rule
  for rule in backend-layers frontend-layers contracts deploy; do
    assert_file "$CLAUDE_DIR/rules/$rule.md"
    assert_eq "paths:" "$(sed -n '2p' "$CLAUDE_DIR/rules/$rule.md")" "$rule.md starts with paths: front matter"
  done
}
