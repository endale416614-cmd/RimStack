#!/usr/bin/env bash
# generated-sources.json basically duplicates Cargo.lock
# we generate it and use Cargo.lock as source of truth
set -euo pipefail
cd "$(dirname "$0")/../.."
# Pinned for safety and simplicity
generator="https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/9d86d336a39b9af38b86f114b6ce5820fe260f91/cargo/flatpak-cargo-generator.py"
generator_sha=0a2db6be87d75910facef28ab46d4d6460802e8419ab850d0caa6a364d26b380

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
curl -fsSL -o "$work/generator.py" "$generator"
if ! echo "$generator_sha  $work/generator.py" | sha256sum --check --status; then
    echo "the generator is not the one this script pinned ($generator_sha)" >&2
    exit 1
fi

uv run --quiet --with aiohttp --with tomlkit --with pyyaml \
    "$work/generator.py" Cargo.lock -o packaging/flatpak/generated-sources.json
echo "packaging/flatpak/generated-sources.json"
