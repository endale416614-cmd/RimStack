#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
version=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "rimstack-app"))')
app=target/packaging/RimStack.app
out=target/packages
mkdir -p "$out"

profile="${RIMSTACK_PROFILE:-dist}"
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --profile "$profile" -p rimstack-app --target aarch64-apple-darwin --target x86_64-apple-darwin
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
# Qt's frameworks hold both architectures already; the program is joined from its two builds.
lipo -create -output "$app/Contents/MacOS/rimstack" \
    "target/aarch64-apple-darwin/$profile/rimstack" "target/x86_64-apple-darwin/$profile/rimstack"
sed "s/@VERSION@/$version/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"

iconset=target/packaging/rimstack.iconset
rm -rf "$iconset" && mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    rsvg-convert -w $size -h $size crates/app/assets/rimstack.svg -o "$iconset/icon_${size}x${size}.png"
    rsvg-convert -w $((size * 2)) -h $((size * 2)) crates/app/assets/rimstack.svg -o "$iconset/icon_${size}x${size}@2x.png"
done
iconutil -c icns -o "$app/Contents/Resources/rimstack.icns" "$iconset"

if [ -n "${MAC_SIGN_IDENTITY:-}" ]; then
    macdeployqt "$app" -qmldir=crates/app/qml -hardened-runtime -timestamp -codesign="$MAC_SIGN_IDENTITY"
    codesign --force --timestamp --options runtime \
        --entitlements packaging/macos/entitlements.plist \
        --sign "$MAC_SIGN_IDENTITY" "$app"
    codesign --verify --strict --verbose=2 "$app"
else
    macdeployqt "$app" -qmldir=crates/app/qml
    codesign --force --deep --sign - "$app"
fi

if [ -n "${MAC_SIGN_IDENTITY:-}" ] && [ -n "${MAC_NOTARY_PROFILE:-}" ]; then
    zip="target/packaging/RimStack-notarize.zip"
    ditto -c -k --keepParent "$app" "$zip"
    xcrun notarytool submit "$zip" --keychain-profile "$MAC_NOTARY_PROFILE" --wait
    xcrun stapler staple "$app"
fi

dmg="$out/RimStack-$version-macos-universal.dmg"
hdiutil create -volname RimStack -srcfolder "$app" -ov -format UDZO "$dmg"
echo "$dmg"
