# Gate self-tests, imported by the root justfile. Each canary makes one deliberate violation in
# a temporary worktree and checks that its gate fails. See docs/ci.md, "Gate self-tests".
# Too slow for `just verify`: the nightly workflow runs them.

# Run the gate canaries, or only the named ones (`just canaries --list` lists them)
[positional-arguments]
canaries *names:
    shellcheck -x --source-path=SCRIPTDIR tools/canaries/run.sh tools/canaries/*/apply.sh
    tools/canaries/run.sh "$@"
