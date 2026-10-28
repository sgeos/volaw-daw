#!/usr/bin/env bash
# The local gate. Continuous integration runs this same script, so a green run here
# means what a green run there means, apart from operating system and tool versions.
#
# It exits zero only when every check passes. Each check prints one line on success
# and an explanation on failure, and the script stops at the first failure.
#
# Repository hygiene runs first and needs git, python3 and shellcheck. Add the
# project's own checks in the marked section below, such as lint, format, type and test
# commands, in the order that fails fastest.
#
# Usage:
#   scripts/check.sh                       # hygiene, then the project's checks
#   scripts/check.sh --hygiene-only        # hygiene alone, for a cheap separate CI job
#   scripts/check.sh [--hygiene-only] -- ARGUMENTS...
#
# Everything after `--` is kept in the project_args array for the project checks, such
# as test markers for a test runner. Any other argument is a usage error, so a typo
# never passes silently.

set -euo pipefail

usage() {
    printf 'usage: %s [--hygiene-only] [-- ARGUMENTS...]\n' "$0" >&2
    exit 2
}

hygiene_only=false
project_args=()
while (($# > 0)); do
    case "$1" in
        --hygiene-only) hygiene_only=true; shift ;;
        --) shift; project_args=("$@"); break ;;
        *) usage ;;
    esac
done

cd "$(git rev-parse --show-toplevel)"

fail() {
    printf 'check.sh: FAIL: %s\n' "$1" >&2
    exit 1
}

pass() {
    printf 'check.sh: ok: %s\n' "$1"
}

# --- Repository hygiene -------------------------------------------------------------

# secret/ is ignored whole. A tracked file there was added deliberately with
# `git add -f` or before the ignore rule existed, and either way it is a disclosure.
tracked_secret="$(git ls-files -- secret)"
if [[ -n "$tracked_secret" ]]; then
    fail "files under secret/ are tracked and must be removed from the index and history:
$tracked_secret"
fi
pass "nothing under secret/ is tracked"

# tmp/ is scratch. Only its marker is tracked, so the directory exists in a clone.
tracked_tmp="$(git ls-files -- tmp | grep -vx 'tmp/.gitkeep' || true)"
if [[ -n "$tracked_tmp" ]]; then
    fail "files under tmp/ other than tmp/.gitkeep are tracked:
$tracked_tmp"
fi
if [[ -z "$(git ls-files -- tmp/.gitkeep)" ]]; then
    fail "tmp/.gitkeep is not tracked, so tmp/ will not exist in a fresh clone"
fi
pass "only tmp/.gitkeep is tracked under tmp/"

# Unresolved merge markers in tracked text files. Binary files are skipped by -I. Only
# the opening and closing markers are matched, because a line of exactly seven equals
# signs is also a valid Markdown setext heading underline.
if git grep -nI -E '^(<<<<<<< |>>>>>>> )' -- . ':!scripts/check.sh' >/dev/null; then
    git grep -nI -E '^(<<<<<<< |>>>>>>> )' -- . ':!scripts/check.sh' >&2
    fail "unresolved merge markers are present"
fi
pass "no unresolved merge markers"

# A repository made from the template must not keep the template's README title or CI
# badge. The badge would show the template's status as if it were the project's, and
# the title would name the wrong project. These are steps 1 and 2 of the README's new
# project checklist. The repository's name
# is taken from the origin remote, so this check is skipped when origin is missing or
# is not on GitHub, such as a clone of a local path.
origin="$(git remote get-url origin 2>/dev/null || true)"
repository=""
if [[ "$origin" =~ github\.com[:/][^/]+/([^/]+)$ ]]; then
    repository="${BASH_REMATCH[1]%.git}"
fi
if [[ -n "$repository" && "$repository" != "project-template" ]] &&
    grep -q 'sgeos/project-template/actions' README.md 2>/dev/null; then
    fail "README.md still shows the template's CI badge. Point it at sgeos/$repository"
fi
if [[ -n "$repository" && "$repository" != "project-template" ]] &&
    grep -qx '# Project Template' README.md 2>/dev/null; then
    fail "README.md still has the template's title. Replace it with the project's name"
fi
if [[ -n "$repository" ]]; then
    pass "the README's title and CI badge are not the template's, or this is the template"
else
    printf 'check.sh: note: origin is not a GitHub repository, so the README title and CI badge were not checked\n'
fi

# Relative links in tracked Markdown files must resolve to a tracked or present path.
# Both inline links, `[text](target)`, and reference definitions, `[name]: target`, are
# checked. Footnote definitions, `[^1]: text`, are not links. External links, mail links and links to an anchor in the same file are not checked,
# and neither are anchors within another file. Links inside fenced code blocks and
# inline code spans are skipped. This needs python3, which GitHub's runners and macOS
# both provide. The program is passed with -c rather than a heredoc, because some agent
# sandboxes forbid the temporary file that bash writes for a heredoc.
python3 -I -c '
import pathlib
import re
import subprocess
import sys

files = subprocess.run(
    ["git", "ls-files", "-z", "--", "*.md"], check=True, capture_output=True
).stdout.split(b"\0")
link = re.compile(r"\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
definition = re.compile(r"^ {0,3}\[(?!\^)[^\]]+\]:\s*(<[^>]*>|\S+)")
# A fence opens with three or more backticks or tildes, and closes only with a run of
# the same character that is at least as long, so a ``` line inside a ~~~ fence does
# not close it. Inline code spans are removed before links are matched.
fence_line = re.compile(r"^ {0,3}(`{3,}|~{3,})(.*)$")
code_span = re.compile(r"(`+)(?:(?!\1).)+?\1")
broken = []
for raw in filter(None, files):
    path = pathlib.Path(raw.decode())
    fence = None
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        opener = fence_line.match(line)
        if fence is None and opener:
            fence = opener.group(1)
            continue
        if fence is not None:
            closer = fence_line.match(line)
            if closer and closer.group(1)[0] == fence[0] and len(closer.group(1)) >= len(fence) and not closer.group(2).strip():
                fence = None
            continue
        text = code_span.sub("", line)
        targets = link.findall(text) + [m.group(1) for m in [definition.match(text)] if m]
        for target in targets:
            if target.startswith(("http://", "https://", "mailto:", "#")):
                continue
            target = target.split("#", 1)[0].strip("<>")
            # GitHub resolves a link that starts with a slash from the repository root.
            base = pathlib.Path(".") if target.startswith("/") else path.parent
            if target and not (base / target.lstrip("/")).exists():
                broken.append(f"{path}:{number}: {target}")
for entry in broken:
    print(entry, file=sys.stderr)
sys.exit(1 if broken else 0)
' || fail "broken relative links in Markdown, listed above"
pass "relative Markdown links resolve"

# Text and file hygiene, matching .editorconfig and .gitattributes. Every tracked text
# file ends with a newline and has no carriage returns, except Windows batch files,
# which need them, and files other than Markdown have no trailing whitespace, since
# Markdown uses it for line breaks. A file whose whitespace attribute is unset in
# .gitattributes, with `-whitespace`, is exempt from the trailing whitespace check, for
# fixtures that must stay byte for byte. No tracked file exceeds MAX_FILE_BYTES, which
# defaults to 5 MiB, so a large binary is added deliberately or not at all. A file is
# text when its first 8 KiB holds no NUL byte, the same test git uses.
#
# This step also finds the shell scripts for the shellcheck step below, and requires
# them to be executable. A shell script is a tracked file named *.sh, a file under
# scripts/hooks/, or any file whose first line is a sh, bash, dash or ksh shebang,
# which covers scripts without an extension. It prints their paths, one per line.
if ! shell_list="$(MAX_FILE_BYTES="${MAX_FILE_BYTES:-5242880}" python3 -I -c '
import os
import re
import subprocess
import sys

shebang = re.compile(rb"^#!\s*(?:\S*/)?(?:env\s+(?:-S\s+)?)?(?:sh|bash|dash|ksh)(?:\s|$)")

limit = int(os.environ["MAX_FILE_BYTES"])
listing = subprocess.run(
    ["git", "ls-files", "-s", "-z"], check=True, capture_output=True
).stdout.split(b"\0")
paths = [entry.split(b"\t", 1)[1] for entry in filter(None, listing)]
attributes = subprocess.run(
    ["git", "check-attr", "-z", "--stdin", "whitespace"],
    input=b"\0".join(paths), check=True, capture_output=True,
).stdout.split(b"\0")
verbatim = {
    attributes[i].decode()
    for i in range(0, len(attributes) - 2, 3)
    if attributes[i + 2] == b"unset"
}
problems = []
shells = []
for entry in filter(None, listing):
    meta, raw_path = entry.split(b"\t", 1)
    mode = meta.split()[0].decode()
    path = raw_path.decode()
    if mode == "120000" or not os.path.isfile(path):
        continue
    size = os.path.getsize(path)
    if size > limit:
        problems.append(f"{path}: {size} bytes, above the limit of {limit}")
    with open(path, "rb") as handle:
        data = handle.read()
    first_line = data.split(b"\n", 1)[0]
    if path.endswith(".sh") or path.startswith("scripts/hooks/") or shebang.match(first_line):
        shells.append(path)
        if mode != "100755":
            problems.append(f"{path}: not executable, fix with git update-index --chmod=+x")
    if b"\0" in data[:8192] or not data:
        continue
    if b"\r" in data and not path.lower().endswith((".bat", ".cmd")):
        problems.append(f"{path}: contains carriage returns")
    if not data.endswith(b"\n"):
        problems.append(f"{path}: does not end with a newline")
    if not path.endswith(".md") and path not in verbatim:
        for number, line in enumerate(data.split(b"\n"), 1):
            if line.rstrip(b" \t") != line:
                problems.append(f"{path}:{number}: trailing whitespace")
for problem in problems:
    print(problem, file=sys.stderr)
for path in shells:
    print(path)
sys.exit(1 if problems else 0)
')"; then
    fail "text or file hygiene problems, listed above"
fi
pass "tracked files end with newlines, have no stray whitespace, and are within size"

# Shell scripts and hooks pass shellcheck. A missing shellcheck is a failure, not a
# skip, so that a green gate never means "not checked". GitHub's Ubuntu runners include
# it. Elsewhere, install it with the system package manager, such as
# `sudo port install shellcheck`, `brew install shellcheck` or `apt install shellcheck`.
# The list comes from the hygiene step above. A read loop rather than mapfile, because
# macOS ships bash 3.2, which lacks mapfile. A path containing a newline is not
# supported.
shell_files=()
while IFS= read -r file; do
    [[ -n "$file" ]] && shell_files+=("$file")
done < <(printf '%s\n' "$shell_list")
if ((${#shell_files[@]} > 0)); then
    command -v shellcheck >/dev/null ||
        fail "shellcheck is not installed, so ${#shell_files[@]} shell files went unchecked"
    shellcheck -- "${shell_files[@]}" || fail "shellcheck reported problems, listed above"
fi
noun="files"
((${#shell_files[@]} == 1)) && noun="file"
pass "shellcheck is clean on ${#shell_files[@]} shell $noun"

# Optional disclosure scan. If secret/disclosure-terms.txt exists, no tracked file may
# contain any of its terms, matched as fixed strings without regard to case. Blank
# lines and lines starting with # are ignored. The scan prints only the file and line
# of each match, never the matched text, so that the gate's own output does not
# disclose a term. The list lives under secret/, so the scan cannot run in a clone or
# in continuous integration, and it says so rather than passing silently. A line that
# starts with `:!` is not a term but a git pathspec to exclude, such as `:!uv.lock`
# for a generated lockfile whose dependency names would otherwise match.
terms_file="secret/disclosure-terms.txt"
if [[ -f "$terms_file" ]]; then
    patterns=()
    exclusions=()
    while IFS= read -r term || [[ -n "$term" ]]; do
        [[ "$term" =~ ^[[:space:]]*(#|$) ]] && continue
        # Trim surrounding whitespace, so that a term saved with a trailing space still
        # matches the word at the end of a line.
        term="${term#"${term%%[![:space:]]*}"}"
        term="${term%"${term##*[![:space:]]}"}"
        if [[ "$term" == :!* ]]; then
            exclusions+=("$term")
        else
            patterns+=(-e "$term")
        fi
    done < "$terms_file"
    if ((${#patterns[@]} > 0)); then
        # git grep exits 0 on a match and 1 on none. Any other status is an error, such as
        # a bad pathspec, and the scan fails closed rather than reporting it as clean.
        if raw="$(git grep -n -I -i -F "${patterns[@]}" -- . "${exclusions[@]+"${exclusions[@]}"}")"; then
            grep_status=0
        else
            grep_status=$?
        fi
        case "$grep_status" in
            0) hits="$(printf '%s\n' "$raw" | cut -d: -f1,2)" ;;
            1) hits="" ;;
            *) fail "git grep exited with status $grep_status, so the disclosure scan could not run" ;;
        esac
        if [[ -n "$hits" ]]; then
            printf '%s\n' "$hits" >&2
            fail "tracked files contain terms from $terms_file, at the lines listed above"
        fi
        excluded="${#exclusions[@]} excluded paths"
        ((${#exclusions[@]} == 1)) && excluded="1 excluded path"
        pass "disclosure scan found no listed term in tracked files, with $excluded"
    else
        printf 'check.sh: note: %s lists no terms, so the disclosure scan did nothing\n' "$terms_file"
    fi
else
    printf 'check.sh: note: no %s, so the disclosure scan was skipped\n' "$terms_file"
fi

if [[ "$hygiene_only" == true ]]; then
    printf 'check.sh: note: --hygiene-only, so the project checks were not run\n'
    exit 0
fi

# --- Project checks -----------------------------------------------------------------
#
# Add the project's commands here. Each should exit non-zero on failure, which stops
# the script through `set -e`. Until any are added, the gate covers hygiene only, and
# it says so rather than implying more. Pass the arguments given after `--` with
# "${project_args[@]+"${project_args[@]}"}", the form that macOS's bash 3.2 accepts for
# an empty array under `set -u`. For example:
#
#   uv run pytest "${project_args[@]+"${project_args[@]}"}"
#
# The Rust workspace. Formatting fails fastest, then Clippy, which type checks every
# target without code generation. The build keeps the non-test daw binary covered,
# because cargo test compiles the binary only under the test configuration. Arguments
# after -- reach cargo test, such as a test name filter.
#
# Inside a sandbox that denies the sccache socket, run the gate with RUSTC_WRAPPER set
# to empty, which bypasses sccache without changing what is checked.

cargo fmt --all --check || fail "cargo fmt found unformatted code"
pass "cargo fmt is clean"
cargo clippy --workspace --all-targets --locked -- -D warnings ||
    fail "cargo clippy reported warnings or errors"
pass "cargo clippy is clean with warnings denied"
cargo build --workspace --all-targets --locked || fail "cargo build failed"
pass "cargo build succeeded for every target"
cargo test --workspace --locked -- "${project_args[@]+"${project_args[@]}"}" ||
    fail "cargo test failed"
pass "cargo test passed, see the counts above"
