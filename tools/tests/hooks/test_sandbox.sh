# shellcheck shell=bash
# The test sandbox itself. lefthook's pre-push runs these tests through `just verify`, and git
# exports GIT_DIR, GIT_INDEX_FILE and more to its hooks, so a sandbox must never inherit them.

test_sandbox_ignores_the_git_environment_of_a_hook() {
  local outer="$T/outer" log="$T/inner.log"
  git init -q "$outer"
  git -C "$outer" commit -q --allow-empty -m "outer: before"
  local head_before
  head_before=$(git -C "$outer" rev-parse HEAD)

  # A hook's environment, aimed at the outer repository; then the usual sandbox and repo.
  if ! GIT_DIR="$outer/.git" GIT_INDEX_FILE="$outer/.git/index" \
    bash -c '. "$1/lib.sh"; sandbox; trap cleanup EXIT; make_repo' _ "$TESTS_DIR" >"$log" 2>&1; then
    fail "make_repo failed under a hook's git environment: $(cat "$log")"
  fi

  assert_eq "false" "$(git -C "$outer" config --get core.bare)" "core.bare of the outer repository"
  assert_eq "$head_before" "$(git -C "$outer" rev-parse HEAD)" "HEAD of the outer repository"
  assert_eq "" "$(git -C "$outer" remote)" "remotes of the outer repository"
}
