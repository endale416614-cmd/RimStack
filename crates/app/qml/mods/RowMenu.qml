import QtQuick
import RimStack.Ui

QtObject {
    function open(row, x, y) {
        const inActive = row.ownModel === Mods.active
        if (!(row.current))
            Mods.select(row.modId, inActive)
        const about = Hub.mods.notInstalled(inActive, row.modIndex) || Mods.about(row.modId)
        const many = Mods.selectedCount > 1
        const workshop = !!about.workshop
        const page = about.workshopPage || ""
        const groups = []
        groups.push([
            { text: about.takeOff || (many ? (inActive ? Theme.plural(Mods.selectedCount, "Deactivate {} mod", "Deactivate {} mods") : Theme.plural(Mods.selectedCount, "Activate {} mod", "Activate {} mods")) : inActive ? Theme.say("Deactivate") : Theme.say("Activate")), icon: inActive ? "minus" : "plus", off: Hub.mods.previewing,
              run: () => many ? Mods.moveSelected() : Mods.moveAcross(row.ownModel, row.modIndex, row.modId) },
        ])
        const open = []
        if (about.path)
            open.push({ text: Theme.say("Open folder"), icon: "folder-open", run: () => Qt.openUrlExternally(Theme.fileUrl(about.path)) })
        if (about.url)
            open.push({ text: Theme.say("Mod's own page"), icon: "link", run: () => Qt.openUrlExternally(about.url) })
        open.push({ text: Theme.say("Copy name"), icon: "copy", run: () => Theme.copy(about.name || "") })
        if (about.packageId)
            open.push({ text: Theme.say("Copy package id"), icon: "copy", run: () => Theme.copy(about.packageId) })
        groups.push(open)
        const fetch = []
        for (const fix of about.fixes || [])
            fetch.push({ text: Theme.say(fix.label), icon: "external-link", run: () => Mods.applyFix(row.modId, fix) })
        for (const id of about.resubscribe || [])
            fetch.push({ text: Theme.say("Subscribe again"), icon: "download", run: () => Hub.downloads.subscribe(id) })
        groups.push(fetch)
        // With several selected, its actions on one mod would read as acting on them all.
        if (workshop && !many) {
            const steam = [
                { title: Theme.say("Steam Workshop") },
                { text: Theme.say("Page in Steam"), icon: "external-link", run: () => Qt.openUrlExternally(about.steamPage) },
                { text: Theme.say("Page in the browser"), icon: "external-link", run: () => Qt.openUrlExternally(page) },
                { text: Theme.say("Copy link"), icon: "copy", run: () => Theme.copy(page) },
                { text: Theme.say("Copy to the game's Mods folder…"), icon: "copy", run: () => Notices.ask({
                    title: Theme.fill("Copy {} to the game's Mods folder", (about.name || "")),
                    lines: [Theme.say("From"), { path: about.path }, Theme.say("Into"), { path: Hub.settings.report.localMods || "" }, Theme.say("The Workshop copy stays where it is; Steam keeps updating that one, not this.")],
                    label: Theme.say("Copy"), run: () => Mods.copyToLocal(row.modId) }) },
            ]
            if (Hub.settings.report.steamcmd)
                steam.push({ text: Theme.say("Download again with SteamCMD…"), icon: "download", run: () => Notices.ask({
                    title: Theme.fill("Download {} again", (about.name || "")),
                    lines: [Theme.say("SteamCMD downloads the mod anew, and the fresh copy replaces this folder:"), { path: about.path }],
                    label: Theme.say("Download"), run: () => Hub.downloads.redownload(row.modId) }) })
            if (!inActive)
                steam.push({ text: Theme.say("Unsubscribe…"), icon: "minus", danger: true, run: () => Mods.unsubscribe(row.modId) })
            groups.push(steam)
        }
        if (about.repository)
            groups.push([
                { title: Theme.say("Git") },
                { text: Theme.say("Repository"), icon: "link", run: () => Qt.openUrlExternally(about.repository) },
                { text: Theme.say("Update"), icon: "download", run: () => Hub.downloads.updateFromGit(row.modId) },
            ])
        const filing = [{ title: Theme.say("Group under a tag") }]
        for (const tag of about.groupChoices || [])
            filing.push({ text: Theme.fill("Group under {}", tag), run: () => Mods.putInSub(row.modId, tag) })
        filing.push({ text: Theme.say("A new tag…"), icon: "plus", run: () => Mods.subNameRequested(row.modId) })
        if (about.sub)
            filing.push({ text: Theme.fill("Remove the tag {}", about.sub), run: () => Mods.putInSub(row.modId, "") })
        if (about.groupChoices)
            groups.push(filing)
        const last = []
        if (many && Mods.versionWarned.length > 0)
            last.push({ text: Theme.plural(Mods.versionWarned.length, "Ignore the game version mismatch of {} mod", "Ignore the game version mismatch of {} mods"), icon: "check", run: () => Mods.trustSelected(true) })
        else if (many && about.versionOk)
            last.push({ text: Theme.plural(Mods.selectedCount, "Warn about the game version of {} mod again", "Warn about the game version of {} mods again"), run: () => Mods.trustSelected(false) })
        else if (about.versionOk)
            last.push({ text: Theme.say("Warn about its game version again"), run: () => Mods.trust(row.modId, false) })
        else if (about.offerTrust)
            last.push({ text: Theme.say("Ignore the game version mismatch"), icon: "check", run: () => Mods.trust(row.modId, true) })
        const leaving = many && !inActive ? Mods.upkept.unsubscribe.length : 0
        const binnable = many ? Mods.upkept.bin.length : 0
        if (leaving > 0)
            last.push({ text: Theme.plural(leaving, "Unsubscribe from {} mod…", "Unsubscribe from {} mods…"), icon: "minus", danger: true, run: Mods.unsubscribeSelected })
        if (binnable > 0)
            last.push({ text: Theme.plural(binnable, "Move {} mod to the bin…", "Move {} mods to the bin…"), icon: "trash-2", danger: true, run: Mods.binSelected })
        if (!many && about.canBin)
            last.push({ text: Theme.say("Move to the bin…"), icon: "trash-2", danger: true, run: () => Mods.binRequested(about.name || "", about.path, row.modId) })
        groups.push(last)
        const entries = []
        for (const group of groups.filter(group => group.length > 0)) {
            if (entries.length > 0)
                entries.push({ line: true })
            entries.push(...group)
        }
        Mods.menuRequested(row, x, y, entries)
    }
}
