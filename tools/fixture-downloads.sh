#!/usr/bin/env bash
# What the Downloads screen shows, added to a made-up install from tools/fixture-game.sh:
# fixture-downloads.sh <fixture dir> <RimStack's data folder>. Workshop mods, three of them behind
# the last check and two changed since the last start, a Git checkout behind its remote, two
# SteamCMD downloads in the game's Mods folder (one RimStack recorded, behind; one it did not; one
# Steam said nothing of), and three mods on the download list, one whose download failed.
# Its Workshop folder is <fixture dir>/steam/workshop/content/294100, where Steam's record is found.
set -euo pipefail
mkdir -p "$2"
dir="$(realpath "$1")"
data="$(realpath "$2")"
workshop="$dir/steam/workshop/content/294100"
mkdir -p "$workshop" "$data"
now=$(date +%s)
installed=$((now - 30 * 86400))
started=$((now - 3 * 86400))

ids=(3000000001 3000000002 3000000003 3000000004)
names=("Workshop Fixture Alpha" "Workshop Fixture Beta" "Workshop Fixture Gamma" "Workshop Fixture Delta")
for i in "${!ids[@]}"; do
    mkdir -p "$workshop/${ids[$i]}/About" "$workshop/${ids[$i]}/Defs"
    printf '<?xml version="1.0" encoding="utf-8"?>\n<ModMetaData>\n  <packageId>fixture.workshop%s</packageId>\n  <name>%s</name>\n  <author>Fixture</author>\n  <supportedVersions><li>1.6</li></supportedVersions>\n</ModMetaData>\n' \
        "$i" "${names[$i]}" > "$workshop/${ids[$i]}/About/About.xml"
done

# Alpha and Beta changed since the last start; Gamma is current; Delta, inactive, is behind.
updated=($((now - 86400)) $((now - 2 * 86400)) "$installed" "$installed")
{
    printf '"AppWorkshop"\n{\n\t"appid"\t\t"294100"\n\t"WorkshopItemsInstalled"\n\t{\n'
    for i in "${!ids[@]}"; do
        printf '\t\t"%s"\n\t\t{\n\t\t\t"size"\t\t"%s"\n\t\t\t"timeupdated"\t\t"%s"\n\t\t}\n' "${ids[$i]}" $(((i + 1) * 4200000)) "${updated[$i]}"
    done
    printf '\t}\n}\n'
} > "$dir/steam/workshop/appworkshop_294100.acf"

copies=(3200000001 3200000002 3200000003)
copy_names=("SteamCMD Fixture Epsilon" "SteamCMD Fixture Zeta" "SteamCMD Fixture Eta")
for i in "${!copies[@]}"; do
    mkdir -p "$dir/game/Mods/${copies[$i]}/About"
    printf '<?xml version="1.0" encoding="utf-8"?>\n<ModMetaData>\n  <packageId>fixture.steamcmd%s</packageId>\n  <name>%s</name>\n  <author>Fixture</author>\n  <supportedVersions><li>1.6</li></supportedVersions>\n</ModMetaData>\n' \
        "$i" "${copy_names[$i]}" > "$dir/game/Mods/${copies[$i]}/About/About.xml"
done
printf '{"%s":{"manifest":1,"updated":%s}}' "${copies[0]}" "$installed" > "$data/steamcmd-copies.json"

sed -i 's|    <li>fixture.mod040</li>|    <li>fixture.workshop0</li>\n    <li>fixture.workshop1</li>\n    <li>fixture.workshop2</li>\n&|' "$dir/config/Config/ModsConfig.xml"

checkout="$dir/game/Mods/fixture-005"
git() { GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 command git -C "$checkout" -c user.name=Fixture -c user.email=fixture@example.invalid "$@"; }
git init -q
git add -A
GIT_AUTHOR_DATE="@$installed" GIT_COMMITTER_DATE="@$installed" git commit -q -m "First version"
GIT_AUTHOR_DATE="@$((now - 7200))" GIT_COMMITTER_DATE="@$((now - 7200))" git commit -q --allow-empty -m "Faster pawn search"

printf '{"time":%s,"workshop":{"%s":%s,"%s":%s,"%s":%s,"%s":%s}}' "$started" \
    "${ids[0]}" "$installed" "${ids[1]}" "$installed" "${ids[2]}" "$installed" "${ids[3]}" "$installed" > "$data/at-launch.json"
printf '{"checked":%s,"asked":7,"now":{"%s":%s,"%s":%s,"%s":%s,"%s":%s,"%s":%s,"%s":%s},"manifests":{"%s":2},"unanswered":["%s"],"checkouts":{"%s":{"behind":3}}}' $((now - 600)) \
    "${ids[0]}" $((now - 3600)) "${ids[1]}" $((now - 3600)) "${ids[2]}" "$installed" "${ids[3]}" $((now - 3600)) \
    "${copies[0]}" $((now - 3600)) "${copies[1]}" "$installed" "${copies[0]}" "${copies[2]}" "$checkout" > "$data/updates.json"

cat > "$data/downloadList.json" <<'LIST'
[{"id":"3100000001","title":"Listed Fixture Hospitality","size":48234496,"why":"added","failed":null},
 {"id":"3100000002","title":"Listed Fixture Library","size":null,"why":"missing","failed":null},
 {"id":"3100000003","title":"Listed Fixture Vehicles","size":1260000,"why":"added","failed":["Not on the Workshop"]}]
LIST
