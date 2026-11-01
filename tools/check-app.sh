#!/usr/bin/env bash
# Drives every action of the Mods screen on your real mods, with no window,
# and checks what Save writes. The game's settings are COPIED to a temporary
# folder first; the real ones are only read.
#
#   cargo build -p rimstack-app --release && tools/check-app.sh
#
# RIMSTACK_LANGUAGE=Français runs them with that language chosen; English
# is chosen otherwise, whatever the system is set to.
set -euo pipefail

real="${RIMSTACK_CONFIG:-$HOME/.config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios}"
app="$(dirname "$0")/../target/release/rimstack"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

if [ -n "${RIMSTACK_FIXTURE:-}" ] || [ ! -f "$real/Config/ModsConfig.xml" ]; then
    "$(dirname "$0")/fixture-game.sh" "$tmp/fixture"
    real="$tmp/fixture/config"
    export RIMSTACK_GAME="$tmp/fixture/game" RIMSTACK_WORKSHOP="$tmp/fixture/workshop"
    echo "check on a made-up install of $(ls "$RIMSTACK_GAME/Mods" | wc -l) mods"
fi

mkdir -p "$tmp/config/Config" "$tmp/data/RimStack"
language="${RIMSTACK_LANGUAGE:-English}"
for settings in settings settings2; do
    mkdir -p "$tmp/$settings/RimStack"
    printf '{ "language": "%s" }' "$language" > "$tmp/$settings/RimStack/settings.json"
done
cp "$real/Config/ModsConfig.xml" "$tmp/config/Config/"
cp "$tmp/config/Config/ModsConfig.xml" "$tmp/original.xml"
# Sort with the same community rules as the real run, if there are any.
cp "${XDG_DATA_HOME:-$HOME/.local/share}/RimStack/communityRules.json" "$tmp/data/RimStack/" 2>/dev/null || true

fail() { echo "check FAIL $1"; exit 1; }
# A QML error or a panic off the window's thread does not stop the app,
# so a check can pass over either.
sound() {
    local errors
    errors=$(grep -E 'TypeError|ReferenceError|is not a function|is not defined|Cannot read property|panicked' "$1" | head -3 || true)
    [ -z "$errors" ] || fail "errors in $2:
$errors"
    # The window's thread kept busy: by one check's actions (step_ms, timed
    # in QML) or between them (stall_ms, timed from another thread). On
    # 2,000 mods the steps take 61 ms at most, and one-off waits that come
    # back on no later run reached 360 ms; half a second is a freeze.
    local slow
    slow=$(grep -oE '(step|stall)_ms=[0-9]+.*' "$1" | awk -F'[= ]' '$2 >= 500' | head -3 || true)
    [ -z "$slow" ] || fail "the window's thread was kept busy in $2:
$slow"
}
shown() { grep -E '^qml: (check|step_ms=)|^stall_ms=' "$1" | sed 's/^qml: //' || true; }
run() {
    local log="$tmp/run.log" status=0
    RIMSTACK_CONFIG="$tmp/config" XDG_DATA_HOME="$tmp/data" XDG_CONFIG_HOME="$tmp/settings" \
        QT_QPA_PLATFORM=offscreen QT_FORCE_STDERR_LOGGING=1 timeout 60 "$app" "$@" >| "$log" 2>&1 || status=$?
    shown "$log"
    sound "$log" "$*"
    return "$status"
}
saved="$tmp/config/Config/ModsConfig.xml"

run --check
cmp -s "$saved" "$tmp/original.xml" && fail "the saved file is unchanged"
others=$(diff "$tmp/original.xml" "$saved" | grep '^[<>]' | grep -vc '<li>' || true)
[ "$others" = 0 ] || fail "Save changed something besides the list of mods"
cmp -s "$saved.rimstack-original" "$tmp/original.xml" || fail "no untouched copy of the first file"
[ -f "$saved.rimstack-backup" ] || fail "no backup of the file before the save"
[ ! -e "$saved.rimstack-part" ] || fail "a half-written file was left behind"
echo "check ok    Save changed only the list of mods and kept both copies"

run --check=restore
cmp -s "$saved" "$tmp/original.xml" || fail "moving the mod back did not give the original file"
echo "check ok    saving the original order gives the original file, byte for byte"

# What only the mouse shows: dragging, the right button, the wheel. Sent
# by Qt's test module (QtTest), which only this step needs. Nothing is
# saved, so the file must still be the original afterwards.
run --check=pointer || fail "the pointer checks did not pass"
cmp -s "$saved" "$tmp/original.xml" || fail "the pointer checks changed the file"

cmp -s "$real/Config/ModsConfig.xml" "$tmp/original.xml" || fail "THE REAL FILE CHANGED"
echo "check ok    the real ModsConfig.xml was not touched"

# Choosing between two mods' versions of a texture renames a file inside a
# mod, so it is only ever checked on two made-up mods in the temporary folder.
game="$tmp/game"
picture() {
    python3 - "$@" <<'PY'
import struct, sys, zlib
path, colour = sys.argv[1], bytes(int(c) for c in sys.argv[2:5])
def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
header = struct.pack(">IIBBBBB", 8, 8, 8, 2, 0, 0, 0)
pixels = zlib.compress((b"\0" + colour * 8) * 8)
with open(path, "wb") as out:
    out.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", pixels) + chunk(b"IEND", b""))
PY
}
for m in first second; do
    mkdir -p "$game/Mods/$m/About" "$game/Mods/$m/Textures/Things"
    printf '<ModMetaData><name>%s</name><packageId>check.%s</packageId><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>' "$m" "$m" > "$game/Mods/$m/About/About.xml"
    picture "$game/Mods/$m/Textures/Things/Shared.png" $([ $m = first ] && echo 255 0 0 || echo 0 0 255)
done
echo "1.6.4871 rev598" > "$game/Version.txt"
mkdir -p "$tmp/empty-workshop" "$tmp/config2/Config"
printf '<ModsConfigData>\n  <version>1.6.4871 rev598</version>\n  <activeMods>\n    <li>check.first</li>\n    <li>check.second</li>\n  </activeMods>\n</ModsConfigData>\n' > "$tmp/config2/Config/ModsConfig.xml"
cp -r "$game" "$tmp/game-before"

RIMSTACK_GAME="$game" RIMSTACK_WORKSHOP="$tmp/empty-workshop" RIMSTACK_CONFIG="$tmp/config2" \
    XDG_DATA_HOME="$tmp/data2" XDG_CONFIG_HOME="$tmp/settings2" \
    QT_QPA_PLATFORM=offscreen QT_FORCE_STDERR_LOGGING=1 timeout 60 "$app" --check=textures >| "$tmp/textures.log" 2>&1 && ok=0 || ok=$?
shown "$tmp/textures.log"
sound "$tmp/textures.log" "--check=textures"
[ "$ok" = 0 ] || fail "the texture choice checks"
diff -r "$game" "$tmp/game-before" > /dev/null || fail "the two mods are not as they were after unhiding"
echo "check ok    after unhiding, the two mods are exactly as they were"

# What changes on disk while RimStack is open: the same made-up game, a
# second Workshop folder to change to, and a mod and a list written from
# outside once the window says it is ready.
mkdir -p "$tmp/workshop-b/900001/About" "$tmp/settings3/RimStack" "$tmp/config3/Config"
printf '<ModMetaData><name>third</name><packageId>check.third</packageId><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>' > "$tmp/workshop-b/900001/About/About.xml"
cp "$tmp/config2/Config/ModsConfig.xml" "$tmp/config3/Config/"
printf '{ "workshop": "%s", "setupDone": "yes", "language": "%s" }' "$tmp/empty-workshop" "$language" > "$tmp/settings3/RimStack/settings.json"
live="$tmp/live.log"
env -u RIMSTACK_WORKSHOP RIMSTACK_GAME="$game" RIMSTACK_CONFIG="$tmp/config3" XDG_DATA_HOME="$tmp/data3" XDG_CONFIG_HOME="$tmp/settings3" \
    QT_QPA_PLATFORM=offscreen QT_FORCE_STDERR_LOGGING=1 timeout 60 "$app" --check=live "--live-folder=$tmp/workshop-b" > "$live" 2>&1 &
shown=$!
for _ in $(seq 60); do grep -q 'check live-ready' "$live" && break; sleep 0.5; done
mkdir -p "$game/Mods/fourth/About"
printf '<ModMetaData><name>fourth</name><packageId>check.fourth</packageId><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>' > "$game/Mods/fourth/About/About.xml"
sleep 4
sed -i '/check.second/d' "$tmp/config3/Config/ModsConfig.xml"
wait "$shown" && ok=0 || ok=$?
shown "$live"
sound "$live" "--check=live"
[ -f "$game/Mods/900001/About/About.xml" ] || fail "the copied mod is not in the game's own Mods folder"
rm -rf "$game/Mods/fourth" "$game/Mods/900001"
[ "$ok" = 0 ] || fail "the live changes"

"$(dirname "$0")/fixture-game.sh" "$tmp/screens"
on_fixture() {
    local check="$1" seconds="$2"
    mkdir -p "$tmp/settings-$check/RimStack"
    printf '{ "language": "%s", "setupDone": "yes" }' "$language" > "$tmp/settings-$check/RimStack/settings.json"
    RIMSTACK_GAME="$tmp/screens/game" RIMSTACK_WORKSHOP="$tmp/screens/workshop" RIMSTACK_CONFIG="$tmp/screens/config" \
        XDG_DATA_HOME="$tmp/data-$check" XDG_CONFIG_HOME="$tmp/settings-$check" \
        QT_QPA_PLATFORM=offscreen QT_FORCE_STDERR_LOGGING=1 timeout "$seconds" "$app" "--check=$check" >| "$tmp/$check.log" 2>&1 && ok=0 || ok=$?
}
on_fixture screens 90
shown "$tmp/screens.log"
sound "$tmp/screens.log" "--check=screens"
[ "$ok" = 0 ] || fail "the screen checks"

on_fixture layout 300
grep -E '^qml: check (FAIL|  )' "$tmp/layout.log" | sed 's/^qml: //' || true
echo "check ok    $(grep -c '^qml: check ok' "$tmp/layout.log") layouts with nothing overlapping or spilling"
sound "$tmp/layout.log" "--check=layout"
[ "$ok" = 0 ] || fail "the layout checks"

# Closing the window as a person does, while work runs, and starting with
# something missing or broken: each must end with 0, no panic, no QML error.
closes() {
    local what="$1" log="$tmp/close.log" status=0
    shift
    env "$@" QT_QPA_PLATFORM=offscreen QT_FORCE_STDERR_LOGGING=1 timeout 60 "$app" --check=close >| "$log" 2>&1 || status=$?
    shown "$log"
    sound "$log" "$what"
    [ "$status" = 0 ] || fail "$what: ended with $status"
    echo "check ok    $what: closed, ended with 0"
}
closes "closed while a rescan and a texture audit run" RIMSTACK_CONFIG="$tmp/config" XDG_DATA_HOME="$tmp/data" XDG_CONFIG_HOME="$tmp/settings"
cmp -s "$real/Config/ModsConfig.xml" "$tmp/original.xml" || fail "THE REAL FILE CHANGED"

for n in 4 5 6; do mkdir -p "$tmp/settings$n/RimStack" "$tmp/data$n"; done
printf '{ "language": "%s" }' "$language" > "$tmp/settings4/RimStack/settings.json"
printf '{ "language": ' > "$tmp/settings5/RimStack/settings.json"
printf '{ "language": "%s" }' "$language" > "$tmp/settings6/RimStack/settings.json"
closes "no game folder" RIMSTACK_GAME="$tmp/nowhere" RIMSTACK_WORKSHOP="$tmp/empty-workshop" RIMSTACK_CONFIG="$tmp/config2" XDG_DATA_HOME="$tmp/data4" XDG_CONFIG_HOME="$tmp/settings4"
grep -qE '^qml: check state [1-9][0-9]* active, [1-9][0-9]* errors' "$tmp/close.log" || fail "no game folder: the window did not say its mods are missing"
closes "a damaged settings.json" RIMSTACK_GAME="$game" RIMSTACK_WORKSHOP="$tmp/empty-workshop" RIMSTACK_CONFIG="$tmp/config2" XDG_DATA_HOME="$tmp/data5" XDG_CONFIG_HOME="$tmp/settings5"
chmod 500 "$tmp/data6"
closes "a data folder that cannot be written" RIMSTACK_GAME="$game" RIMSTACK_WORKSHOP="$tmp/empty-workshop" RIMSTACK_CONFIG="$tmp/config2" XDG_DATA_HOME="$tmp/data6" XDG_CONFIG_HOME="$tmp/settings6"
chmod 700 "$tmp/data6"

# --check saves a load order: without a copy of the settings it must refuse.
mkdir -p "$tmp/home"
status=0
env -u RIMSTACK_CONFIG HOME="$tmp/home" XDG_DATA_HOME="$tmp/home/data" XDG_CONFIG_HOME="$tmp/home/settings" \
    QT_QPA_PLATFORM=offscreen timeout 60 "$app" --check >| "$tmp/refused.log" 2>&1 || status=$?
[ "$status" = 2 ] || fail "--check without a copy of the settings ended with $status, not 2"
[ -z "$(find "$tmp/home" -type f)" ] || fail "--check without a copy of the settings wrote files"
echo "check ok    --check without a copy of the settings is refused before anything is written"
