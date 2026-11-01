pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

Item {
    id: screen
    Appear { of: screen }

    readonly property DownloadsData downloads: Hub.downloads
    signal showMod(int modId)

    readonly property var report: downloads.updates || ({})
    readonly property var found: downloads.queue || ({})
    readonly property var queue: found.items || []
    readonly property var repositories: found.repositories || []
    readonly property var wanted: found.wanted || []
    readonly property bool steamcmd: downloads.steamcmd
    readonly property var needed: Mods.overview.toGet || []
    readonly property var info: Hub.settings.report
    readonly property var since: downloads.since || ({ time: 0, count: 0, active: 0, rows: [], workshop: 0 })

    property bool sinceOpen: false
    readonly property var changedShown: since.rows
    function day(seconds) {
        return seconds ? Theme.day(seconds) : ""
    }

    function when(seconds) {
        return Theme.moment(seconds)
    }

    function megabytes(n) {
        return !n ? "" : n < 10 ? Theme.fill("{} MB", n.toFixed(1)) : Theme.fill("{} MB", Theme.count(Math.round(n)))
    }

    readonly property var activeUpdates: report.active || []
    readonly property var inactiveUpdates: report.inactive || []
    property bool inactiveOpen: false
    readonly property var checkouts: report.checkouts || []
    readonly property var checkoutsBehind: report.checkoutsBehind || []
    property bool checkoutsOpen: false
    readonly property int ignoredReplacements: (Hub.settings.report.replacementsIgnored || []).length

    component UpdateRow: RowLayout {
        id: row
        required property var modelData
        width: page.width
        height: Theme.sized(30)
        spacing: Theme.gapLarge
        Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: row.modelData.name }
        Chip { visible: row.modelData.asked; tone: Theme.ok; icon: "check"; text: Theme.say("Asked of Steam") }
        Chip { tone: Theme.moved; text: screen.day(row.modelData.have) + " → " + screen.day(row.modelData.now); tip: Theme.fill("Published {}", Theme.ago(row.modelData.now)) }
        Btn { text: Theme.say("Show"); spoken: Theme.fill("Show {}", row.modelData.name); onClicked: screen.showMod(row.modelData.modId) }
        Btn { text: Theme.say("What changed"); spoken: Theme.fill("What changed in {}", row.modelData.name); tip: Theme.say("The mod's change notes on the Workshop."); onClicked: Theme.openChangeNotes(row.modelData.id) }
    }

    component Cell: Text {
        elide: Text.ElideRight
        color: Theme.ink2
        font.pixelSize: Theme.body
    }

    Flickable {
        id: scroller
        anchors.fill: parent
        contentHeight: page.implicitHeight + 2 * Theme.gapWide
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        Wheel {}
        ScrollBar.vertical: Scroller {}

        Column {
            id: page
            x: Math.max(Theme.gapWide, (parent.width - width) / 2)
            y: Theme.gapWide
            width: screen.width - 2 * Theme.gapWide
            spacing: Theme.gapPage

            Column {
                width: parent.width
                spacing: Theme.gapSmall
                Text { color: Theme.ink2; font.pixelSize: Theme.small; font.weight: Font.DemiBold; text: Theme.say("Add mods") }
                RowLayout {
                    width: parent.width
                    spacing: Theme.gap
                    FilterField {
                        id: pasted
                        Layout.fillWidth: true
                        implicitHeight: Theme.sized(30)
                        placeholder: Theme.say("Paste Workshop links, collection links, mod ids or Git links")
                        onAccepted: add.clicked()
                    }
                    Btn {
                        id: add
                        enabled: !screen.downloads.queueBusy && pasted.text.trim() !== ""
                        primary: pasted.text.trim() !== ""
                        text: screen.downloads.queueBusy ? Theme.say("Asking Steam…") : Theme.say("Look up")
                        tip: Theme.say("Looks up what is pasted beside it: paste a link or an id first.")
                        onClicked: screen.downloads.lookUp(pasted.text)
                    }
                }
            }

            Card {
                visible: screen.queue.length + screen.repositories.length > 0 || !!(screen.found.problem || screen.found.note)
                title: Theme.say("Look up")
                note: screen.queue.length + screen.repositories.length > 0 ? screen.queue.length + screen.repositories.length : ""
                actions: [
                    Btn {
                        visible: screen.wanted.length > 0
                        icon: "plus"
                        text: Theme.say("Subscribe in Steam")
                        tip: Theme.say("Steam, which must be running, subscribes to each and downloads it; it keeps them up to date afterwards.")
                        onClicked: screen.downloads.subscribeWanted()
                    },
                    Btn {
                        visible: screen.wanted.length > 0 && screen.steamcmd
                        icon: "download"
                        text: Theme.say("Download with SteamCMD…")
                        tip: Theme.fill("Into the game's own Mods folder, {}, without subscribing: Steam will not update these.", (screen.info.localMods || ""))
                        onClicked: Notices.ask({
                            title: Theme.plural(screen.wanted.length, "Download {} mod with SteamCMD", "Download {} mods with SteamCMD"),
                            lines: [Theme.say("Each is downloaded and copied into the game's own Mods folder:"), { path: screen.info.localMods || "" }, Theme.say("Steam does not update mods installed this way.")],
                            label: Theme.say("Download"), run: () => screen.downloads.downloadWanted() })
                    }
                ]
                Text { visible: screen.queue.length === 0 && !!(screen.found.problem || screen.found.note); width: parent.width; wrapMode: Text.WordWrap; color: screen.found.problem ? Theme.err : Theme.ink2; font.pixelSize: Theme.body; text: screen.found.problem || screen.found.note || "" }
                Repeater {
                    model: screen.repositories
                    RowLayout {
                        id: repository
                        required property var modelData
                        required property int index
                        readonly property bool cloning: screen.downloads.cloning === index
                        width: page.width
                        height: Theme.sized(32)
                        spacing: Theme.gapLarge
                        Icon { name: "git-branch"; color: Theme.ink3 }
                        Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: repository.modelData.name }
                        Chip { visible: repository.modelData.installed; tone: Theme.ok; icon: "check"; text: Theme.say("Installed") }
                        Btn { icon: "external-link"; text: Theme.say("Repository page"); onClicked: Qt.openUrlExternally(repository.modelData.link) }
                        Btn {
                            visible: !repository.modelData.installed
                            enabled: !!screen.info.git && !!repository.modelData.into && screen.downloads.cloning < 0
                            primary: !repository.cloning
                            icon: "download"
                            text: repository.cloning ? Theme.say("Cloning…") : Theme.say("Clone…")
                            tip: screen.info.git ? Theme.say("git clone into the game's own Mods folder; Check for updates then asks its repository") : Theme.say("Git is not installed")
                            onClicked: Notices.ask({
                                title: Theme.fill("Clone {}", repository.modelData.name),
                                lines: [Theme.say("git clone, from"), { path: repository.modelData.link }, Theme.say("Into"), { path: repository.modelData.into }],
                                label: Theme.say("Clone"), run: () => screen.downloads.cloneRepository(repository.index) })
                        }
                    }
                }
                Repeater {
                    model: screen.queue
                    RowLayout {
                        id: item
                        required property var modelData
                        width: page.width
                        height: Theme.sized(32)
                        spacing: Theme.gapLarge
                        Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: item.modelData.title || item.modelData.id }
                        Cell { Layout.preferredWidth: Theme.sized(100); text: screen.day(item.modelData.updated) }
                        Cell { Layout.preferredWidth: Theme.sized(70); horizontalAlignment: Text.AlignRight; text: screen.megabytes(item.modelData.megabytes) }
                        Chip { visible: !item.modelData.title; tone: Theme.err; icon: "circle-alert"; text: Theme.say("Not on the Workshop") }
                        Chip { visible: !!item.modelData.title && item.modelData.installed; tone: Theme.ok; icon: "check"; text: Theme.say("Installed") }
                        Btn { visible: !!item.modelData.title && !item.modelData.installed; icon: "external-link"; text: Theme.say("Workshop page"); onClicked: Theme.openWorkshop(item.modelData.id) }
                    }
                }
            }

            Flow {
                id: cards
                width: parent.width
                spacing: Theme.gapLarge
                readonly property int across: Math.max(1, Math.min(3, Math.floor((width + spacing) / (Theme.sized(300) + spacing))))
                readonly property real each: (width - (across - 1) * spacing) / across
                readonly property real tallest: Math.max(updatesCard.natural, missingCard.natural, steamcmdCard.natural)

                StatusCard {
                    id: updatesCard
                    width: cards.each
                    minHeight: cards.tallest
                    title: Theme.say("Updates")
                    note: screen.report.checkedSaid || Theme.say("Never checked")
                    status: [
                        Chip {
                            visible: screen.downloads.updatesBusy
                            text: Theme.say("Asking Steam…")
                        },
                        Chip {
                            visible: !screen.downloads.updatesBusy && !!screen.report.problem
                            tone: Theme.err
                            icon: "circle-alert"
                            text: Theme.say("Steam not reached")
                        },
                        Chip {
                            visible: !screen.downloads.updatesBusy && !screen.report.problem && !!screen.report.checked && screen.report.count === 0
                            tone: Theme.ok
                            icon: "check"
                            text: Theme.say("All Workshop mods are current")
                        },
                        Chip {
                            visible: !screen.downloads.updatesBusy && !screen.report.problem && screen.report.count > 0
                            tone: Theme.moved
                            icon: "download"
                            text: Theme.plural(screen.report.count, "{} update available", "{} updates available")
                        }
                    ]
                    actions: Btn {
                        primary: !screen.report.checked
                        enabled: !screen.downloads.updatesBusy
                        icon: "refresh-cw"
                        text: Theme.say("Check now")
                        tip: screen.info.gitForWorkshop ? Theme.say("Asks Steam for the current version of each installed Workshop mod (only their Workshop ids are sent), and Git for new commits on each checkout in the game's Mods folder and in the Workshop folder.")
                            : Theme.say("Asks Steam for the current version of each installed Workshop mod (only their Workshop ids are sent), and Git for new commits on each checkout in the game's Mods folder.")
                        onClicked: screen.downloads.checkUpdates()
                    }
                }

                StatusCard {
                    id: missingCard
                    width: cards.each
                    minHeight: cards.tallest
                    title: Theme.say("Missing")
                    note: Theme.say("Needed by the load order")
                    status: [
                        Chip { visible: screen.needed.length === 0; tone: Theme.ok; icon: "check"; text: Theme.say("Nothing missing") },
                        Chip { visible: screen.needed.length > 0; tone: Theme.warn; icon: "triangle-alert"; text: Theme.plural(screen.needed.length, "{} mod missing", "{} mods missing") }
                    ]
                    Repeater {
                        model: screen.needed
                        ProblemLine {
                            required property var modelData
                            width: parent.width
                            stacked: true
                            name: modelData.name
                            words: modelData.words
                            fix: modelData.fix
                            ignore: modelData.ignore
                            packageId: modelData.packageId
                            modId: modelData.modId
                        }
                    }
                }

                StatusCard {
                    id: steamcmdCard
                    width: cards.each
                    minHeight: cards.tallest
                    title: Theme.say("SteamCMD")
                    status: [
                        Chip { visible: screen.steamcmd; tone: Theme.ok; icon: "check"; text: Theme.say("Installed") },
                        Chip { visible: !screen.steamcmd; text: Theme.say("Not installed") }
                    ]
                    Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink3; font.pixelSize: Theme.small; text: Theme.say("Fetches a mod without subscribing."); Tip { text: Theme.say("Valve's own downloader, fetched from Valve into RimStack's data folder. It downloads Workshop mods without a Steam account, into the game's Mods folder. It runs with a home folder of its own, so it cannot touch Steam's settings.") } }
                    actions: [
                        Btn {
                            visible: !screen.steamcmd
                            text: Theme.say("Install…")
                            onClicked: Notices.ask({
                                title: Theme.say("Install SteamCMD"),
                                lines: [Theme.say("Valve's own downloader is fetched from Valve into"), { path: screen.info.steamcmdFolder || "" }, Theme.say("It runs with a home folder of its own there, apart from Steam's settings.")],
                                label: Theme.say("Install"), run: () => screen.downloads.installSteamcmd() })
                        }
                    ]
                }
            }

            Card {
                visible: screen.downloads.updatesBusy || !!screen.report.problem || (screen.report.count || 0) > 0
                title: Theme.say("Newer versions")
                note: screen.downloads.updatesBusy ? Theme.say("Asking Steam…") : Theme.fill("{} for active mods", screen.activeUpdates.length)
                actions: [
                    Btn {
                        visible: (screen.report.count || 0) > 0
                        primary: screen.activeUpdates.length > 0
                        enabled: !screen.downloads.taskRunning
                        icon: "download"
                        text: Theme.say("Update in Steam")
                        tip: Theme.say("Steam, which must be running, downloads the newer version of each now. Left alone it does so the next time it starts.")
                        onClicked: screen.downloads.updateInSteam()
                    }
                ]
                Rectangle {
                    visible: screen.downloads.updatesBusy
                    width: parent.width
                    height: 3
                    radius: Math.min(width, height) / 2
                    color: Theme.chip
                    clip: true
                    Rectangle {
                        width: parent.width / 4
                        height: parent.height
                        radius: Math.min(width, height) / 2
                        color: Theme.accent
                        NumberAnimation on x { running: screen.downloads.updatesBusy && !Theme.stored.reduceMotion; from: -page.width / 4; to: page.width; duration: 1200; loops: Animation.Infinite }
                    }
                }
                Text { visible: !!screen.report.problem; width: parent.width; wrapMode: Text.WordWrap; color: Theme.err; font.pixelSize: Theme.body; text: Theme.fill("Steam not reached: {}", (screen.report.problem || "")) }
                Repeater {
                    model: screen.activeUpdates
                    UpdateRow {}
                }
                Row {
                    visible: screen.inactiveUpdates.length > 0
                    topPadding: screen.activeUpdates.length > 0 ? 8 : 0
                    spacing: Theme.gap
                    Text { color: Theme.ink2; font.pixelSize: Theme.small; font.weight: Font.DemiBold; text: Theme.say("Inactive mods") }
                    Text { color: Theme.ink3; font.pixelSize: Theme.small; text: screen.inactiveUpdates.length }
                }
                Repeater {
                    model: screen.inactiveOpen ? screen.inactiveUpdates : screen.inactiveUpdates.slice(0, 3)
                    UpdateRow {}
                }
                Btn {
                    visible: screen.inactiveUpdates.length > 3
                    text: screen.inactiveOpen ? Theme.fill("Show {}", 3) : Theme.fill("{} more", (screen.inactiveUpdates.length - 3))
                    onClicked: screen.inactiveOpen = !screen.inactiveOpen
                }
            }

            Card {
                visible: screen.checkouts.length > 0
                title: Theme.say("From Git")
                note: screen.checkoutsBehind.length > 0 ? Theme.fill("{} behind", screen.checkoutsBehind.length) : Theme.say("all up to date")
                Repeater {
                    model: screen.checkoutsOpen ? screen.checkouts : screen.checkoutsBehind
                    RowLayout {
                        id: checkout
                        required property var modelData
                        readonly property bool updating: screen.downloads.updating === modelData.modId
                        width: page.width
                        height: Theme.sized(30)
                        spacing: Theme.gapLarge
                        Icon { name: "git-branch"; color: Theme.ink3 }
                        Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: checkout.modelData.name }
                        Chip {
                            maxWidth: Theme.sized(340)
                            tone: checkout.modelData.problem || !checkout.modelData.checked ? Theme.ink2 : checkout.modelData.behind > 0 ? Theme.moved : Theme.ok
                            text: checkout.modelData.problem ? Theme.fill("Not checked: {}", checkout.modelData.problem)
                                : !checkout.modelData.checked ? Theme.say("Not checked")
                                : checkout.modelData.behind > 0 ? Theme.plural(checkout.modelData.behind, "{} commit behind", "{} commits behind") : Theme.say("Up to date")
                        }
                        Btn { text: Theme.say("Show"); spoken: Theme.fill("Show {}", checkout.modelData.name); onClicked: screen.showMod(checkout.modelData.modId) }
                        Btn {
                            visible: !checkout.modelData.workshop && checkout.modelData.behind > 0
                            enabled: screen.downloads.updating < 0
                            primary: !checkout.updating
                            text: checkout.updating ? Theme.say("Updating…") : Theme.say("Update")
                            spoken: Theme.fill("Update {}", checkout.modelData.name)
                            tip: Theme.say("Moves forward to the remote; stops if the folder has changes of its own.")
                            onClicked: screen.downloads.updateFromGit(checkout.modelData.modId)
                        }
                    }
                }
                RowLayout {
                    readonly property int settled: screen.checkouts.length - screen.checkoutsBehind.length
                    visible: settled > 0
                    width: page.width
                    height: Theme.sized(30)
                    spacing: Theme.gapLarge
                    Text { Layout.fillWidth: true; color: Theme.ink2; font.pixelSize: Theme.body; text: screen.checkoutsOpen ? "" : Theme.fill("{} not behind", parent.settled) }
                    Btn { text: screen.checkoutsOpen ? Theme.say("Only those behind") : Theme.say("Show all"); onClicked: screen.checkoutsOpen = !screen.checkoutsOpen }
                }
            }

            Card {
                visible: screen.since.count > 0
                title: Theme.say("Changed since the game last started")
                note: Theme.fill("{}, since {}", screen.since.count, screen.when(screen.since.time))
                key: "downloads-changed"
                actions: [
                    Choice {
                        readonly property int active: screen.since.active
                        visible: active > 0 && active < screen.since.count
                        title: Theme.say("Changed mods shown")
                        options: [Theme.say("All"), Theme.fill("Active {}", active), Theme.fill("Inactive {}", screen.since.count - active)]
                        current: ["", "active", "inactive"].indexOf(screen.downloads.sinceOnly)
                        onPicked: index => Hub.downloads.sinceOnly = ["", "active", "inactive"][index]
                    },
                    AskSteam {
                    visible: screen.since.workshop > 0
                    idle: Theme.say("Fetch change notes")
                    tip: Theme.say("Reads each mod's change notes from its Workshop page. Only the Workshop ids are sent.")
                    onClicked: screen.downloads.fetchSinceNotes()
                    }
                ]
                spacing: Theme.gapLarge
                Column {
                id: changedRows
                property int at: 0
                readonly property var shown: screen.sinceOpen ? screen.changedShown : screen.changedShown.slice(0, 12)
                function go(to) {
                    at = Math.max(0, Math.min(shown.length - 1, to))
                    const row = rows.itemAt(at)
                    if (!row)
                        return
                    const top = row.mapToItem(scroller.contentItem, 0, 0).y
                    if (top < scroller.contentY)
                        scroller.contentY = Math.max(0, top - Theme.gapLarge)
                    else if (top + row.height > scroller.contentY + scroller.height)
                        scroller.contentY = top + row.height + Theme.gapLarge - scroller.height
                }
                width: page.width
                spacing: Theme.gapLarge
                activeFocusOnTab: shown.length > 0
                Accessible.role: Accessible.List
                Accessible.name: Theme.say("Changed since the game last started")
                Keys.onUpPressed: go(at - 1)
                Keys.onDownPressed: go(at + 1)
                Keys.onReturnPressed: if (shown[at]) screen.showMod(shown[at].modId)
                Keys.onEnterPressed: if (shown[at]) screen.showMod(shown[at].modId)
                Keys.onPressed: event => {
                    if (event.key !== Qt.Key_N || !shown[at] || shown[at].git)
                        return
                    Theme.openChangeNotes(shown[at].id)
                    event.accepted = true
                }
                Repeater {
                    id: rows
                    model: changedRows.shown
                    Column {
                        id: changed
                        readonly property bool reached: changedRows.activeFocus && changedRows.at === index
                        required property var modelData
                        required property int index
                        readonly property string day: screen.day(modelData.now)
                        readonly property bool opensDay: modelData.opensDay
                        readonly property var said: modelData.said
                        width: page.width
                        spacing: Theme.gapTiny
                        Text {
                            visible: changed.opensDay
                            topPadding: changed.index === 0 ? 0 : Theme.gap
                            bottomPadding: Theme.gapThin
                            color: Theme.ink2
                            font.pixelSize: Theme.small
                            font.weight: Font.DemiBold
                            text: changed.day + "  " + Theme.plural(changed.modelData.dayCount, "{} mod", "{} mods")
                        }
                        RowLayout {
                            width: parent.width
                            spacing: Theme.gap
                            Icon { name: changed.modelData.git ? "git-branch" : "cloud"; color: changed.reached ? Theme.focus : Theme.ink3 }
                            Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; font.weight: Font.Medium; font.underline: changed.reached; text: changed.modelData.name; Accessible.role: Accessible.ListItem; Accessible.name: text }
                            Chip { visible: !changed.modelData.active; text: Theme.say("inactive") }
                            Chip { visible: changed.modelData.git; tone: Theme.moved; text: Theme.plural(changed.modelData.notes.length, "{} commit", "{} commits") }
                            Text {
                                visible: !changed.modelData.git
                                color: Theme.ink3
                                font.pixelSize: Theme.small
                                text: changed.modelData.sameDay ? Theme.clock(changed.modelData.was) + " → " + Theme.clock(changed.modelData.now)
                                    : screen.day(changed.modelData.was) + " → " + (changed.day === screen.day(Date.now() / 1000) ? Theme.say("today") : changed.day)
                            }
                            Btn { activeFocusOnTab: false; text: Theme.say("Show"); spoken: Theme.fill("Show {}", changed.modelData.name); tip: Theme.say("Enter"); onClicked: screen.showMod(changed.modelData.modId) }
                            Btn { activeFocusOnTab: false; visible: !changed.modelData.git; icon: "external-link"; spoken: Theme.fill("Change notes of {}", changed.modelData.name); tip: Theme.say("The mod's change notes on the Workshop."); onClicked: Theme.openChangeNotes(changed.modelData.id) }
                        }
                        Repeater {
                            model: changed.said
                            RowLayout {
                                id: entry
                                required property var modelData
                                width: changed.width
                                spacing: Theme.gap
                                Text { Layout.preferredWidth: Theme.sized(22); color: Theme.ink3; font.pixelSize: Theme.small; text: "" }
                                Text { Layout.preferredWidth: Theme.sized(74); Layout.alignment: Qt.AlignTop; color: Theme.ink3; font.pixelSize: Theme.small; text: entry.modelData.time ? screen.day(entry.modelData.time) : "" }
                                Text {
                                    Layout.fillWidth: true
                                    wrapMode: Text.WordWrap
                                    maximumLineCount: 12
                                    elide: Text.ElideRight
                                    textFormat: Text.PlainText
                                    color: entry.modelData.time ? Theme.ink2 : Theme.err
                                    font.pixelSize: Theme.small
                                    lineHeight: 1.25
                                    text: entry.modelData.text !== "" ? entry.modelData.text : Theme.say("No note written")
                                }
                            }
                        }
                    }
                }
                }
                Btn {
                    visible: screen.changedShown.length > 12
                    text: screen.sinceOpen ? Theme.fill("Show {}", 12) : Theme.fill("{} more", (screen.changedShown.length - 12))
                    onClicked: screen.sinceOpen = !screen.sinceOpen
                }
            }

            Card {
                visible: replaced.count > 0 || screen.ignoredReplacements > 0
                title: Theme.say("Replacements suggested")
                note: Theme.fill("{}, from the Use This Instead list", replaced.count)
                key: "downloads-replaced"
                actions: [
                    Chip {
                        visible: screen.ignoredReplacements > 0
                        pressable: true
                        icon: "eye-off"
                        text: Theme.fill("{} ignored", screen.ignoredReplacements)
                        tip: Theme.say("Suggest them again")
                        onClicked: screen.downloads.suggestReplacementsAgain()
                    }
                ]
                Repeater {
                    id: replaced
                    model: screen.downloads.replacements || []
                    RowLayout {
                        id: old
                        required property var modelData
                        width: page.width
                        height: Theme.sized(46)
                        spacing: Theme.gapLarge
                        Column {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            spacing: Theme.gapHair
                            RowLayout {
                                width: parent.width
                                spacing: Theme.gap
                                Text { Layout.maximumWidth: parent.width - (wasOff.visible ? wasOff.width + Theme.gap : 0); elide: Text.ElideRight; color: Theme.ink2; font.pixelSize: Theme.body; text: old.modelData.name }
                                Chip { id: wasOff; visible: !old.modelData.active; text: Theme.say("inactive") }
                                Item { Layout.fillWidth: true }
                            }
                            RowLayout {
                                width: parent.width
                                spacing: Theme.gap
                                Icon { name: "corner-down-right"; color: Theme.ink3 }
                                Text { Layout.maximumWidth: parent.width - Theme.icon - Theme.gap - (hasNew.visible ? hasNew.width + Theme.gap : 0); elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; font.weight: Font.Medium; text: old.modelData.newName }
                                Chip { id: hasNew; visible: old.modelData.haveNew; tone: Theme.ok; icon: "check"; text: Theme.say("installed") }
                                Item { Layout.fillWidth: true }
                            }
                        }
                        Btn { text: Theme.say("Show old mod"); spoken: Theme.fill("Show {}", old.modelData.name); tip: Theme.say("The installed mod the list would replace"); onClicked: screen.showMod(old.modelData.modId) }
                        Btn { visible: !old.modelData.haveNew; icon: "external-link"; text: Theme.say("Replacement"); tip: Theme.say("Workshop page of the replacement"); onClicked: Theme.openWorkshop(old.modelData.newId) }
                        Btn { icon: "eye-off"; text: Theme.say("Ignore"); spoken: Theme.fill("Ignore the replacement of {}", old.modelData.name); tip: Theme.say("No replacement is suggested for this mod again"); onClicked: screen.downloads.ignoreReplacement(old.modelData.modId) }
                        Btn {
                            danger: true
                            text: Theme.say("Unsubscribe…")
                            spoken: Theme.fill("Unsubscribe from {}", old.modelData.name)
                            tip: Theme.say("Lets the replaced mod go: Steam unsubscribes and deletes its folder")
                            onClicked: Mods.unsubscribe(old.modelData.modId)
                        }
                    }
                }
            }
        }
    }
}
