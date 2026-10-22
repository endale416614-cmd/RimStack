// Nothing is written to disk until Save.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

Item {
    id: screen
    Appear { of: screen }

    property bool drawerOpen: Hub.given("--drawer")
    Connections {
        target: Mods.prefs
        function onInactiveHiddenChanged() { screen.drawerOpen = false }
    }

    Rectangle {
        id: rail
        z: 2
        visible: Mods.prefs.inactiveHidden && Hub.mods.problem === "" && Mods.comparing === null
        width: visible ? Theme.sized(44) : 0
        height: parent.height - bulk.height
        color: Theme.panel
        Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: Theme.edge }
        Btn {
                id: railButton
                anchors.horizontalCenter: parent.horizontalCenter
                y: 9
                primary: !screen.drawerOpen
                icon: screen.drawerOpen ? "x" : "plus"
                tip: screen.drawerOpen ? Theme.say("Close the inactive mods") : Theme.say("Inactive mods")
                onClicked: screen.drawerOpen = !screen.drawerOpen
            }
        Text {
            rotation: -90
            x: (rail.width - width) / 2
            y: railButton.y + railButton.height + Theme.gapLarge + (width - height) / 2
            color: Theme.ink3
            font.pixelSize: Theme.small
            text: Theme.fill("Inactive {}", Theme.count(Mods.inactive.count))
        }
        DropArea {
            anchors.fill: parent
            onEntered: Mods.drag.target = { model: Mods.inactive, modIndex: Mods.inactive.count, item: rail }
            onExited: if (Mods.drag.target && Mods.drag.target.item === rail) Mods.drag.target = null
            Rectangle { visible: parent.containsDrag; anchors.fill: parent; anchors.margins: Theme.gapThin; radius: Theme.corner; color: "transparent"; border.width: 2; border.color: Theme.focus }
        }
    }

    SplitView {
        id: panes
        // Qt hands a press taken under the open drawer to the handlers below as well.
        enabled: !drawerLoader.active || Mods.drag.held !== null
        anchors.fill: parent
        anchors.leftMargin: rail.width
        anchors.bottomMargin: bulk.height
        orientation: Qt.Horizontal
        // What the two lists share, side by side as in RimSort: the details take the rest.
        readonly property real listsRoom: width - (detailsPane.visible ? detailsPane.width + 7 : 0) - 7
        readonly property real inactiveWide: Math.max(inactivePane.SplitView.minimumWidth, Math.min(Math.round(listsRoom * Mods.prefs.inactiveShare), listsRoom - activePane.SplitView.minimumWidth))
        // The drag sets the width itself; the share is bound again after, to follow the window.
        onResizingChanged: if (!resizing) inactivePane.SplitView.preferredWidth = Qt.binding(() => panes.inactiveWide)
        handle: Rectangle {
            id: grip
            // The divider right of the details; the other is between the two lists.
            readonly property bool first: !inactivePane.visible || x < inactivePane.x
            function move(by) {
                const pane = first ? detailsPane : inactivePane
                const wide = Math.max(pane.SplitView.minimumWidth, pane.width + by)
                if (first) {
                    pane.SplitView.preferredWidth = wide
                    Mods.prefs.details = wide
                } else {
                    Mods.prefs.inactiveShare = wide / panes.listsRoom
                }
            }
            readonly property bool held: SplitHandle.pressed || SplitHandle.hovered || activeFocus
            implicitWidth: 7
            color: "transparent"
            activeFocusOnTab: true
            Accessible.role: Accessible.Separator
            Accessible.name: first ? Theme.say("Width of the details") : Theme.say("Width of the inactive mods")
            Keys.onLeftPressed: move(-24)
            Keys.onRightPressed: move(24)
            // Not further left: a list's scroll bar sits there.
            containmentMask: Item { x: -Theme.gapHair; width: grip.width + Theme.gapHair + Theme.gapSmall; height: grip.height }
            Rectangle {
                anchors.horizontalCenter: parent.horizontalCenter
                width: grip.held ? 3 : 1
                height: parent.height
                color: grip.held ? Theme.focus : Theme.edge
            }
            // Takes no button, so the drag still reaches the divider.
            MouseArea { x: -Theme.gapHair; width: grip.width + Theme.gapHair + Theme.gapSmall; height: parent.height; acceptedButtons: Qt.NoButton; cursorShape: Qt.SizeHorCursor }
            TapHandler { onDoubleTapped: grip.move(grip.first ? 360 - detailsPane.width : panes.listsRoom / 2 - inactivePane.width) }
        }

        DetailsPane {
            id: detailsPane
            SplitView.preferredWidth: Mods.prefs.details
            SplitView.minimumWidth: 260
            onWidthChanged: if (panes.resizing) Mods.prefs.details = width
            visible: Hub.mods.problem === ""
        }
        InactivePane {
            id: inactivePane
            enabled: Mods.comparing === null
            SplitView.preferredWidth: panes.inactiveWide
            SplitView.minimumWidth: 200
            onWidthChanged: if (panes.resizing && panes.listsRoom > 0) Mods.prefs.inactiveShare = width / panes.listsRoom
            visible: Hub.mods.problem === "" && !Mods.prefs.inactiveHidden
        }
        ActivePane {
            id: activePane
            enabled: Mods.comparing === null
            SplitView.fillWidth: true
            SplitView.minimumWidth: 380
        }
    }

    MouseArea {
        visible: drawerLoader.active
        z: 1
        x: rail.width
        width: parent.width - rail.width
        height: parent.height - bulk.height
        onClicked: screen.drawerOpen = false
        Rectangle { anchors.fill: parent; color: Theme.scrim }
    }
    Loader {
        id: drawerLoader
        active: screen.drawerOpen && rail.visible
        x: rail.width
        width: Math.min(Math.max(Theme.sized(260), Mods.prefs.inactive), parent.width - rail.width - 60)
        height: parent.height - bulk.height
        z: 1
        sourceComponent: InactivePane {
            drawer: true
            // Takes the presses its items leave, so none reach the scrim or the load order.
            MouseArea { anchors.fill: parent; z: -1; acceptedButtons: Qt.AllButtons; hoverEnabled: true }
            transform: Translate { id: drawerShift }
            NumberAnimation { target: drawerShift; property: "x"; from: -drawerLoader.width; to: 0; duration: Theme.calm; easing.type: Easing.OutCubic; running: true }
            Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: Theme.edge }
            Rectangle { anchors.left: parent.right; width: 12; height: parent.height; gradient: Gradient { orientation: Gradient.Horizontal; GradientStop { position: 0; color: Qt.rgba(0, 0, 0, 0.3) } GradientStop { position: 1; color: "transparent" } } }
        }
    }

    Loader {
    id: compareLoader
    onItemChanged: Mods.compareView = item
    x: detailsPane.width + 1
    width: parent.width - x
    height: parent.height - bulk.height
    active: Mods.comparing !== null
    sourceComponent: CompareView {
                    earlier: Mods.comparing.earlier
        later: Mods.comparing.later
        earlierPlace: Mods.active.indexOf(Mods.comparing.earlier) + 1
        laterPlace: Mods.active.indexOf(Mods.comparing.later) + 1
        Component.onCompleted: {
            reload()
            forceActiveFocus()
        }
        onDone: {
            Mods.comparing = null
            Mods.focusList(Mods.active)
        }
    }
}

Rectangle {
    id: ghost
    visible: Mods.drag.held !== null
    width: Theme.sized(300)
    height: Theme.sized(26)
    radius: Theme.corner
    color: Theme.accent
    opacity: 0.9
    readonly property point at: screen.mapFromItem(null, Mods.drag.point.x, Mods.drag.point.y)
    x: at.x + 10
    y: at.y - height / 2
    Drag.active: visible
    Drag.hotSpot.x: -10
    Drag.hotSpot.y: height / 2
    Text {
        anchors.verticalCenter: parent.verticalCenter
        x: 10
        width: parent.width - 20
        elide: Text.ElideRight
        color: "white"
        font.pixelSize: Theme.body
        text: Mods.drag.held ? Mods.drag.held.name : ""
    }
}

    BulkBar {
        id: bulk
        anchors.bottom: parent.bottom
        width: parent.width
        onHeightChanged: {
            if (height !== Theme.header || Mods.selectedId < 0)
                return
            const model = Mods.selectedActive ? Mods.active : Mods.inactive
            Mods.reveal(model, model.rowOf(model.indexOf(Mods.selectedId)), false)
        }
    }

    PopMenu { id: rowMenu }
    Prompt {
        id: subPrompt
        property int modId: -1
        function create() {
            if (subName.text.trim() === "")
                return
            Mods.putInSub(subPrompt.modId, subName.text.trim())
            close()
        }
        title: Theme.say("Group under a new tag")
        onOpened: { subName.text = ""; subName.focusInput() }
        FilterField { id: subName; width: parent.width; placeholder: Theme.say("Name"); onAccepted: subPrompt.create() }
        buttons: [
            Btn { text: Theme.say("Cancel"); onClicked: subPrompt.close() },
            Btn { primary: true; text: Theme.say("Create"); enabled: subName.text.trim() !== ""; onClicked: subPrompt.create() }
        ]
    }
    Prompt {
        id: bulkPrompt
        property string what
        function apply() {
            const word = bulkWord.text.trim()
            if (word === "")
                return
            if (what === "tag")
                Mods.tagSelected(word)
            else
                Mods.subSelected(word)
            close()
        }
        title: what === "tag" ? Theme.plural(Mods.selectedCount, "Tag for {} mod", "Tag for {} mods") : Theme.plural(Mods.selectedCount, "Tag to group under, for {} mod", "Tag to group under, for {} mods")
        onOpened: { bulkWord.text = ""; bulkWord.focusInput() }
        FilterField { id: bulkWord; width: parent.width; placeholder: Theme.say("Name"); onAccepted: bulkPrompt.apply() }
        buttons: [
            Btn { text: Theme.say("Cancel"); onClicked: bulkPrompt.close() },
            Btn { primary: true; text: Theme.say("Apply"); enabled: bulkWord.text.trim() !== ""; onClicked: bulkPrompt.apply() }
        ]
    }
    Prompt {
        id: binPrompt
        property string name
        property string folder
        property int modId: -1
        title: Theme.fill("Move to the bin: {}", binPrompt.name)
        Text { width: parent.width; wrapMode: Text.WrapAnywhere; color: Theme.ink2; font.pixelSize: Theme.small; text: binPrompt.folder }
        Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say("Steam downloads a Workshop mod again while you stay subscribed to it.") }
        buttons: [
            Btn { text: Theme.say("Cancel"); onClicked: binPrompt.close() },
            Btn { danger: true; text: Theme.say("Move to the bin"); onClicked: { Mods.binMod(binPrompt.modId); binPrompt.close() } }
        ]
    }
    Connections {
        target: Mods
        function onSubNameRequested(modId) { subPrompt.modId = modId; subPrompt.open() }
        function onBulkRequested(what) { bulkPrompt.what = what; bulkPrompt.open() }
        function onBinRequested(name, folder, modId) { binPrompt.name = name; binPrompt.folder = folder; binPrompt.modId = modId; binPrompt.open() }
    }
    Connections {
        target: Mods
        function onMenuRequested(item, x, y, entries) { rowMenu.openAt(item, x, y, entries) }
    }

    Shortcut { sequences: [StandardKey.Find]; enabled: !drawerLoader.active; onActivated: Mods.filterFocusRequested() }
    Shortcut { sequences: [StandardKey.Undo]; onActivated: Hub.mods.undo() }
    Shortcut { sequences: [StandardKey.Redo]; onActivated: Hub.mods.redo() }
    Shortcut { sequences: [StandardKey.Save]; onActivated: if (Hub.mods.dirty) Mods.save() }
    // Off while a text field has the focus, so that Esc reaches the field. Window attached
    // to the Shortcut itself is null: it is read off the screen.
    Shortcut { sequence: "Escape"; enabled: Mods.comparing === null && !subPrompt.opened && !(screen.Window.activeFocusItem && screen.Window.activeFocusItem.cursorPosition !== undefined); onActivated: screen.drawerOpen ? screen.drawerOpen = false : Mods.picking !== "" ? Mods.picking = "" : Mods.select(-1, false) }

Timer {
    readonly property bool sorting: Hub.given("--sorting")
    readonly property bool comparing: Hub.given("--comparing")
    interval: 400
    repeat: true
    running: sorting || comparing
    onTriggered: {
        if (!Hub.mods.filesRead)
            return
        stop()
        if (sorting)
            Mods.previewSort()
        for (let at = 0; comparing && at < Mods.active.count && Mods.comparing === null; at++) {
            Mods.select(Mods.nthMod(Mods.active, at), true)
            const other = (Mods.info.losesTo || []).concat(Mods.info.winsOver || [])[0]
            if (other)
                Mods.compareWith(other.modId)
        }
    }
}

Timer {
    readonly property int picked: Hub.number("--pick", -1)
    interval: 1500
    running: picked >= 0
    onTriggered: {
        Mods.select(Mods.active.modIdAt(picked), true)
        Mods.reveal(Mods.active, picked, true)
        Mods.tab = Hub.number("--tab", 0)
        if (Hub.given("--notes") && Mods.info.workshop)
            Hub.downloads.fetchNotes(Mods.selectedId)
    }
}

    // `visible` is true from the click; `opened` only once the menu has faded in.
    readonly property alias menuOpen: rowMenu.visible

    Loader {
        active: (Mods.checkMode !== "" && Mods.checkMode !== "pointer") || Theme.selfTest
        sourceComponent: ModsChecks { view: activePane.list }
    }
    // Opened by address, so Qt's test module is needed only for this.
    Loader {
        source: Mods.checkMode === "pointer" ? "qrc:/qt/qml/RimStack/Ui/qml/checks/PointerChecks.qml" : ""
        onLoaded: {
            item.screen = screen
            item.activeList = activePane.list
            item.inactiveList = inactivePane.list
            item.details = detailsPane
        }
    }
}
