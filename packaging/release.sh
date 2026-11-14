#!/usr/bin/env bash
set -euo pipefail
dir="$1"
commit="$2"
tag="${3:-beta}"
short=${commit:0:7}

case "$tag" in
    beta)
        title="RimStack beta ($short)"
        kind=(--prerelease)
        ;;
    v*)
        version=$(grep -m1 -E '^version = "' Cargo.toml | cut -d'"' -f2)
        if [ "${tag#v}" != "$version" ]; then
            echo "$tag does not name the version in Cargo.toml, $version" >&2
            exit 1
        fi
        title="RimStack ${tag#v}"
        case "$tag" in
            *-*) kind=(--prerelease) ;;
            *) kind=(--prerelease=false --latest) ;;
        esac
        ;;
    *)
        echo "not a release tag: $tag" >&2
        exit 1
        ;;
esac

shopt -s nullglob
built=("$dir"/*)
fresh=$(mktemp)
trap 'rm -f "$fresh"' EXIT
[ ${#built[@]} -gt 0 ] || { echo "no package was built" >&2; exit 1; }

system_of() {
    case "$1" in
        *.AppImage | *.flatpak) echo linux ;;
        *-windows-*.zip | *-setup.exe) echo windows ;;
        *-macos-*.dmg | *-macos.dmg) echo macos ;;
        *) echo other ;;
    esac
}

if [ "$tag" = beta ]; then
    git tag -f "$tag" "$commit"
    git push -f origin "refs/tags/$tag"
else
    named=$(git ls-remote --tags origin "refs/tags/$tag" "refs/tags/$tag^{}" | sort -k2 | tail -1 | cut -f1)
    if [ -z "$named" ]; then
        git tag "$tag" "$commit"
        git push origin "refs/tags/$tag"
    elif [ "$named" != "$commit" ]; then
        echo "$tag already names $named, not $commit" >&2
        exit 1
    fi
fi

if ! gh release view "$tag" > /dev/null 2>&1; then
    gh release create "$tag" "${kind[@]}" --verify-tag --title "$title" --notes "Being built."
fi

mapfile -t kept < <(gh release view "$tag" --json assets --jq '.assets[].name')
for system in linux windows macos; do
    new=()
    for file in "${built[@]}"; do
        if [ "$(system_of "$file")" = "$system" ]; then new+=("$file"); fi
    done
    [ ${#new[@]} -gt 0 ] || continue
    for name in "${kept[@]}"; do
        if [ "$(system_of "$name")" = "$system" ]; then gh release delete-asset "$tag" "$name" --yes; fi
    done
    gh release upload "$tag" "${new[@]}" --clobber
    echo "$system $short" >> "$fresh"
done

before=$(gh release view "$tag" --json body --jq .body || true)
line_for() {
    local system=$1 from
    from=$(grep -E "^$system " "$fresh" | cut -d' ' -f2 || true)
    if [ -n "$from" ]; then
        echo "- $system: built from $from"
    else
        from=$(grep -oE "^- $system: built from [0-9a-f]+" <<< "$before" | grep -oE '[0-9a-f]+$' || true)
        echo "- $system: ${from:+built from $from, }not rebuilt from $short (its build failed)"
    fi
}
changes=$(packaging/changes.sh "$commit" "$tag")
notes="${changes:+$changes

}Built at $short on $(date -u '+%Y-%m-%d %H:%M UTC').

$(line_for linux)
$(line_for windows)
$(line_for macos)

The programs are not signed yet, so Windows and macOS warn the first time. The files are fine; this gets past it once.

Windows: SmartScreen says \"Windows protected your PC\": choose More info, then Run anyway. For the zip, right-click it, choose Properties, tick Unblock, then extract. If Smart App Control is on (Windows 11), it blocks unsigned programs with no way past it except turning Smart App Control off.
macOS: drag RimStack to Applications and open it once. Then open System Settings, Privacy & Security, scroll down and choose Open Anyway next to RimStack. If macOS says the app is damaged, run this in Terminal: \`xattr -dr com.apple.quarantine /Applications/RimStack.app\`
Linux: make the AppImage executable (\`chmod +x\`) and run it, or install the Flatpak with \`flatpak install RimStack-*.flatpak\`."
gh release edit "$tag" "${kind[@]}" --title "$title" --notes "$notes"
