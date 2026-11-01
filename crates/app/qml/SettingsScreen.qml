pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

Item {
    id: screen
    Appear { of: screen }
    readonly property real labelWidth: Theme.sized(190)

    signal useList(var ids)
    signal trust(int modId, bool yes)
    signal font(string family)
    property var overview: ({})

    readonly property var info: Hub.settings.report
    readonly property string problem: Hub.settings.problem

    function keep(name, value) {
        Hub.settings.keep(name, value)
    }

    property string finding
    function matches(words) {
        const wanted = finding.trim().toLowerCase().split(" ").filter(word => word !== "")
        return wanted.every(word => words.toLowerCase().indexOf(word) !== -1)
    }
    function resetIn(item) {
        for (const child of item.children) {
            if (child.visible && child.changed === true)
                child.reset()
            else if (child.visible)
                resetIn(child)
        }
    }
    function changedIn(item) {
        let count = 0
        for (const child of item.children) {
            if (child.visible && child.changed === true)
                count += 1
            else if (child.visible && child.changed === undefined)
                count += changedIn(child)
        }
        return count
    }

    component Line: Column {
        id: line
        property string title
        property string tip
        property bool changed: false
        signal reset()
        default property alias content: held.data
        readonly property real named: screen.labelWidth
        width: parent.width
        visible: screen.matches(title + " " + tip)
        spacing: Theme.gapLine
        bottomPadding: Theme.gapSmall
        RowLayout {
            width: parent.width
            spacing: Theme.gap
            RowLayout {
                Layout.preferredWidth: line.named
                Layout.minimumWidth: line.named
                Layout.maximumWidth: line.named
                Layout.alignment: Qt.AlignTop
                Layout.topMargin: Theme.gapTiny
                spacing: Theme.gapSmall
                Rectangle { visible: line.changed; implicitWidth: 6; implicitHeight: 6; radius: Math.min(width, height) / 2; color: Theme.moved; Tip { text: Theme.say("Not the default") } }
                Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.body; text: line.title }
            }
            RowLayout { id: held; Layout.fillWidth: true; Layout.minimumWidth: 0; Layout.minimumHeight: Theme.sized(28); spacing: Theme.gap }
            Btn { visible: line.changed; icon: "undo-2"; tip: Theme.say("Back to the default"); onClicked: line.reset() }
        }
        Text {
            id: help
            visible: line.tip !== ""
            x: line.named + Theme.gap
            width: parent.width - x
            wrapMode: Text.WordWrap
            color: Theme.ink3
            font.pixelSize: Theme.small
            lineHeight: 1.2
            text: line.tip
            Tip { text: help.truncated ? line.tip : "" }
        }
    }

    component Folder: Line {
        id: folder
        property string name
        property string inUse
        property string setByHand
        property string note
        readonly property var verdict: (Hub.settings.folderChecks || {})[name]
        ColumnLayout {
            Layout.fillWidth: true
            spacing: Theme.gapThin
            RowLayout {
                Layout.fillWidth: true
                spacing: Theme.gap
                FilterField {
                    id: box
                    Layout.fillWidth: true
                    placeholder: folder.inUse || Theme.say("Not found")
                    text: folder.setByHand
                    onTextChanged: Hub.settings.checkFolder(folder.name, box.text)
                }
                Btn { enabled: folder.inUse !== ""; icon: "folder-open"; tip: Theme.say("Open folder"); onClicked: Qt.openUrlExternally(Theme.fileUrl(folder.inUse)) }
            }
            Text { visible: !!folder.verdict && folder.verdict.ok; color: Theme.ok; font.pixelSize: Theme.small; text: (folder.verdict || { said: "" }).said }
            Text { visible: !!folder.verdict && !folder.verdict.ok; color: Theme.err; font.pixelSize: Theme.small; text: (folder.verdict || { said: "" }).said }
            Text {
                visible: !folder.verdict
                color: folder.inUse ? Theme.ink3 : Theme.err
                font.pixelSize: Theme.small
                text: [folder.inUse ? (folder.setByHand ? Theme.say("Set by hand") : Theme.say("Found automatically")) : Theme.say("Not found"), folder.note].filter(s => s).join(", ")
            }
        }
    }

    property int part: {
        return Hub.number("--part", 0)
    }
    readonly property var parts: [Theme.say("Game and folders"), Theme.say("Appearance"), Theme.say("Sorting"), "RimStack"]

    Rectangle {
        id: nav
        width: Theme.sized(210)
        height: parent.height
        color: Theme.panel
        Column {
            x: Theme.gap
            y: Theme.gapMedium
            width: parent.width - 2 * Theme.gap
            spacing: Theme.gapHair
            FilterField { id: find; width: parent.width; placeholder: Theme.say("Find a setting"); onTextChanged: screen.finding = text }
            Item { width: 1; height: Theme.gapSmall }
            Repeater {
                id: partRows
                model: screen.parts
                Rectangle {
                    id: choice
                    required property string modelData
                    required property int index
                    readonly property bool on: index === screen.part && screen.finding === ""
                    width: parent.width
                    height: Theme.sized(28)
                    radius: Theme.corner
                    color: on ? Theme.accent : choiceHover.hovered ? Theme.chip : "transparent"
                    Accessible.role: Accessible.PageTab
                    Accessible.name: modelData
                    Accessible.onPressAction: choice.pick(index)
                    function pick(to) {
                        if (to < 0 || to >= screen.parts.length)
                            return
                        find.text = ""
                        screen.part = to
                        partRows.itemAt(to).forceActiveFocus()
                    }
                    activeFocusOnTab: on || (screen.finding !== "" && index === 0)
                    Keys.onUpPressed: pick(index - 1)
                    Keys.onDownPressed: pick(index + 1)
                    Keys.onSpacePressed: pick(index)
                    Keys.onReturnPressed: pick(index)
                    FocusRing { anchors.margins: -2 }
                    Text { x: Theme.gapMedium; anchors.verticalCenter: parent.verticalCenter; color: choice.on ? Theme.accentInk : Theme.ink; font.pixelSize: Theme.body; text: Theme.say(choice.modelData) }
                    HoverHandler { id: choiceHover }
                    TapHandler { onTapped: choice.pick(choice.index) }
                }
            }
        }
        Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: Theme.edge }
    }

    Flickable {
        id: flick
        anchors.fill: parent
        anchors.leftMargin: nav.width
        contentHeight: page.y + page.implicitHeight + Theme.gapPage + 2 * Theme.gapCard
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: Scroller {}
        Wheel {}

        Column {
            id: page
            readonly property real gutter: Theme.gapPage + Theme.gapSmall
            width: Math.min(Theme.sized(860), flick.width - 2 * gutter)
            x: Math.max(gutter, (flick.width - width) / 2)
            y: Theme.gapCard
            spacing: Theme.gapSmall

            RowLayout {
                width: parent.width
                Text { Layout.fillWidth: true; color: Theme.ink; font.pixelSize: Theme.heading; font.weight: Font.DemiBold; text: screen.finding !== "" ? Theme.fill("Settings matching “{}”", screen.finding.trim()) : Theme.say(screen.parts[screen.part]) }
                Btn {
                    // Not named `changed`: changedIn reads that property on every child, this button included.
                    readonly property int marked: (screen.info, Theme.stored.theme, Theme.stored.comfortable, Theme.stored.largeRows, Theme.stored.workshopInSteam, Theme.stored.textScale, Theme.stored.reduceMotion, Theme.stored.highContrast, Theme.stored.ownColours, screen.part, screen.finding, screen.changedIn(page))
                    opacity: marked > 0 ? 1 : 0
                    enabled: marked > 0
                    icon: "undo-2"
                    text: Theme.say("Reset this page") + "  " + marked
                    tip: Theme.say("Every marked setting on this page goes back to its default")
                    onClicked: screen.resetIn(page)
                }
            }
            Text {
                visible: screen.problem !== ""
                width: parent.width
                wrapMode: Text.WordWrap
                color: Theme.err
                font.pixelSize: Theme.body
                text: Theme.fill("Not kept: {}", screen.problem)
            }

            Column {
                width: parent.width
                spacing: Theme.gapSmall
                visible: screen.part === 0 || screen.finding !== ""
                GroupTitle { visible: screen.finding === ""; text: Theme.say("Folders") }
                Text { visible: screen.finding === ""; width: parent.width; bottomPadding: Theme.gapSmall; wrapMode: Text.WordWrap; color: Theme.ink3; font.pixelSize: Theme.small; text: Theme.say("Found through Steam. Type a folder to use another; empty the box to find it again.") }
                Folder { title: Theme.say("RimWorld"); note: screen.info.gameVersion ? Theme.fill("version {}", screen.info.gameVersion) : ""; name: "game"; inUse: screen.info.game || ""; setByHand: screen.info.setGame || "" }
                Folder { title: Theme.say("Config and saves"); name: "config"; inUse: screen.info.config || ""; setByHand: screen.info.setConfig || "" }
                Folder { title: Theme.say("Workshop mods"); name: "workshop"; inUse: screen.info.workshop || ""; setByHand: screen.info.setWorkshop || "" }

                GroupTitle { visible: screen.finding === ""; text: Theme.say("Play") }
                Line {
                    title: Theme.say("Play starts the game")
                    changed: !!screen.info.launchDirectly
                    onReset: screen.keep("launch", "steam")
                    tip: Theme.say("Directly runs the game's own program from its folder, for a copy that is not Steam's.")
                    Choice {
                        title: Theme.say("Play starts the game")
                        options: ["Through Steam", "Directly"]
                        current: screen.info.launchDirectly ? 1 : 0
                        onPicked: (index) => screen.keep("launch", index === 1 ? "direct" : "steam")
                    }
                    Item { Layout.fillWidth: true }
                }
                Line {
                    title: Theme.say("Launch arguments")
                    changed: (screen.info.launchArguments || "") !== ""
                    onReset: screen.keep("launchArguments", "")
                    tip: Theme.say("Given to the game when Play starts it, as in Steam's own launch options: -logfile, -popupwindow, -savedatafolder=…")
                    FilterField {
                        id: launchArguments
                        Layout.fillWidth: true
                        placeholder: Theme.say("None")
                        text: screen.info.launchArguments || ""
                        onTextChanged: Hub.settings.keepLater("launchArguments", launchArguments.text)
                    }
                }

                GroupTitle { visible: screen.finding === ""; text: Theme.say("Load order") }
                Line {
                    title: Theme.say("File")
                    tip: Theme.say("The game's list of active mods. Save writes it.")
                    PathText { path: screen.info.modsConfig || Theme.say("Not found"); tone: screen.info.modsConfig ? Theme.ink : Theme.err; copyable: true }
                }
                Line {
                    title: Theme.say("Copies")
                    tip: Theme.say("Before each Save the file as it was is copied beside itself to .rimstack-backup, and the first time also to .rimstack-original.")
                    Text { Layout.fillWidth: true; color: screen.info.backup ? Theme.ink : Theme.ink3; font.pixelSize: Theme.body; text: screen.info.backup ? ".rimstack-backup, .rimstack-original" : Theme.say("None yet") }
                }

                GroupTitle { visible: screen.finding === ""; text: Theme.say("Updates") }
                Line {
                    title: Theme.say("Git")
                    tip: Theme.say("A mod that is a Git checkout is asked for updates, and updated, with the git program on this computer.")
                    Text { Layout.fillWidth: true; color: screen.info.git ? Theme.ink : Theme.err; font.pixelSize: Theme.body; text: screen.info.git ? Theme.fill("Installed, version {}", screen.info.git) : Theme.say("Not installed: Git checkouts cannot be checked or updated") }
                }
                Line {
                    title: Theme.say("Git for Workshop mods")
                    tip: Theme.say("Check now asks Steam about a Workshop mod, and Git about a checkout in the game's Mods folder. On: a Workshop mod that is a Git checkout is asked through Git as well, which fetches into its folder.")
                    changed: !!screen.info.gitForWorkshop
                    onReset: screen.keep("gitForWorkshop", "off")
                    Toggle {
                        title: Theme.say("Git for Workshop mods")
                        on: !!screen.info.gitForWorkshop
                        onFlipped: on => screen.keep("gitForWorkshop", on ? "on" : "off")
                    }
                    Item { Layout.fillWidth: true }
                }
            }

            Column {
                width: parent.width
                spacing: Theme.gapSmall
                visible: screen.part === 1 || screen.finding !== ""
                GroupTitle { visible: screen.finding === ""; text: Theme.say("Window") }
                Line {
                    title: Theme.say("Theme")
                    changed: Theme.stored.theme !== "rim"
                    onReset: Theme.stored.theme = "rim"
                    Choice {
                        title: Theme.say("Theme")
                        options: Theme.themeLabels
                        current: Math.max(0, Theme.themeNames.indexOf(Theme.stored.theme))
                        onPicked: (index) => Theme.stored.theme = Theme.themeNames[index]
                    }
                    Item { Layout.fillWidth: true }
                }
                Line {
                    id: language
                    property string said
                    function use(name) {
                        Hub.settings.useLanguage(name)
                        // Mods' own lines are in the language too, so rescan rewrites them.
                        if (!Theme.demo)
                            Hub.mods.rescan()
                    }
                    title: Theme.say("Language")
                    tip: Theme.say("A file of the same name beside the settings stands over a language, text by text; one left empty there stays as it is.")
                    readonly property int translated: Object.keys(Theme.words).filter(text => Theme.words[text] !== "").length
                    function file(name) {
                        const made = Hub.settings.startLanguage(name)
                        language.said = made.problem || ""
                        return made.file || ""
                    }
                    Btn {
                        id: languageButton
                        menu: true
                        text: screen.info.language ? screen.info.languageInUse : Theme.fill("{} (system)", screen.info.languageInUse)
                        onClicked: {
                            Mods.menuRequested(languageButton, 0, languageButton.height + 2,
                                [{ text: Theme.say("System language"), run: () => language.use("") }].concat(
                                    Hub.settings.languages.map(name => ({ text: name, run: () => language.use(name) }))))
                        }
                    }
                    Chip {
                        visible: !screen.info.english
                        tone: language.translated > 0 ? Theme.ink2 : Theme.warn
                        text: language.translated > 0 ? Theme.fill("{} texts translated", Theme.count(language.translated)) : Theme.say("Not translated yet")
                    }
                    Btn {
                        icon: "external-link"
                        text: Theme.say("Edit translations")
                        tip: Theme.say("Opens your corrections to this language: a wording saved there replaces RimStack's at once")
                        onClicked: { const found = language.file(screen.info.languageInUse); if (found !== "") Qt.openUrlExternally(Theme.fileUrl(found)) }
                    }
                    Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.err; font.pixelSize: Theme.body; text: language.said }
                }
                Line {
                    id: ownTheme
                    property string said
                    readonly property string themeLabel: Theme.themeLabels[Math.max(0, Theme.themeNames.indexOf(Theme.name))]
                    title: Theme.say("Colours")
                    changed: Theme.stored.ownColours
                    onReset: Theme.stored.ownColours = false
                    tip: Theme.say("Your own colours for the chosen theme, read from theme.json in RimStack's settings folder. Edit theme.json opens it; a colour saved there shows at once.")
                    Toggle {
                        title: ownTheme.title
                        on: Theme.stored.ownColours
                        onFlipped: on => Theme.stored.ownColours = on
                    }
                    Chip { text: Object.keys(Theme.custom).length > 0 ? Theme.fill("{} for {}", Object.keys(Theme.custom).length, ownTheme.themeLabel) : Theme.fill("None for {}", ownTheme.themeLabel) }
                    Chip {
                        readonly property var short: {
                            const shown = {}
                            for (const name of ["win", "panel", "bar", "ink", "ink2", "ink3", "accent", "accentInk", "focus", "fieldLine", "warn", "err", "ok"])
                                shown[name] = Theme.colour(name)
                            return Hub.settings.shortContrast(shown)
                        }
                        visible: Theme.ownInUse && short.length > 0
                        tone: Theme.warn
                        icon: "triangle-alert"
                        text: Theme.plural(short.length, "{} pair under its contrast", "{} pairs under their contrast")
                        tip: short.map(pair => Theme.fill("{} {}, needs {}", Theme.say(pair.what), pair.found.toFixed(1), pair.needed)).join("\n")
                    }
                    Btn {
                        icon: "external-link"
                        text: Theme.say("Edit theme.json")
                        tip: Theme.say("Opens the file, written with this theme's colours when it has none for it; what is saved there shows at once")
                        onClicked: {
                            const now = {}
                            for (const name of Theme.colourNames)
                                now[name] = "" + Theme[name]
                            const made = Hub.settings.startOwnTheme(now)
                            ownTheme.said = made.problem || ""
                            if (ownTheme.said === "")
                                Qt.openUrlExternally(Theme.fileUrl(made.file))
                        }
                    }
                    Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.err; font.pixelSize: Theme.body; text: ownTheme.said }
                }
                Line {
                    title: Theme.say("Font")
                    changed: (screen.info.font || "") !== ""
                    onReset: screen.font("")
                    Btn {
                        id: fontButton
                        menu: true
                        text: screen.info.font || Theme.say("System font")
                        onClicked: fontPicker.visible = !fontPicker.visible
                    }
                    Popup {
                        enter: PopIn {}
                        exit: PopOut {}
                        id: fontPicker
                        readonly property var shown: [""].concat(Qt.fontFamilies()).filter(name => (name || Theme.say("System font")).toLowerCase().indexOf(fontFilter.text.toLowerCase()) !== -1)
                        parent: fontButton
                        y: fontButton.height + 4
                        width: Theme.sized(320)
                        height: Theme.sized(380)
                        padding: Theme.gap
                        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutsideParent
                        background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }
                        onOpened: { fontFilter.text = ""; fontFilter.focusInput() }
                        contentItem: ColumnLayout {
                            spacing: Theme.gapSmall
                            FilterField { id: fontFilter; Layout.fillWidth: true; placeholder: Theme.say("Find a font") }
                            ListView {
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                clip: true
                                model: fontPicker.shown
                                Wheel {}
                                ScrollBar.vertical: Scroller {}
                                delegate: Rectangle {
                                    id: family
                                    required property string modelData
                                    readonly property bool system: !modelData
                                    width: ListView.view.width
                                    height: Theme.sized(30)
                                    radius: Theme.corner
                                    color: familyHover.hovered ? Theme.accent : "transparent"
                                    Text {
                                        x: Theme.gap
                                        anchors.verticalCenter: parent.verticalCenter
                                        width: parent.width - 2 * Theme.gap
                                        elide: Text.ElideRight
                                        color: familyHover.hovered ? Theme.accentInk : Theme.ink
                                        font.pixelSize: Theme.title
                                        font.family: family.system ? Qt.application.font.family : family.modelData
                                        text: family.system ? Theme.say("System font") : family.modelData
                                    }
                                    HoverHandler { id: familyHover }
                                    TapHandler {
                                        onTapped: {
                                            screen.font(family.modelData)
                                            fontPicker.close()
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Item { Layout.fillWidth: true }
                }
                Line {
                    title: Theme.say("Text size")
                    tip: Theme.say("Resizes every text at once; rows, buttons and bars grow with it.")
                    changed: Theme.stored.textScale !== 1
                    onReset: Theme.stored.textScale = 1
                    Slide { id: textSize; title: Theme.say("Text size"); reading: Math.round(shown * 100) + "%"; from: 0.85; to: 1.6; step: 0.05; value: Theme.stored.textScale; onMoved: value => Theme.stored.textScale = Math.round(value * 100) / 100 }
                    Text { color: Theme.ink2; font.pixelSize: Theme.body; text: textSize.reading }
                    Item { Layout.fillWidth: true }
                }
                Line {
                    title: Theme.say("Mod rows")
                    changed: !Theme.stored.comfortable || Theme.stored.largeRows
                    onReset: { Theme.stored.comfortable = true; Theme.stored.largeRows = false }
                    tip: Theme.say("Roomy: more space around each row. With pictures: taller rows in the load order, each with the mod's picture.")
                    Choice {
                        title: Theme.say("Mod rows")
                        options: ["Compact", "Roomy", "With pictures"]
                        current: Theme.stored.comfortable ? 2 : Theme.stored.largeRows ? 1 : 0
                        onPicked: (index) => { Theme.stored.comfortable = index === 2; Theme.stored.largeRows = index === 1 }
                    }
                    Item { Layout.fillWidth: true }
                }
                GroupTitle { visible: screen.finding === ""; text: Theme.say("Ease of use") }
                Line {
                    title: Theme.say("Motion")
                    tip: Theme.say("Reduced: nothing slides, fades or spins; changes show at once.")
                    changed: Theme.stored.reduceMotion
                    onReset: Theme.stored.reduceMotion = false
                    Choice { title: Theme.say("Motion"); options: ["Full", "Reduced"]; current: Theme.stored.reduceMotion ? 1 : 0; onPicked: (index) => Theme.stored.reduceMotion = index === 1 }
                    Item { Layout.fillWidth: true }
                }
                Line {
                    title: Theme.say("Contrast")
                    tip: Theme.say("High: grey text and thin lines are drawn stronger, and panels get an outline.")
                    changed: Theme.stored.highContrast
                    onReset: Theme.stored.highContrast = false
                    Choice { title: Theme.say("Contrast"); options: ["Normal", "High"]; current: Theme.stored.highContrast ? 1 : 0; onPicked: (index) => Theme.stored.highContrast = index === 1 }
                    Item { Layout.fillWidth: true }
                }
                Line {
                    title: Theme.say("First-run hints")
                    tip: Theme.say("Three notes on the Mods screen, about problems, Sort and Save.")
                    Btn { text: Theme.say("Show again"); onClicked: { Theme.stored.hintsSeen = false; Mods.startHints(); Notices.say(Theme.say("The hints show on the Mods screen")) } }
                    Item { Layout.fillWidth: true }
                }
                GroupTitle { visible: screen.finding === ""; text: Theme.say("Links") }
                Line {
                    title: Theme.say("Workshop pages open in")
                    changed: Theme.stored.workshopInSteam
                    onReset: Theme.stored.workshopInSteam = false
                    Choice {
                        title: Theme.say("Workshop pages open in")
                        options: ["Browser", "Steam"]
                        current: Theme.stored.workshopInSteam ? 1 : 0
                        onPicked: (index) => Theme.stored.workshopInSteam = index === 1
                    }
                    Item { Layout.fillWidth: true }
                }
            }

            Column {
                width: parent.width
                spacing: Theme.gapSmall
                visible: screen.part === 2 || screen.finding !== ""
                GroupTitle { visible: screen.finding === ""; text: Theme.say("Sort follows") }
                Line {
                    title: Theme.say("Community rules")
                    changed: !screen.info.communityRules
                    onReset: screen.keep("communityRules", "on")
                    Toggle {
                        title: Theme.say("Community databases used by Sort")
                        on: !!screen.info.communityRules
                        onFlipped: on => screen.keep("communityRules", on ? "on" : "off")
                    }
                    Item { Layout.fillWidth: true }
                }
                Line {
                    title: Theme.say("Patch and def hints")
                    changed: !screen.info.fileHints
                    onReset: screen.keep("fileHints", "on")
                    tip: Theme.say("A mod whose patches or defs point at another mod's defs. The game applies patches after every mod's defs are loaded, so these are hints, not requirements.")
                    Choice {
                        title: Theme.say("Patch and def hints")
                        options: ["Shown only", "Used by Sort"]
                        current: screen.info.fileHints ? 1 : 0
                        onPicked: (index) => screen.keep("fileHints", index === 1 ? "on" : "off")
                    }
                    Item { Layout.fillWidth: true }
                }

                GroupTitle { visible: screen.finding === ""; text: Theme.say("Community databases") }
                Line {
                    title: Theme.say("Downloaded")
                    tip: Theme.say("From GitHub, on request: Community Rules Database, No Version Warning, Use This Instead, Steam Workshop Database (49 MB).")
                    Text {
                        color: screen.overview.communityMods ? Theme.ink : Theme.ink3
                        font.pixelSize: Theme.body
                        text: screen.overview.communityMods
                            ? Theme.plural(screen.overview.communityMods, "{} mod, published {}", "{} mods, published {}", Theme.day(screen.overview.communityTime))
                            : Theme.say("Not downloaded · 49 MB")
                    }
                    Btn {
                        enabled: !Hub.settings.communityBusy && !Hub.settings.communityCurrent
                        icon: Hub.settings.communityCurrent ? "check" : "download"
                        text: Hub.settings.communityBusy ? Theme.say("Downloading…") : Hub.settings.communityCurrent ? Theme.say("Up to date") : screen.overview.communityMods ? Theme.say("Update") : Theme.say("Download")
                        onClicked: Hub.settings.downloadCommunity()
                    }
                    Item { Layout.fillWidth: true }
                }
                Repeater {
                    model: Hub.settings.databases
                    Line {
                        id: database
                        required property var modelData
                        title: modelData.name
                        Toggle {
                            title: database.modelData.name
                            on: database.modelData.on
                            silent: database.modelData.waiting
                            dimmed: database.modelData.waiting
                            onFlipped: on => Hub.settings.useDatabase(database.modelData.id, on)
                        }
                        Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.body; text: database.modelData.use }
                        Text { visible: database.modelData.updated > 0; color: Theme.ink2; font.pixelSize: Theme.small; text: Theme.day(database.modelData.updated) }
                    }
                }
                Text {
                    visible: Hub.settings.communityProblem !== ""
                    x: screen.labelWidth + Theme.gap
                    width: parent.width - x
                    wrapMode: Text.WordWrap
                    color: Theme.err
                    font.pixelSize: Theme.body
                    text: Theme.fill("Not updated: {}", Hub.settings.communityProblem)
                }

                GroupTitle { visible: rimsort.visible && screen.finding === ""; text: Theme.say("RimSort") }
                Line {
                    id: rimsort
                    property string said
                    visible: !!screen.info.rimsortRules
                    title: Theme.say("Your rules in RimSort")
                    tip: Theme.say("Copies the rules you made in RimSort into RimStack. RimSort's file is only read.")
                    Btn { icon: "download"; text: Theme.say("Import"); onClicked: rimsort.said = Hub.settings.importRimSortRules() }
                    Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.body; text: rimsort.said }
                }

                GroupTitle { visible: screen.finding === ""; text: Theme.say("Game version") }
                Line {
                    title: Theme.say("Game version mismatch")
                    id: versionLine
                    readonly property string game: screen.info.gameVersion || ""
                    readonly property bool aside: game !== "" && screen.info.versionAsideFor === game
                    changed: !screen.info.versionWarnings || aside
                    onReset: version(0)
                    function version(choice) {
                        Hub.settings.versionWarnings(choice)
                    }
                    tip: Theme.say("A mod that does not list the installed game version. For this version: the warnings return when the game is updated. One mod can be excused from its menu.")
                    Choice {
                        title: Theme.say("Game version mismatch")
                        options: ["Warn", "Ignore for this version", "Ignore"]
                        current: !screen.info.versionWarnings ? 2 : versionLine.aside ? 1 : 0
                        onPicked: (index) => versionLine.version(index)
                    }
                    Item { Layout.fillWidth: true }
                }
                NameList {
                    width: parent.width
                    title: Theme.fill("Marked as working on RimWorld {}", (screen.info.gameVersion || ""))
                    lines: screen.overview.trusted || []
                    action: Theme.say("Remove")
                    onActed: (id) => {
                        if (id !== null)
                            screen.trust(id, false)
                    }
                }
            }

            Column {
                width: parent.width
                spacing: Theme.gapSmall
                visible: screen.part === 3 || screen.finding !== ""
                GroupTitle { visible: screen.finding === ""; text: Theme.say("When the game misbehaves") }
                Line {
                    title: Theme.say("Game files")
                    tip: Theme.say("Steam compares every file of the game with its own copy and downloads what differs.")
                    Btn { text: Theme.say("Verify with Steam"); onClicked: Qt.openUrlExternally("steam://validate/294100") }
                    Item { Layout.fillWidth: true }
                }
                Line {
                    id: modSettings
                    property string said
                    title: Theme.say("Mod settings")
                    tip: Theme.say("Moves every Mod_*.xml of the game's Config folder into a dated folder of RimStack's. Mods start from their defaults.")
                    Btn {
                        danger: true
                        text: Theme.say("Set aside…")
                        onClicked: Notices.ask({
                            title: Theme.say("Set the mods' settings aside"),
                            lines: [Theme.say("Every Mod_*.xml is moved out of"), { path: screen.info.configFolder || "" }, Theme.say("into a new folder, named mod-settings and the time, in"), { path: screen.info.dataFolder || "" }, Theme.say("Mods start from their defaults. Moving the files back restores them.")],
                            label: Theme.say("Set aside"), run: () => modSettings.said = Hub.settings.setModSettingsAside() })
                    }
                    Text { Layout.fillWidth: true; elide: Text.ElideMiddle; color: Theme.ink2; font.pixelSize: Theme.body; text: modSettings.said }
                }

                GroupTitle { visible: screen.finding === ""; text: Theme.say("Load order") }
                Line {
                    title: Theme.say("List from before RimStack")
                    tip: Theme.say("The load order the game had before RimStack first saved, kept in ModsConfig.xml.rimstack-original. Using it changes the list here; Save writes it.")
                    Btn {
                        enabled: !!screen.info.backup
                        text: Theme.say("Use")
                        onClicked: {
                            const ids = Hub.lists.original()
                            if (ids)
                                screen.useList(ids)
                        }
                    }
                    Text { visible: !screen.info.backup; color: Theme.ink3; font.pixelSize: Theme.body; text: Theme.say("None yet") }
                    Item { Layout.fillWidth: true }
                }

                GroupTitle { visible: screen.finding === ""; text: Theme.say("Files") }
                Line {
                    title: Theme.say("Logs kept")
                    changed: screen.info.logsKept !== 10
                    onReset: screen.keep("logsKept", "10")
                    tip: Theme.say("The game keeps the logs of its last two launches. RimStack copies each log once the game has stopped, so an older launch can still be read in Logs; the oldest give way.")
                    Choice {
                        readonly property var limits: [0, 5, 10, 25, 50]
                        title: Theme.say("Logs kept")
                        options: [Theme.say("None"), "5", "10", "25", "50"]
                        current: Math.max(0, limits.indexOf(screen.info.logsKept))
                        onPicked: index => screen.keep("logsKept", String(limits[index]))
                    }
                    Text { Layout.fillWidth: true; color: Theme.ink2; font.pixelSize: Theme.small; text: Theme.plural(screen.info.logsKeptNow || 0, "{} kept now", "{} kept now") }
                }
                Line {
                    title: Theme.say("Downloads and records")
                    PathText { path: screen.info.dataFolder || ""; copyable: true; openTip: Theme.say("Open folder") }
                }
                Line {
                    title: Theme.say("Settings")
                    PathText { path: screen.info.settingsFile || ""; copyable: true }
                }
                Line {
                    title: Theme.say("Diagnostics")
                    PathText { path: screen.info.diagnosticsFile || ""; copyable: true; openIcon: "scroll-text"; openTip: Theme.say("Open file") }
                }

                GroupTitle { visible: screen.finding === ""; text: Theme.say("About") }
                Line {
                    id: newer
                    readonly property var found: Hub.settings.release || null
                    title: Theme.say("Version")
                    CopyText { text: screen.info.version || "" }
                    Btn { enabled: !Hub.settings.releaseBusy; text: Hub.settings.releaseBusy ? Theme.say("Asking GitHub…") : Theme.say("Look now"); onClicked: Hub.settings.checkRelease() }
                    Btn { visible: !!newer.found && !!newer.found.version; primary: true; icon: "external-link"; text: newer.found && newer.found.version ? Theme.fill("Get {}", newer.found.version) : ""; onClicked: Qt.openUrlExternally(newer.found.page) }
                    Text { Layout.fillWidth: true; elide: Text.ElideRight; color: newer.found && newer.found.problem ? Theme.err : Theme.ink2; font.pixelSize: Theme.small; text: !newer.found ? "" : newer.found.problem ? newer.found.problem : newer.found.version ? "" : Theme.say("No newer version is published") }
                }
                Line {
                    title: Theme.say("Look for a new version at each start")
                    changed: !screen.info.selfCheck
                    onReset: screen.keep("selfCheck", "on")
                    tip: Theme.say("On: at each start RimStack asks GitHub whether a newer version is published. It installs nothing by itself.")
                    Toggle { title: Theme.say("Look for a new version at each start"); on: !!screen.info.selfCheck; onFlipped: on => screen.keep("selfCheck", on ? "on" : "off") }
                    Item { Layout.fillWidth: true }
                }
                Line {
                    title: Theme.say("Versions to look for")
                    changed: !!screen.info.nightlyUpdates
                    onReset: screen.keep("updates", "stable")
                    tip: Theme.say("Stable: a numbered release. Nightly: the newest build of the main branch, which is made often and may break. Nightly compares the commit this program was built from.")
                    Choice {
                        title: Theme.say("Versions to look for")
                        options: ["Stable", "Nightly"]
                        current: Number(!!screen.info.nightlyUpdates)
                        onPicked: (index) => { screen.keep("updates", ["stable", "nightly"][index]); Hub.settings.checkRelease() }
                    }
                    Item { Layout.fillWidth: true }
                }
                Line {
                    title: Theme.say("Source and issues")
                    Btn { icon: "external-link"; text: Theme.say("Report a problem"); tip: Theme.issues; onClicked: Qt.openUrlExternally(Theme.issues) }
                    Item { Layout.fillWidth: true }
                }
                Line { title: Theme.say("Licence"); Text { Layout.fillWidth: true; color: Theme.ink; font.pixelSize: Theme.body; text: Theme.say("GNU General Public License, version 3 or later") } }
                Line { title: Theme.say("Icons"); Text { Layout.fillWidth: true; color: Theme.ink; font.pixelSize: Theme.body; text: Theme.say("Lucide, ISC licence") } }
            }
        }
    }
}
