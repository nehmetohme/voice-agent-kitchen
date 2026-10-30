"""Print the signed contract tag that pins a file, or nothing.

Usage: python3 -I contract_tags.py <repo-root> <repo-relative-path> <tag>...

Tags look like contract/<feature>/v<N>. A file is pinned when:
  1. the tag for the feature's *current* version exists. The current version comes from
     the working tree's contracts/contracts.toml, or is the highest tagged version when
     the manifest doesn't list the feature. A bumped, not yet signed version pins nothing,
     so a contract can be changed by bumping its version first.
  2. the file exists in that tag's tree and a path (git-style glob) listed for the feature
     in the tag's own contracts/contracts.toml matches it.

Works on Python 3.9+: uses tomllib, then tomli, then a small parser for the manifest
shape (VAK_HOOK_TOML=mini forces the small parser).
"""

import os
import re
import subprocess
import sys

TAG = re.compile(r"contract/(.+)/v([0-9]+)")


def git(root, *args):
    result = subprocess.run(
        ["git", "-C", root, *args], capture_output=True, text=True, check=False
    )
    return result.stdout if result.returncode == 0 else ""


def parse_toml(text):
    if not text:
        return {}
    if os.environ.get("VAK_HOOK_TOML") != "mini":
        for module in ("tomllib", "tomli"):
            try:
                return __import__(module).loads(text)
            except ImportError:
                continue
            except Exception:
                return {}
    return mini_toml(text)


def strip_comment(line):
    quoted = False
    for i, ch in enumerate(line):
        if ch == '"':
            quoted = not quoted
        elif ch == "#" and not quoted:
            return line[:i]
    return line


def mini_value(text):
    text = text.strip()
    if text.startswith("["):
        return re.findall(r'"((?:[^"\\]|\\.)*)"', text)
    if text.startswith('"') and text.endswith('"'):
        return text[1:-1]
    if re.fullmatch(r"[+-]?[0-9]+", text):
        return int(text)
    return text


def mini_toml(text):
    """Just enough TOML for contracts.toml: [a.b] tables, ints, strings, string arrays."""
    data = {}
    table = data
    key, pending = None, ""
    for raw in text.splitlines():
        line = strip_comment(raw).strip()
        if pending:
            pending += " " + line
            if pending.count("[") <= pending.count("]"):
                table[key] = mini_value(pending)
                pending = ""
            continue
        if not line:
            continue
        header = re.fullmatch(r"\[([^\[\]]+)\]", line)
        if header:
            table = data
            for part in header.group(1).split("."):
                table = table.setdefault(part.strip().strip('"'), {})
            continue
        if "=" in line:
            key, value = (s.strip() for s in line.split("=", 1))
            key = key.strip('"')
            if value.startswith("[") and value.count("[") > value.count("]"):
                pending = value
            else:
                table[key] = mini_value(value)
    return data


def glob_regex(pattern):
    """Git-style glob: `**/` any folders, `/**` everything below, `*` and `?` within a
    segment. Like a pathspec, a folder pattern also covers the files under it."""
    out, i = "", 0
    while i < len(pattern):
        if pattern.startswith("**/", i):
            out, i = out + "(?:.*/)?", i + 3
        elif pattern.startswith("**", i):
            out, i = out + ".*", i + 2
        elif pattern[i] == "*":
            out, i = out + "[^/]*", i + 1
        elif pattern[i] == "?":
            out, i = out + "[^/]", i + 1
        else:
            out, i = out + re.escape(pattern[i]), i + 1
    return re.compile(out + "(?:/.*)?")


def features_of(text):
    features = parse_toml(text).get("features")
    return features if isinstance(features, dict) else {}


def main():
    root, rel, tags = sys.argv[1], sys.argv[2], sys.argv[3:]
    tagged = {}
    for tag in tags:
        match = TAG.fullmatch(tag)
        if match:
            tagged.setdefault(match.group(1), set()).add(int(match.group(2)))
    if not tagged:
        return

    manifest_path = os.path.join(root, "contracts", "contracts.toml")
    manifest = ""
    if os.path.isfile(manifest_path):
        with open(manifest_path, encoding="utf-8") as handle:
            manifest = handle.read()
    current = features_of(manifest)

    candidates = []
    for feature, versions in sorted(tagged.items()):
        entry = current.get(feature)
        entry = entry if isinstance(entry, dict) else {}
        version = entry.get("version")
        if not isinstance(version, int):
            version = max(versions)
        if version in versions:
            candidates.append((feature, f"contract/{feature}/v{version}", entry))
    if not candidates:
        return

    # One git call answers "which current tags contain this file at all?".
    query = "".join(f"refs/tags/{tag}:{rel}\n" for _, tag, _ in candidates)
    result = subprocess.run(
        ["git", "-C", root, "cat-file", "--batch-check"],
        input=query, capture_output=True, text=True, check=False,
    )
    present = [not line.endswith(" missing") for line in result.stdout.splitlines()]

    for (feature, tag, entry), exists in zip(candidates, present):
        if not exists:
            continue
        tagged_entry = features_of(git(root, "show", f"refs/tags/{tag}:contracts/contracts.toml")).get(feature)
        tagged_entry = tagged_entry if isinstance(tagged_entry, dict) else {}
        paths = tagged_entry.get("paths") or entry.get("paths") or []
        paths = [p for p in paths if isinstance(p, str) and p]
        # The file exists in the tag, so it is pinned if a manifest path matches it.
        # (git ls-tree rejects :(glob) pathspecs, so match here.)
        if any(glob_regex(p.lstrip("/")).fullmatch(rel) for p in paths):
            print(tag)
            return


if __name__ == "__main__":
    main()
