#!/usr/bin/env bash
# Adds the Flatpak to a release packaging/release.sh has made: it is built after
# the other packages, around the Linux job's program, and the release does not wait for it.
set -euo pipefail
dir="$1"
tag="${2:-beta}"

shopt -s nullglob
built=("$dir"/*.flatpak)
[ ${#built[@]} -gt 0 ] || { echo "no Flatpak was built" >&2; exit 1; }

names=()
for file in "${built[@]}"; do names+=("$(basename "$file")"); done
mapfile -t kept < <(gh release view "$tag" --json assets --jq '.assets[].name | select(endswith(".flatpak"))')
gh release upload "$tag" "${built[@]}" --clobber
for name in "${kept[@]}"; do
    case " ${names[*]} " in
        *" $name "*) ;;
        *) gh release delete-asset "$tag" "$name" --yes ;;
    esac
done
