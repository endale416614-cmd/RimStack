#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
version=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "rimstack-app"))')
tools=target/packaging/tools
app=target/packaging/AppDir
out=target/packages
mkdir -p "$tools" "$out"

fetch() { # name url
    [ -x "$tools/$1" ] && return
    curl --fail --location --silent --show-error --output "$tools/$1" "$2"
    chmod +x "$tools/$1"
}
fetch linuxdeploy https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage

profile="${RIMSTACK_PROFILE:-dist}"
cargo build --profile "$profile" -p rimstack-app
rm -rf "$app"
mkdir -p "$app/usr/share/icons/hicolor/scalable/apps"
cp crates/app/assets/rimstack.svg "$app/usr/share/icons/hicolor/scalable/apps/"

export LDAI_OUTPUT="$out/RimStack-$version-x86_64.AppImage"
export APPIMAGE_EXTRACT_AND_RUN=1
export NO_STRIP=1
PATH="$PWD/$tools:$PATH" linuxdeploy \
    --appdir "$app" \
    --executable "target/$profile/rimstack" \
    --desktop-file crates/app/assets/rimstack.desktop \
    --icon-file crates/app/assets/rimstack.svg \
    --output appimage
echo "$LDAI_OUTPUT"
