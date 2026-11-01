pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

ApplicationWindow {
    id: window
    width: Theme.sized(1440)
    height: Theme.sized(860)
    onWidthChanged: if (shotPath === "" && visible) Theme.stored.windowWidth = width
    onHeightChanged: if (shotPath === "" && visible) Theme.stored.windowHeight = height
    readonly property size smallest: Qt.size(Theme.sized(1000), Theme.sized(560))
    minimumWidth: Math.min(Screen.desktopAvailableWidth, smallest.width)
    minimumHeight: Math.min(Screen.desktopAvailableHeight, smallest.height)
    visible: true
    title: Theme.say("RimStack")
    color: Theme.win

    property int section: {
        return Hub.number("--section", 0)
    }

    property bool firstRun: Hub.given("--first")
        || (!Theme.demo && shotPath === "" && Mods.checkMode === "" && !Hub.settings.report.setupDone)

    readonly property var inTheWay: [
        Mods.errors > 0 ? Theme.plural(Mods.errors, "{} mod has a problem the game will trip on", "{} mods have problems the game will trip on") : "",
        Mods.sortPending > 0 && Theme.plural(Mods.sortPending, "Sorting would move {} mod", "Sorting would move {} mods"),
        Mods.dirty ? Theme.say("The list has unsaved changes; the game would start with the saved one") : "",
    ].filter(line => line)
    Connections {
        target: Hub.game
        function onResetByGameChanged() {
            const ids = Hub.game.resetByGame
            if (!window.firstRun)
                Notices.offer(Theme.plural(ids.length, "The game emptied its list of mods, as it does on a new version. The last launch from RimStack had {} mod.", "The game emptied its list of mods, as it does on a new version. The last launch from RimStack had {} mods."), Theme.say("Use that list"), () => window.useList(ids))
        }
        function onOpenSteam(address) { Qt.openUrlExternally(address) }
        function onNotStarted(why) { Notices.say(why, "err") }
        function onWrittenOutside(ids) {
            Notices.offer(Theme.plural(ids.length, "ModsConfig.xml was written outside RimStack and now lists {} mod; the list here has {}.", "ModsConfig.xml was written outside RimStack and now lists {} mods; the list here has {}.", Mods.active.count), Theme.say("Use the file's"), () => Hub.mods.keepFile(true))
        }
    }
    Binding { target: Hub.game; property: "watching"; value: (window.visibility !== Window.Minimized && window.visibility !== Window.Hidden) || Mods.checkMode === "live" }
    Binding { target: Hub.game; property: "held"; value: Hub.mods.rescanning || Hub.mods.previewing || Mods.drag.held !== null || Mods.comparing !== null }
    function pressPlay() {
        if (Hub.game.running)
            return
        if (inTheWay.length > 0)
            topBar.beforePlay.open()
        else
            Hub.game.play(false)
    }

    function useFont(name) {
        const from = Qt.application.font.family
        const to = Hub.settings.useFont(name)
        Theme.refont(window.contentItem, from, to)
        Theme.refont(Overlay.overlay, from, to)
    }

    Component.onCompleted: {
        if (shotPath === "") {
            width = Math.max(minimumWidth, Theme.stored.windowWidth)
            height = Math.max(minimumHeight, Theme.stored.windowHeight)
        } else {
            const given = Hub.argument("--size").split("x")
            if (given.length === 2) {
                width = Number(given[0])
                height = Number(given[1])
            }
        }
        const size = Number(Hub.settings.report.interfaceScale || 1)
        if (size !== 1 && size > 0) {
            Theme.stored.textScale = Math.round(Theme.stored.textScale * size * 20) / 20
            Hub.settings.keep("interfaceScale", "")
        }
    }

    Connections {
        target: Hub.downloads
        function onEnded(note, failed) { Notices.say(note, failed ? "err" : "ok") }
        function onNotesStateChanged() {
            const state = Hub.downloads.notesState || {}
            if (state.refused)
                Notices.say(Theme.plural(state.asked, "Steam's site still refuses: {} mod asked, change notes of {} read", "Steam's site still refuses: {} mods asked, change notes of {} read", state.done), "err")
        }
    }
    Connections {
        target: Hub.settings
        function onReleaseChanged() {
            const found = Hub.settings.release || ({})
            if (found.version)
                Notices.offer(Theme.fill("RimStack {} is published; this is {}", found.version, Hub.settings.report.version), Theme.say("Open its page"), () => Qt.openUrlExternally(found.page))
        }
    }
    readonly property var tasks: [
        Hub.mods.rescanning ? { text: Theme.say("Reading the mod folders…") } : null,
        !Hub.mods.filesRead && Mods.active.count > 0 && !Theme.demo ? { text: Theme.say("Reading the active mods' files…") } : null,
        Hub.textures.busy ? { text: Hub.textures.progress || Theme.say("Counting textures…"), cancel: () => Hub.textures.cancel() } : null,
        Hub.downloads.updatesBusy ? { text: Theme.say("Asking Steam for updates…") } : null,
        Hub.settings.communityBusy ? { text: Theme.say("Downloading community data…") } : null,
        Hub.downloads.taskRunning ? { text: Hub.downloads.taskNote } : null,
    ].filter(task => task !== null)
    Connections {
        target: Mods
        function onShowSection(section) { window.section = section }
        function onFindBrokenRequested() { hunt.start() }
    }

    function showMod(modId) {
        section = 0
        Mods.showMod(modId)
    }

    function useList(ids) {
        Mods.useList(ids)
        section = 0
    }

    Rectangle {
        id: everything
        anchors.fill: parent
        color: Theme.win

        ColumnLayout {
            anchors.fill: parent
            spacing: 0
            enabled: !window.firstRun

            TopBar {
                id: topBar
                Layout.fillWidth: true
                section: window.section
                behind: Hub.downloads.behindActive
                gameRunning: Hub.game.running
                inTheWay: window.inTheWay
                onGo: section => window.section = section
                onSearch: commands.open()
                onPressPlay: window.pressPlay()
                onPlay: Hub.game.play(false)
                onUseList: ids => window.useList(ids)
            }

            HuntBar {
                id: hunt
                Layout.fillWidth: true
                onUseList: ids => Mods.useList(ids, true)
                onSaveAndPlay: Mods.save(() => Hub.game.play(true))
            }

            StackLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                currentIndex: window.section
                ModsScreen { id: modsScreen }
                Loader {
                    active: window.section === 1 || item !== null
                    sourceComponent: DownloadsScreen {
                        onShowMod: modId => window.showMod(modId)
                    }
                }
                Loader {
                    active: window.section === 2 || item !== null
                    sourceComponent: TexturesScreen {}
                }
                Loader {
                    active: window.section === 3 || item !== null
                    sourceComponent: SavesScreen {
                        onUseList: ids => window.useList(ids)
                    }
                }
                Loader {
                    id: logsLoader
                    active: window.section === 4 || item !== null
                    sourceComponent: LogsScreen {
                        onUseList: ids => window.useList(ids)
                        onShowMod: modId => window.showMod(modId)
                        onFindBroken: hunt.start()
                    }
                }
                Loader {
                    id: settingsLoader
                    active: window.section === 5 || item !== null
                    sourceComponent: SettingsScreen {
                        overview: Mods.overview
                        onUseList: ids => window.useList(ids)
                        onTrust: (modId, yes) => Mods.trust(modId, yes)
                        onFont: family => window.useFont(family)
                    }
                }
            }

            StatusBar {
                id: statusBar
                Layout.fillWidth: true
                lastLaunch: Hub.game.lastStart
                tasks: window.tasks
                onUseList: ids => window.useList(ids)
                onKeys: keysSheet.open()
                onLogs: window.section = 4
            }
        }

        Toasts {
            anchors.left: parent.left
            anchors.bottom: parent.bottom
            anchors.leftMargin: Theme.gapCard
            anchors.bottomMargin: statusBar.height + Theme.gapLarge
            z: 20
        }
    }

    readonly property string shotPath: {
        return Hub.argument("--shot")
    }
    Timer {
        interval: {
            return Hub.number("--shot-delay", 2000)
        }
        running: window.shotPath !== ""
        // The window's own picture leaves out popups, so a menu or dialog is grabbed by itself.
        onTriggered: (window.opened ? window.opened.contentItem : everything).grabToImage(result => {
            result.saveToFile(window.shotPath)
            Qt.quit()
        })
    }

    Timer { interval: 4000; running: !Theme.demo && window.shotPath === "" && Mods.checkMode === ""; onTriggered: if (Hub.settings.report.selfCheck) Hub.settings.checkRelease() }

    Commands {
        id: commands
        x: Math.round((window.width - width) / 2)
        y: Theme.sized(90)
        findMods: text => Hub.mods.findMods(text, 12).map(m => ({
            name: m.name, group: m.active ? Theme.say("Active mod") : Theme.say("Inactive mod"), run: () => window.showMod(m.modId) }))
        commands: {
            const folders = Theme.demo ? ({}) : Hub.settings.report
            const open = path => Qt.openUrlExternally(Theme.fileUrl(path))
            const mod = Mods.selectedId >= 0 && Mods.selectedCount === 1 ? Mods.info : null
            const filter = text => { window.section = 0; Mods.filterBy(text) }
            const settings = part => { window.section = 5; Qt.callLater(() => settingsLoader.item.part = part) }
            const logs = view => { window.section = 4; Qt.callLater(() => logsLoader.item.view = view) }
            const found = [
                ["RimWorld folder", 0], ["Config and saves folder", 0], ["Workshop mods folder", 0], ["Play starts the game: through Steam or directly", 0], ["Launch arguments", 0], ["Git for Workshop mods", 0],
                [Theme.say("Theme"), 1], [Theme.say("Language"), 1], [Theme.say("Colours"), 1], [Theme.say("Font"), 1], [Theme.say("Text size"), 1], [Theme.say("Mod rows"), 1], [Theme.say("Motion"), 1], [Theme.say("Contrast"), 1], [Theme.say("First-run hints"), 1], [Theme.say("Workshop pages open in"), 1],
                [Theme.say("Community databases"), 2], [Theme.say("Game version mismatch"), 2], [Theme.say("Patch and def hints"), 2], [Theme.say("Mods marked as working"), 2],
                [Theme.say("Verify game files with Steam"), 3], [Theme.say("Set mod settings aside"), 3], [Theme.say("List from before RimStack"), 3], [Theme.say("RimStack's data folder"), 3], [Theme.say("Version and licence"), 3],
            ].map(setting => ({ name: Theme.say(setting[0]), group: Theme.say("Setting"), run: () => settings(setting[1]) }))
            return found.concat([
                { name: Theme.say("Keyboard shortcuts"), group: Theme.say("Help"), keys: "?", run: () => keysSheet.open() },
                { name: Theme.say("Report a problem"), group: Theme.say("Help"), run: () => Qt.openUrlExternally(Theme.issues) },
                { name: Theme.say("Show the first-run hints again"), group: Theme.say("Help"), run: () => { Theme.stored.hintsSeen = false; window.section = 0; Mods.startHints() } },
                { name: Theme.say("Find the broken mod"), group: Theme.say("Game"), run: () => hunt.start(), when: !Theme.demo && !Hub.search.active },
                { name: Theme.say("All problems"), group: Theme.say("Load order"), run: () => { window.section = 0; Mods.problemsOpen = true }, when: Mods.troubles.length > 0 },
                { name: Theme.say("What RimStack changed"), group: Theme.say("Help"), run: () => statusBar.historyMenu.open(), when: !Theme.demo },
                { name: Theme.say("Sort"), group: Theme.say("Load order"), run: () => { window.section = 0; Mods.previewSort() }, when: !Theme.demo },
                { name: Theme.say("Save"), group: Theme.say("Load order"), keys: Theme.say("Ctrl + S"), run: () => Mods.save(), when: Mods.dirty },
                { name: Theme.say("Undo"), group: Theme.say("Load order"), keys: Theme.say("Ctrl + Z"), run: () => Hub.mods.undo(), when: Hub.mods.canUndo },
                { name: Theme.say("Redo"), group: Theme.say("Load order"), keys: Theme.say("Ctrl + Shift + Z"), run: () => Hub.mods.redo(), when: Hub.mods.canRedo },
                { name: Theme.say("Play"), group: Theme.say("Game"), run: () => window.pressPlay() },
                { name: Theme.say("Next problem"), group: Theme.say("Load order"), run: () => { window.section = 0; Mods.showNextProblem() }, when: Mods.errors + Hub.mods.warnings > 0 },
                { name: Theme.say("Saved lists"), group: Theme.say("Load order"), run: () => topBar.listsMenu.open(), when: !Theme.demo },
                { name: Theme.say("Use the list from before RimStack"), group: Theme.say("Load order"), run: () => { const ids = Hub.lists.original(); if (ids) window.useList(ids) }, when: !!folders.backup },

                { name: Theme.fill(Mods.selectedActive ? "Deactivate {}" : "Activate {}", mod ? mod.name : ""), group: Theme.say("Selected mod"), run: () => Mods.toggleSelected(), when: !!mod && !Hub.mods.previewing },
                { name: Theme.fill("Open folder of {}", (mod ? mod.name : "")), group: Theme.say("Selected mod"), run: () => open(mod.path), when: !!mod && !!mod.path },
                { name: Theme.fill("Workshop page of {}", (mod ? mod.name : "")), group: Theme.say("Selected mod"), run: () => Theme.openWorkshop(mod.folder), when: !!mod && mod.workshop },
                { name: Theme.fill("Copy package id of {}", (mod ? mod.name : "")), group: Theme.say("Selected mod"), run: () => Theme.copy(mod.packageId), when: !!mod && !!mod.packageId },
                { name: Theme.say("Clear selection"), group: Theme.say("Selected mod"), keys: Theme.say("Esc"), run: () => Mods.select(-1, false), when: Mods.selectedId >= 0 },

                { name: Theme.say("Use the list on the clipboard"), group: Theme.say("Load order"), run: () => { const ids = Hub.lists.idsInText(Theme.pasted()); if (ids.length > 0) window.useList(ids) }, when: !Theme.demo },
                { name: Theme.say("Copy the load order as text"), group: Theme.say("Load order"), run: () => Theme.copy(Hub.mods.listing()) },
                { name: Theme.say("Mods with errors"), group: Theme.say("Filter"), run: () => filter("is:error"), when: Mods.errors > 0 },
                { name: Theme.say("Mods with warnings"), group: Theme.say("Filter"), run: () => filter("is:warning"), when: Hub.mods.warnings > 0 },
                { name: Theme.say("Mods sharing textures"), group: Theme.say("Filter"), run: () => filter("is:conflict"), when: Hub.mods.conflicts > 0 },
                { name: Theme.say("Mods with an update"), group: Theme.say("Filter"), run: () => filter("is:updated"), when: (Hub.downloads.updates || {}).count > 0 },
                { name: Theme.say("Mods with code"), group: Theme.say("Filter"), run: () => filter("has:code") },
                { name: Theme.say("Local mods"), group: Theme.say("Filter"), run: () => filter("is:local") },
                { name: Theme.say("Clear filter"), group: Theme.say("Filter"), run: () => filter("") },

                { name: Theme.say("Read the mod folders again"), group: Theme.say("Mods"), run: () => Hub.mods.rescan(), when: !Theme.demo },
                { name: Theme.say("Check for updates"), group: Theme.say("Downloads"), run: () => { window.section = 1; Hub.downloads.checkUpdates() }, when: !Theme.demo },
                { name: Theme.say("Update community rules"), group: Theme.say("Sorting"), run: () => Hub.settings.downloadCommunity(), when: !Theme.demo && !Hub.settings.communityCurrent },
                { name: Theme.say("Read the saves again"), group: Theme.say("Saves"), run: () => { window.section = 3; Hub.saves.reread() }, when: !Theme.demo },
                { name: Theme.say("Read the log again"), group: Theme.say("Logs"), run: () => { window.section = 4; Qt.callLater(() => logsLoader.item.load()) }, when: !Theme.demo },
                { name: Theme.say("Back up the newest save"), group: Theme.say("Saves"), run: () => Hub.saves.backUp(), when: !Theme.demo },

                { name: Theme.say("Mods"), group: Theme.say("Go to"), keys: Theme.fill("Ctrl + {}", 1), run: () => window.section = 0 },
                { name: Theme.say("Downloads"), group: Theme.say("Go to"), keys: Theme.fill("Ctrl + {}", 2), run: () => window.section = 1 },
                { name: Theme.say("Textures"), group: Theme.say("Go to"), keys: Theme.fill("Ctrl + {}", 3), run: () => window.section = 2 },
                { name: Theme.say("Saves"), group: Theme.say("Go to"), keys: Theme.fill("Ctrl + {}", 4), run: () => window.section = 3 },
                { name: Theme.say("Logs: errors"), group: Theme.say("Go to"), run: () => logs(0) },
                { name: Theme.say("Logs: mods"), group: Theme.say("Go to"), run: () => logs(1) },
                { name: Theme.say("Logs: full log"), group: Theme.say("Go to"), run: () => logs(2) },
                { name: Theme.say("Settings: game and folders"), group: Theme.say("Go to"), run: () => settings(0) },
                { name: Theme.say("Settings: appearance"), group: Theme.say("Go to"), run: () => settings(1) },
                { name: Theme.say("Settings: sorting"), group: Theme.say("Go to"), run: () => settings(2) },
                { name: Theme.say("Settings: RimStack"), group: Theme.say("Go to"), run: () => settings(3) },

                { name: Theme.say("Theme: dark"), group: Theme.say("View"), run: () => Theme.stored.theme = "dark", when: Theme.name !== "dark" },
                { name: Theme.say("Theme: OLED"), group: Theme.say("View"), run: () => Theme.stored.theme = "oled", when: Theme.name !== "oled" },
                { name: Theme.say("Theme: light"), group: Theme.say("View"), run: () => Theme.stored.theme = "light", when: Theme.name !== "light" },
                { name: Theme.say("Theme: Rim"), group: Theme.say("View"), run: () => Theme.stored.theme = "rim", when: Theme.name !== "rim" },
                { name: Theme.stored.comfortable ? Theme.say("Compact rows") : Theme.say("Comfortable rows"), group: Theme.say("View"), run: () => Theme.stored.comfortable = !Theme.stored.comfortable },
                { name: Mods.prefs.inactiveHidden ? Theme.say("Show the inactive list") : Theme.say("Hide the inactive list"), group: Theme.say("View"), run: () => Mods.prefs.inactiveHidden = !Mods.prefs.inactiveHidden },
                { name: (Mods.prefs.inactiveByName ? Theme.say("Inactive mods by category") : Theme.say("Inactive mods by name")), group: Theme.say("View"), run: () => Mods.prefs.inactiveByName = !Mods.prefs.inactiveByName },

                { name: Theme.say("RimWorld folder"), group: Theme.say("Open"), run: () => open(folders.game), when: !!folders.game },
                { name: Theme.say("Workshop mods folder"), group: Theme.say("Open"), run: () => open(folders.workshop), when: !!folders.workshop },
                { name: Theme.say("Config and saves folder"), group: Theme.say("Open"), run: () => open(folders.config), when: !!folders.config },
                { name: Theme.say("Game log"), group: Theme.say("Open"), run: () => open(folders.gameLog), when: !!folders.gameLog },
                { name: Theme.say("RimStack data folder"), group: Theme.say("Open"), run: () => open(folders.dataFolder), when: !!folders.dataFolder },
                { name: Theme.say("RimWorld store page"), group: Theme.say("Open"), run: () => Qt.openUrlExternally("steam://store/294100") },
            ])
        }
    }
    Shortcut { sequence: "Ctrl+K"; onActivated: commands.open() }
    Repeater {
        model: topBar.sections.length
        Item {
            id: sectionKey
            required property int index
            Shortcut { sequence: "Ctrl+" + (sectionKey.index + 1); onActivated: window.section = sectionKey.index }
        }
    }
    KeysSheet { id: keysSheet }
    Prompt {
        id: question
        readonly property var asked: Notices.asking || ({ title: "", lines: [], label: "", danger: false })
        onClosed: Notices.asking = null
        Connections {
            target: Notices
            function onAskingChanged() {
                if (Notices.asking !== null)
                    question.open()
            }
        }
        title: asked.title
        Repeater {
            model: question.asked.lines
            Text {
                required property var modelData
                readonly property bool path: !!modelData.path
                width: parent.width
                text: path ? modelData.path : modelData
                wrapMode: path ? Text.WrapAnywhere : Text.WordWrap
                color: path ? Theme.ink : Theme.ink2
                font.pixelSize: path ? Theme.small : Theme.body
                font.family: path ? "monospace" : Qt.application.font.family
            }
        }
        buttons: [
            Btn { text: Theme.say("Cancel"); onClicked: question.close() },
            Btn {
                primary: !question.asked.danger
                danger: !!question.asked.danger
                text: question.asked.label
                onClicked: {
                    const run = question.asked.run
                    question.close()
                    run()
                }
            }
        ]
    }
    Shortcut { sequence: "?"; enabled: !(window.activeFocusItem && window.activeFocusItem.cursorPosition !== undefined); onActivated: keysSheet.open() }
    Shortcut { sequence: "F1"; onActivated: keysSheet.open() }

    Loader {
        parent: everything
        z: 10
        anchors.fill: parent
        active: window.firstRun
        sourceComponent: FirstRun {
            activeCount: Mods.active.count
            inactiveCount: Mods.inactive.count
            problem: Hub.mods.problem
                        onStart: (withData, withRimSort) => {
                Hub.settings.keep("setupDone", "yes")
                if (withData && (!Mods.overview.communityMods || !Mods.overview.worksOn))
                    Hub.settings.downloadCommunity()
                if (withRimSort)
                    Hub.settings.importRimSortRules()
                window.firstRun = false
                Mods.startHints()
            }
        }
    }

    // `--hunt-works` saves the game's settings: give it a copy.
    Timer {
        interval: 1200
        running: window.shotPath !== ""
        onTriggered: {
            if (Hub.given("--notices")) {
                Notices.say(Theme.say("Saved"), "ok")
                Notices.offer(Theme.plural(3, "Deactivated {} mod", "Deactivated {} mods"), Theme.say("Undo"), () => {})
            }
            if (Hub.given("--hints"))
                Mods.hintStep = Hub.number("--hints", 1)
            if (Hub.given("--problems"))
                Mods.problemsOpen = true
            if (Hub.argument("--font") !== "")
                window.useFont(Hub.argument("--font"))
            if (Hub.given("--hunt-works") && Hub.search.active) {
                hunt.answer(false)
                Mods.save()
            }
            const open = Hub.argument("--open")
            window.opened = window.openable[open] || null
            if (window.opened && open !== "ask")
                window.opened.open()
            if (open === "ask")
                Notices.ask({ title: Theme.say("Set the mods' settings aside"), lines: [Theme.say("Every Mod_*.xml is moved out of"), { path: "/home/example/.config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios/Config" }, Theme.say("Mods start from their defaults. Moving the files back restores them.")], label: Theme.say("Set aside"), run: () => {} })
            if (Hub.given("--several")) {
                Mods.select(Mods.nthMod(Mods.active, 14), true)
                for (const at of [15, 16, 19])
                    Mods.selectAlso(Mods.nthMod(Mods.active, at), true)
            }
        }
    }

    readonly property var openable: ({ play: topBar.beforePlay, history: statusBar.historyMenu, keys: keysSheet, search: commands, lists: topBar.listsMenu, view: modsScreen.viewMenu, columns: modsScreen.popMenu, tray: statusBar.trayMenu, ask: question })
    property var opened: null
    property bool readyMarked: false
    onFrameSwapped: {
        if (!readyMarked) {
            readyMarked = true
            Mods.firstFrame()
        }
    }
}
