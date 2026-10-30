# shellcheck shell=bash
# `just task <name>` and `just task-done <name>` (tools/repo.just), run against a fixture
# repo with a local origin. Uses the real just; prints a note and passes without it.

task_just() { # recipe args...
  local just
  just=$(find_on_orig_path just)
  "$just" --justfile "$REPO_ROOT/tools/repo.just" --working-directory "$R" "$@"
}

have_just() {
  [ -n "$(find_on_orig_path just)" ] || {
    echo "just is not installed; skipped"
    return 1
  }
}

test_task_creates_a_worktree_on_a_task_branch_from_origin_main() {
  have_just || return 0
  make_repo
  git -C "$R" checkout -q main
  task_just task demo >/dev/null
  local worktree="$R/.claude/worktrees/demo"
  assert_file "$worktree/README.md"
  assert_eq "task/demo" "$(git -C "$worktree" rev-parse --abbrev-ref HEAD)"
  assert_eq "$(git -C "$R" rev-parse origin/main)" "$(git -C "$worktree" rev-parse HEAD)"
  assert_eq "" "$(git -C "$worktree" config --get branch.task/demo.merge || true)" "no upstream to main"
}

test_task_done_removes_the_worktree_and_keeps_the_branch() {
  have_just || return 0
  make_repo
  task_just task demo >/dev/null
  task_just task-done demo >/dev/null
  assert_no_file "$R/.claude/worktrees/demo"
  git -C "$R" rev-parse -q --verify refs/heads/task/demo >/dev/null || fail "branch task/demo is gone"
}

test_task_from_inside_a_worktree_creates_a_sibling() {
  have_just || return 0
  make_repo
  task_just task first >/dev/null
  local just
  just=$(find_on_orig_path just)
  "$just" --justfile "$REPO_ROOT/tools/repo.just" --working-directory "$R/.claude/worktrees/first" \
    task second >/dev/null
  assert_file "$R/.claude/worktrees/second/README.md"
}

test_task_rejects_unsafe_names() {
  have_just || return 0
  make_repo
  if task_just task "../escape" >/dev/null 2>&1; then
    fail "a name with ../ was accepted"
  fi
  assert_no_file "$T/escape"
}
