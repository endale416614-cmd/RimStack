pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtQuick.Dialogs
import RimStack.Ui

Item {
    id: activePane
    readonly property bool tight: width < Theme.sized(640)
    readonly property bool stepping: Mods.problemPlace > 0 && Mods.selectedId === Mods.problemMod
    function moreEntries() {
        const entries = []
        if (Mods.problemCount > 0) {
            entries.push({ text: Theme.say("Next problem"), icon: "chevron-down", run: () => Mods.showProblem(true) })
            entries.push({ text: Theme.say("Previous problem"), icon: "chevron-up", run: () => Mods.showProblem(false) })
            entries.push({ text: Theme.say("All problems"), icon: "triangle-alert", run: () => Mods.problemsOpen = true })
        }
        if (warningsShown > 0)
            entries.push({ text: Theme.plural(warningsShown, "{} warning", "{} warnings"), icon: "funnel", run: () => toggleTerm("is:warning") })
        if (Hub.mods.conflicts > 0)
            entries.push({ text: Theme.fill("{} share textures", Hub.mods.conflicts), icon: "image", run: () => toggleTerm("is:conflict") })
        if (Hub.downloads.behindActive > 0)
            entries.push({ text: Theme.plural(Hub.downloads.behindActive, "{} update", "{} updates"), icon: "download", run: () => Mods.showSection(1) })
        if (entries.length > 0)
            entries.push({ line: true })
        entries.push({ text: Theme.say("Undo (Ctrl + Z)"), icon: "undo-2", off: !Hub.mods.canUndo || Hub.mods.previewing, run: Hub.mods.undo })
        entries.push({ text: Theme.say("Redo (Ctrl + Shift + Z)"), icon: "redo-2", off: !Hub.mods.canRedo || Hub.mods.previewing, run: Hub.mods.redo })
        return entries
    }
    readonly property int versionOnly: Mods.overview.versionOnly || 0
    readonly property bool versionLine: versionOnly >= 3 && !Hub.mods.previewing && !Theme.demo
    readonly property int otherWarnings: Mods.overview.otherWarnings || 0
    readonly property int warningsShown: versionLine ? otherWarnings : Hub.mods.warnings
    readonly property alias list: view

    function toggleTerm(term) {
        const now = filter.terms.indexOf(term) >= 0
        const others = filter.text.split(" ").filter(word => word !== "" && word !== term)
        filter.text = (now ? others : others.concat([term])).join(" ")
    }

    // "is:changed" narrows to what is not saved yet: with nothing unsaved it would
    // empty both lists.
    Connections {
        target: Hub.mods
        function onDirtyChanged() {
            if (!Hub.mods.dirty && filter.terms.indexOf("is:changed") >= 0)
                activePane.toggleTerm("is:changed")
        }
    }
    Connections {
        target: Mods
        function onFilterRequested(text) { filter.text = text }
        function onFilterFocusRequested() { filter.focusInput() }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        visible: Hub.mods.problem === ""
        RowLayout {
            z: 6
            Layout.leftMargin: Theme.gapCard
            Layout.rightMargin: Theme.gapLarge
            Layout.preferredHeight: Theme.header
            spacing: Theme.gap
            Text { color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: Theme.say("Active") }
            Text {
                color: Theme.ink3
                font.pixelSize: Theme.body
                text: Theme.count(Mods.active.count)
            }
            component Count: Chip {
                property string term
                pressable: true
                border.width: filter.terms.indexOf(term) >= 0 || activeFocus ? 2 : 0
                border.color: activeFocus ? Theme.focus : tone
                onClicked: activePane.toggleTerm(term)
            }
            Count { id: warningsChip; visible: activePane.warningsShown > 0 && !activePane.tight; term: "is:warning"; tone: Theme.warn; icon: "triangle-alert"; text: activePane.versionLine ? Theme.plural(activePane.warningsShown, "{} more warning", "{} more warnings") : Theme.plural(activePane.warningsShown, "{} warning", "{} warnings"); tip: Theme.say("Mods with a warning: the game runs, something about them may not. Click to show only these.") }
            Row {
                visible: Mods.problemCount > 0 && !activePane.tight
                spacing: Theme.gapLine
                Btn {
                    readonly property bool onlyWarnings: Hub.mods.errors === 0
                    visible: activePane.stepping || !(onlyWarnings && activePane.warningsShown > 0 && activePane.warningsShown === Mods.problemCount)
                    icon: "triangle-alert"
                    tone: Hub.mods.errors > 0 ? Theme.err : Theme.warn
                    text: activePane.stepping ? Theme.fill("Problem {} of {}", Mods.problemPlace, Theme.count(Mods.problemCount))
                        : !onlyWarnings ? Theme.plural(Mods.problemCount, "{} problem · {} with errors", "{} problems · {} with errors", Theme.count(Hub.mods.errors))
                        : Theme.plural(Mods.problemCount, "{} warning", "{} warnings")
                    tip: Theme.say("Opens the list of every problem, with the fix for each")
                    onClicked: Mods.problemsOpen = true
                    Hint { step: 1; text: Theme.say("The arrows beside it go from one mod with a problem to the next. Its details say what is wrong and offer the fix.") }
                }
                Btn { icon: "chevron-up"; spoken: Theme.say("Previous problem"); tip: Theme.say("Previous problem"); onClicked: Mods.showProblem(false) }
                Btn { icon: "chevron-down"; spoken: Theme.say("Next problem"); tip: Theme.say("Next problem"); onClicked: Mods.showProblem(true) }
            }
            Item { Layout.fillWidth: true }
            Btn {
                visible: Hub.downloads.behindActive > 0 && !activePane.tight
                text: Theme.plural(Hub.downloads.behindActive, "{} update", "{} updates")
                tip: Theme.plural(Hub.downloads.behindActive, "{} active Workshop mod is older than its Workshop version", "{} active Workshop mods are older than their Workshop version")
                onClicked: Mods.showSection(1)
            }
            Row {
                visible: !Theme.demo && !activePane.tight
                spacing: Theme.gapLine
                Btn { enabled: Hub.mods.canUndo && !Hub.mods.previewing; icon: "undo-2"; tip: Theme.say("Undo (Ctrl + Z)"); onClicked: Hub.mods.undo() }
                Btn { enabled: Hub.mods.canRedo && !Hub.mods.previewing; icon: "redo-2"; tip: Theme.say("Redo (Ctrl + Shift + Z)"); onClicked: Hub.mods.redo() }
            }
            Btn {
                id: sortButton
                tone: Theme.moved
                visible: !Theme.demo; enabled: !Hub.mods.previewing && Hub.mods.filesRead; icon: "arrow-up-down"; primary: Mods.sortPending > 0 && enabled; text: !Hub.mods.filesRead ? Theme.say("Reading files…") : Mods.sortPending > 0 ? Theme.plural(Mods.sortPending, "Sort · {} move", "Sort · {} moves") : Theme.say("Sort"); tip: Theme.say("Shows the order the rules ask for, to keep or discard. Nothing is written before Save."); onClicked: Mods.previewSort()
                Hint { step: 2; atRight: true; text: Theme.say("Sort shows the order the mods' rules ask for and what would move. Apply moves the mods; Discard leaves the list as it was.") }
            }
            Btn { visible: !Theme.demo; enabled: !Hub.mods.previewing && Mods.active.count > 0; icon: "x"; text: Theme.say("Clear"); tip: Theme.say("Deactivates every mod but the game and its DLCs. Nothing is written before Save."); onClicked: { Hub.mods.clearActive(); Mods.select(-1, false) } }
            // Beside the button, not inside it: a disabled button would hide the hint too.
            Item {
                visible: !Theme.demo
                implicitWidth: saveButton.implicitWidth
                implicitHeight: saveButton.implicitHeight
                Btn { id: saveButton; enabled: Hub.mods.dirty; primary: Hub.mods.dirty; icon: "save"; text: Theme.say("Save"); tip: Theme.say("Writes the load order to ModsConfig.xml (Ctrl + S). The file as it was is copied beside it first."); onClicked: Mods.save() }
                    Hint { step: 3; atRight: true; text: Theme.say("Nothing reaches the game before Save. Until then every change can be undone or discarded.") }
            }
            Btn {
                id: more
                visible: activePane.tight && !Theme.demo
                icon: "ellipsis"
                menu: true
                tone: Hub.mods.errors > 0 ? Theme.err : Mods.problemCount > 0 ? Theme.warn : "transparent"
                text: Mods.problemCount > 0 ? Theme.count(Mods.problemCount) : ""
                tip: Theme.say("Problems, Undo, Redo")
                spoken: Theme.say("Problems, Undo, Redo")
                onClicked: Mods.menuRequested(more, 0, more.height + 2, activePane.moreEntries())
            }
        }
        Rectangle {
            visible: Hub.mods.dirty && !Hub.mods.previewing && !Theme.demo
            Layout.fillWidth: true
            Layout.leftMargin: Theme.gapLarge
            Layout.rightMargin: Theme.gapLarge
            Layout.bottomMargin: Theme.gap
            implicitHeight: Theme.sized(34)
            radius: Theme.corner
            color: Qt.rgba(Theme.warn.r, Theme.warn.g, Theme.warn.b, 0.12)
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: Theme.gapLarge
                anchors.rightMargin: Theme.gapTiny
                spacing: Theme.gap
                Rectangle { implicitWidth: 7; implicitHeight: 7; radius: Math.min(width, height) / 2; color: Theme.warn }
                Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: Theme.fill("Not saved: {}", Hub.mods.unsaved) }
                Btn { text: Theme.say("Show"); tip: Theme.say("Only the mods moved or activated since the last save"); onClicked: activePane.toggleTerm("is:changed") }
                Btn { text: Theme.say("Discard"); tip: Theme.say("Back to the saved list. Undo brings the changes back."); onClicked: Hub.mods.discardChanges() }
                Btn { primary: true; text: Theme.say("Save"); onClicked: Mods.save() }
            }
        }
        Rectangle {
            visible: activePane.versionLine
            Layout.fillWidth: true
            Layout.leftMargin: Theme.gapLarge
            Layout.rightMargin: Theme.gapLarge
            Layout.bottomMargin: Theme.gap
            implicitHeight: banner.implicitHeight + 10
            radius: Theme.corner
            color: Qt.rgba(Theme.warn.r, Theme.warn.g, Theme.warn.b, 0.12)
            GridLayout {
                id: banner
                readonly property bool stacked: Theme.icon + said.implicitWidth + does.implicitWidth + 2 * columnSpacing > width
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                anchors.leftMargin: Theme.gapLarge
                anchors.rightMargin: Theme.gapTiny
                columns: stacked ? 1 : 2
                columnSpacing: Theme.gap
                rowSpacing: Theme.gapSmall
                RowLayout {
                    Layout.fillWidth: true
                    spacing: Theme.gap
                    Icon { Layout.alignment: Qt.AlignTop; name: "triangle-alert"; size: Theme.icon; color: Theme.warn }
                    Text { id: said; Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.body; text: Theme.plural(activePane.versionOnly, "{} mod does not list RimWorld {}", "{} mods do not list RimWorld {}", (Mods.overview.gameVersion || "")) }
                }
                RowLayout {
                    id: does
                    Layout.alignment: Qt.AlignRight
                    spacing: Theme.gap
                    Btn { text: Theme.say("Show"); tip: Theme.say("Only the mods with a warning"); onClicked: activePane.toggleTerm("is:warning") }
                    Btn { text: Theme.fill("Ignore for {}", (Mods.overview.gameVersion || "")); tip: Theme.say("No warning about the game version while RimWorld stays on this version. Settings, Sorting has the switch."); onClicked: Mods.setVersionAside(Mods.overview.gameVersion || "") }
                }
            }
        }
        Text {
            visible: Mods.saveProblem !== "" || Mods.upkeepProblem !== ""
            Layout.fillWidth: true
            Layout.leftMargin: Theme.gapCard
            Layout.rightMargin: Theme.gapCard
            Layout.bottomMargin: Theme.gap
            wrapMode: Text.WordWrap
            color: Theme.err
            font.pixelSize: Theme.body
            text: Mods.saveProblem !== "" ? Theme.fill("The list was not saved. {}", Mods.saveProblem) : Mods.upkeepProblem
        }
        RowLayout {
            visible: Mods.fileChanged
            Layout.leftMargin: Theme.gapCard
            Layout.bottomMargin: Theme.gap
            spacing: Theme.gap
            Btn { text: Theme.say("Use the file's"); tip: Theme.say("Takes the list ModsConfig.xml holds now in place of the one here"); onClicked: Hub.mods.keepFile(true) }
            Btn { text: Theme.say("Keep mine"); tip: Theme.say("Keeps the list here; the next Save replaces what ModsConfig.xml holds now"); onClicked: Hub.mods.keepFile(false) }
        }
        Rectangle {
            visible: Hub.mods.sortNote !== ""
            Layout.fillWidth: true
            Layout.leftMargin: Theme.gapLarge
            Layout.rightMargin: Theme.gapLarge
            Layout.bottomMargin: Theme.gap
            implicitHeight: sortBar.implicitHeight + 14
            radius: Theme.corner
            color: Theme.banner
            RowLayout {
                id: sortBar
                anchors.fill: parent
                anchors.leftMargin: Theme.gapLarge
                anchors.rightMargin: Theme.gap
                spacing: Theme.gap
                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    textFormat: Text.StyledText
                    color: Theme.ink
                    font.pixelSize: Theme.body
                    lineHeight: 1.25
                    text: "<b>" + Theme.plain(Hub.mods.sortNote) + "</b>"
                        + (Hub.mods.previewing ? " <font color=\"" + Theme.ink2 + "\">" + Theme.say("Nothing has changed yet.") + "</font>" : "")
                        + (Hub.mods.sortMore !== "" ? " " + Theme.plain(Hub.mods.sortMore) : "")
                }
                Btn { visible: Hub.mods.previewing; primary: true; text: Theme.say("Apply"); onClicked: Mods.endSort(true) }
                Btn { visible: Hub.mods.previewing; text: Theme.say("Discard"); onClicked: Mods.endSort(false) }
                Btn { visible: !Hub.mods.previewing; text: Theme.say("OK"); onClicked: Hub.mods.sortNote = "" }
                Timer { interval: 8000; running: Hub.mods.sortNote !== "" && !Hub.mods.previewing; onTriggered: Hub.mods.sortNote = "" }
            }
        }
        ModsFilter {
            id: filter
            Layout.fillWidth: true
            Layout.leftMargin: Theme.gapLarge
            Layout.rightMargin: Theme.gapLarge
            Layout.bottomMargin: Theme.gap
            onTextChanged: {
                Mods.filtered = text
                if (Mods.leftOffRestored)
                    Mods.prefs.lastFilter = text
            }
        }
        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: Theme.control
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: Theme.gapWide
                anchors.rightMargin: Theme.gapWide + view.barRoom
                spacing: Theme.gapLarge
                component Heading: Text {
                    id: heading
                    property string column
                    color: Mods.columns.shownBy === column && column !== "" ? Theme.ink : Theme.ink3
                    font.pixelSize: Theme.caption
                    font.weight: Font.Medium
                    activeFocusOnTab: true
                    font.underline: activeFocus
                    Layout.fillHeight: true
                    verticalAlignment: Text.AlignVCenter
                    Accessible.role: Accessible.ColumnHeader
                    Accessible.name: text
                    Keys.onReturnPressed: Mods.columns.showBy(heading.column)
                    Keys.onSpacePressed: Mods.columns.showBy(heading.column)
                    Keys.onMenuPressed: Mods.menuRequested(heading, 0, heading.height + 2, Mods.columns.menu())
                    Keys.onPressed: event => {
                        const by = event.key === Qt.Key_Left ? -1 : event.key === Qt.Key_Right ? 1 : 0
                        const held = (event.modifiers & Qt.ControlModifier) && (event.modifiers & Qt.AltModifier)
                        const at = Mods.columns.shown.indexOf(heading.column)
                        if (by === 0 || !held || at < 0)
                            return
                        Mods.columns.move(heading.column, at + by)
                        event.accepted = true
                    }
                    HoverHandler { cursorShape: Qt.PointingHandCursor }
                    Tip { text: heading.column === "" ? Theme.say("The load order itself") : Theme.say("Click to show the list by this column; again to turn it round. Right-click to choose the columns. Drag it, or Ctrl + Alt + Left or Right, to move it.") }
                    TapHandler { onTapped: Mods.columns.showBy(heading.column) }
                    TapHandler { acceptedButtons: Qt.RightButton; onTapped: point => Mods.menuRequested(heading, point.position.x, heading.height + 2, Mods.columns.menu()) }
                }
                Heading { Layout.preferredWidth: Theme.sized(28); horizontalAlignment: Text.AlignRight; column: ""; text: "#" }
                Heading { Layout.fillWidth: true; column: "name"; text: Theme.say("Name") + (Mods.columns.shownBy === "name" ? (Mods.columns.descending ? " ▾" : " ▴") : "") }
                Repeater {
                    model: view.fitting
                    Heading {
                        id: extra
                        required property string modelData
                        required property int index
                        column: modelData
                        Layout.preferredWidth: view.columnWidth(modelData)
                        horizontalAlignment: modelData === "load" ? Text.AlignRight : Text.AlignLeft
                        text: Theme.say(Mods.columns.labels[modelData]) + (Mods.columns.shownBy === modelData ? (Mods.columns.descending ? " ▾" : " ▴") : "")
                        Item {
                            id: edge
                            x: -Theme.gapLarge / 2 - 4
                            width: 9
                            height: parent.height
                            Rectangle { anchors.horizontalCenter: parent.horizontalCenter; y: 2; width: sizing.active || edgeHover.hovered ? 2 : 1; height: parent.height - 4; color: sizing.active || edgeHover.hovered ? Theme.focus : Theme.fieldLine }
                            HoverHandler { id: edgeHover; cursorShape: Qt.SplitHCursor }
                            Tip { text: Theme.say("Drag to size the column"); over: edgeHover.hovered && !sizing.active }
                            DragHandler {
                                id: sizing
                                property real from: 0
                                target: null
                                yAxis.enabled: false
                                dragThreshold: 1
                                onActiveChanged: if (active) from = extra.width
                                onActiveTranslationChanged: if (active) Mods.columns.size(extra.modelData, from - activeTranslation.x)
                            }
                        }
                        DragHandler {
                            target: null
                            yAxis.enabled: false
                            onActiveChanged: {
                                if (active)
                                    return
                                const moved = centroid.position.x - centroid.pressPosition.x
                                if (Math.abs(moved) > extra.width / 2)
                                    Mods.columns.move(extra.modelData, extra.index + (moved > 0 ? 1 : -1))
                            }
                        }
                    }
                }
                Btn {
                    Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
                    Layout.leftMargin: Math.max(0, (view.wide ? Theme.sized(124) : Theme.sized(44)) - implicitWidth)
                    menu: true
                    text: Theme.say("View")
                    spoken: Theme.say("View options")
                    onClicked: viewMenu.open()
                    Popup {
                        enter: PopIn {}
                        exit: PopOut {}
                        id: viewMenu
                        x: parent.width - width
                        y: parent.height + 6
                        width: Theme.sized(250)
                        padding: Theme.gapMedium
                        background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }
                        contentItem: Column {
                            spacing: Theme.gapSmall
                            Text { topPadding: Theme.gapTiny; color: Theme.ink2; font.pixelSize: Theme.small; font.weight: Font.DemiBold; text: Theme.say("Load order") }
                            Btn { width: parent.width; icon: Mods.prefs.headings !== 0 ? "check" : ""; text: Theme.say("Under category headings"); onClicked: Mods.prefs.headings = 1 }
                            Btn { width: parent.width; icon: Mods.prefs.headings === 0 ? "check" : ""; text: Theme.say("One numbered list"); onClicked: Mods.prefs.headings = 0 }
                            Text { topPadding: Theme.gapTiny; color: Theme.ink2; font.pixelSize: Theme.small; font.weight: Font.DemiBold; text: Theme.say("Columns") }
                            Repeater {
                                model: Object.keys(Mods.columns.labels)
                                Btn {
                                    required property string modelData
                                    width: parent.width
                                    icon: Mods.columns.shown.indexOf(modelData) >= 0 ? "check" : ""
                                    text: Theme.say(Mods.columns.labels[modelData])
                                    onClicked: Mods.columns.toggle(modelData)
                                }
                            }
                            Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink3; font.pixelSize: Theme.caption; text: Theme.say("A heading orders the list when clicked and moves when dragged.") }
                        }
                    }
                }
            }
            Rectangle { anchors.bottom: parent.bottom; x: 6; width: parent.width - 12; height: 1; color: Theme.line }
        }
        ModList {
            id: view
            columns: true
            Layout.fillWidth: true
            Layout.fillHeight: true
            topMargin: Theme.gapTiny
            bottomMargin: Theme.gapTiny
            model: Mods.active
            Accessible.name: Theme.say("Load order")
            onMovementEnded: if (Mods.leftOffRestored) Mods.prefs.lastScroll = contentY
            Connections {
                target: Mods
                function onLeftOffScroll(y) {
                    if (Mods.selectedId < 0)
                        view.contentY = Math.max(-view.topMargin, Math.min(y, view.contentHeight - view.height))
                }
            }
        }
    }

    Column {
        visible: Hub.mods.problem !== ""
        anchors.centerIn: parent
        width: Math.min(parent.width - 80, 520)
        spacing: Theme.gapSmall
        Text { width: parent.width; horizontalAlignment: Text.AlignHCenter; color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: Theme.say("Mods could not be read") }
        Text { visible: !Hub.settings.report.noModFolders; width: parent.width; horizontalAlignment: Text.AlignHCenter; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.body; lineHeight: 1.3; text: Hub.mods.problem }
        readonly property var verdict: (Hub.settings.folderChecks || {}).game
        Text { visible: !!parent.verdict && !parent.verdict.ok; width: parent.width; horizontalAlignment: Text.AlignHCenter; wrapMode: Text.WordWrap; color: Theme.err; font.pixelSize: Theme.body; text: (parent.verdict || { said: "" }).said }
        Row {
            visible: Hub.settings.report.noModFolders
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: Theme.gap
            Btn { primary: true; icon: "folder-open"; text: Theme.say("Choose the game folder…"); onClicked: gameFolder.open() }
            Btn { text: Theme.say("Settings"); onClicked: Mods.showSection(5) }
        }
        Text { visible: Hub.settings.report.noModFolders; width: parent.width; horizontalAlignment: Text.AlignHCenter; wrapMode: Text.WordWrap; color: Theme.ink3; font.pixelSize: Theme.small; text: Hub.mods.problem }
    }

    FolderDialog {
        id: gameFolder
        title: Theme.say("Choose the game folder…")
        onAccepted: Hub.settings.chooseFolder("game", selectedFolder)
    }
}
