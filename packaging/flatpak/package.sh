#!/usr/bin/env bash
# RimStack as one .flatpak file in target/packages.
# Needs flatpak and flatpak-builder installed
set -euo pipefail
cd "$(dirname "$0")/../.."
version=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "rimstack-app"))')
app=io.github.TheForgotten69.RimStack
manifest=packaging/flatpak/$app.yaml
build=target/packaging/flatpak
repo=$build/repo
out=target/packages
mkdir -p "$repo" "$out"

# The crates, as sources: the build then needs no network.
packaging/flatpak/generated-sources.sh

flatpak-builder \
    --user \
    --install-deps-from=flathub \
    --force-clean \
    --repo "$repo" \
    "$build/build" \
    "$manifest"

flatpak build-bundle "$repo" "$out/RimStack-$version-x86_64.flatpak" "$app"
echo "$out/RimStack-$version-x86_64.flatpak"
