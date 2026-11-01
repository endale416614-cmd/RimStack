#!/usr/bin/env bash
# What a release adds: a line per commit since the release before it that changed the program
# (crates/ or packaging/), its subject up to the first ": ", where the commit's own detail starts.
# Version commits, CI, dependency locks and the README or site alone are left out.
set -euo pipefail
commit="$1"
tag="${2:-beta}"
since=$(git describe --tags --abbrev=0 --match 'v[0-9]*' --exclude "$tag" "$commit^" 2> /dev/null || true)
[ -n "$since" ] || exit 0
lines=()
while read -r c; do
    git diff-tree --no-commit-id --name-only -r "$c" | grep -qE '^(crates|packaging)/' || continue
    subject=$(git log -1 --format=%s "$c")
    lines+=("- ${subject%%: *}")
done < <(git log --no-merges --format=%h --invert-grep --grep='^RimStack [0-9]' "$since..$commit")
[ ${#lines[@]} -gt 0 ] || exit 0
echo "Changes since $since:"
echo
printf '%s\n' "${lines[@]}"
