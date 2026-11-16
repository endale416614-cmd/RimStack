#!/usr/bin/env bash
set -euo pipefail
dir="$1"
game="$dir/game"
mkdir -p "$game/Data" "$game/Mods" "$dir/config/Config" "$dir/workshop"
echo "1.6.4871 rev598" > "$game/Version.txt"

about() { # folder packageId name [extra xml]
    mkdir -p "$1/About"
    printf '<?xml version="1.0" encoding="utf-8"?>\n<ModMetaData>\n  <packageId>%s</packageId>\n  <name>%s</name>\n  <author>Fixture</author>\n  <supportedVersions><li>%s</li></supportedVersions>\n%s</ModMetaData>\n' \
        "$2" "$3" "${5:-1.6}" "${4:-}" > "$1/About/About.xml"
}

about "$game/Data/Core" ludeon.rimworld Core
about "$game/Data/Royalty" ludeon.rimworld.royalty Royalty
about "$game/Data/Ideology" ludeon.rimworld.ideology Ideology
mkdir -p "$game/Data/Core/Defs"

kinds=(Defs Patches Textures)
png="iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg=="
for official in Core Royalty Ideology; do
    base64 -d <<< "$png" > "$game/Data/$official/About/Preview.png"
done
for n in $(seq -w 1 100); do
    folder="$game/Mods/fixture-$n"
    extra=""
    if [ $((10#$n % 7)) = 0 ]; then
        extra="  <loadAfter><li>fixture.mod$(printf %03d $((10#$n - 1)))</li></loadAfter>\n"
    fi
    version="1.6"
    [ $((10#$n % 11)) = 0 ] && version="1.4"
    if [ "$n" = 003 ]; then
        extra="$extra  <modDependencies><li><packageId>fixture.missing</packageId><displayName>Fixture Library</displayName><steamWorkshopUrl>steam://url/CommunityFilePage/123456789</steamWorkshopUrl></li></modDependencies>\n"
    fi
    about "$folder" "fixture.mod$n" "Fixture Mod $n" "$(printf "$extra")" "$version"
    kind=${kinds[$((10#$n % 3))]}
    mkdir -p "$folder/$kind"
    if [ "$n" = 001 ]; then
        mkdir -p "$folder/1.4/Defs" "$folder/1.6/Defs" "$folder/Code"
        printf '<Defs>FixtureOldVersion</Defs>\n' > "$folder/1.4/Defs/Old.xml"
        printf '<Defs>FixtureNewVersion</Defs>\n' > "$folder/1.6/Defs/New.xml"
        printf '// FixtureProbe\n' > "$folder/Code/Probe.cs"
    fi
    case $kind in
        Defs) printf '<Defs><ThingDef><defName>Fixture%s</defName></ThingDef></Defs>\n' "$n" > "$folder/Defs/Things.xml" ;;
        Patches) printf '<Patch><Operation Class="PatchOperationAdd"><xpath>Defs</xpath><value/></Operation></Patch>\n' > "$folder/Patches/Patch.xml" ;;
        Textures) base64 -d <<< "$png" > "$folder/Textures/Fixture$n.png" ;;
    esac
done

{
    printf '<?xml version="1.0" encoding="utf-8"?>\n<ModsConfigData>\n  <version>1.6.4871 rev598</version>\n  <activeMods>\n'
    printf '    <li>ludeon.rimworld</li>\n    <li>ludeon.rimworld.royalty</li>\n    <li>ludeon.rimworld.ideology</li>\n'
    for n in $(seq 40 -1 1); do printf '    <li>fixture.mod%03d</li>\n' "$n"; done
    printf '  </activeMods>\n  <knownExpansions>\n    <li>ludeon.rimworld.royalty</li>\n    <li>ludeon.rimworld.ideology</li>\n  </knownExpansions>\n</ModsConfigData>\n'
} > "$dir/config/Config/ModsConfig.xml"

save() { # file mod...
    mkdir -p "$dir/config/Saves"
    {
        printf '\xef\xbb\xbf<?xml version="1.0" encoding="utf-8"?>\n<savegame>\n\t<meta>\n\t\t<gameVersion>1.6.4871 rev598</gameVersion>\n\t\t<modIds>\n'
        for id in "${@:2}"; do printf '\t\t\t<li>%s</li>\n' "$id"; done
        printf '\t\t</modIds>\n\t\t<modNames>\n'
        for id in "${@:2}"; do printf '\t\t\t<li>%s</li>\n' "$id"; done
        printf '\t\t</modNames>\n\t</meta>\n\t<game></game>\n</savegame>\n'
    } > "$dir/config/Saves/$1.rws"
}
active=(ludeon.rimworld ludeon.rimworld.royalty ludeon.rimworld.ideology)
for n in $(seq 40 -1 1); do active+=("$(printf 'fixture.mod%03d' "$n")"); done
save "Colony One" "${active[@]}"
save "Colony Two" "${active[@]}" fixture.gone
touch -d '2026-01-02 10:00' "$dir/config/Saves/Colony Two.rws"
touch -d '2026-01-01 10:00' "$dir/config/Saves/Colony One.rws"

cat > "$dir/config/Player.log" <<'LOG'
Mono path[0] = '/game/RimWorldLinux_Data/Managed'
RimWorld 1.6.4871 rev598
Error in static constructor of FixtureMod005.Init: System.NullReferenceException: Object reference not set to an instance of an object
[Ref 0A1B2C3D]
  at FixtureMod005.Init..cctor () [0x00001] in <abc>:0
Could not resolve cross-reference to Verse.ThingDef named Fixture999 (wanter=thingDef)
LOG
