pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

Popup {
    enter: PopIn {}
    exit: PopOut {}
    id: popup

    // Each command is { name, group, keys, run: function, when: bool }.
    required property var commands
    // Given the typed text, the mods it names: { name, group, run }.
    property var findMods: null
    readonly property var shown: {
        const words = box.text.toLowerCase().split(" ").filter(w => w)
        const holds = c => words.every(w => (c.name + " " + (c.group || "")).toLowerCase().indexOf(w) !== -1)
        const found = commands.filter(c => c.when !== false && holds(c) && (words.length > 0 || c.group !== Theme.say("Setting")))
        return box.text.trim().length >= 2 && findMods ? found.concat(findMods(box.text)) : found
    }
    onChosenChanged: list.positionViewAtIndex(chosen, ListView.Contain)
    property int chosen: 0

    function runChosen() {
        const command = shown[chosen]
        close()
        if (command)
            command.run()
    }

    width: Theme.sized(600)
    padding: Theme.gap
    modal: true
    focus: true
    onAboutToShow: {
        box.text = ""
        chosen = 0
    }
    onOpened: box.focusInput()
    background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }
    Overlay.modal: Rectangle { color: Theme.scrim }

    contentItem: Column {
        spacing: Theme.gapSmall
        FilterField {
            id: box
            width: parent.width
            placeholder: Theme.say("A mod, a command or a setting")
            onTextChanged: popup.chosen = 0
            onAccepted: popup.runChosen()
            Keys.onDownPressed: popup.chosen = Math.min(popup.chosen + 1, popup.shown.length - 1)
            Keys.onUpPressed: popup.chosen = Math.max(popup.chosen - 1, 0)
            Keys.onPressed: (event) => {
                if (event.key === Qt.Key_PageDown)
                    popup.chosen = Math.min(popup.chosen + 10, popup.shown.length - 1)
                else if (event.key === Qt.Key_PageUp)
                    popup.chosen = Math.max(popup.chosen - 10, 0)
                else
                    return
                event.accepted = true
            }
        }
        ListView {
            id: list
            width: parent.width
            height: Math.min(contentHeight, Theme.sized(390))
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            Wheel {}
            model: popup.shown
            currentIndex: popup.chosen
            delegate: Item {
                id: row
                required property int index
                required property var modelData
                readonly property bool current: index === popup.chosen
                width: list.width
                height: Theme.sized(30)
                Rectangle { anchors.fill: parent; radius: Theme.corner; color: row.current ? Theme.accent : Theme.hover; opacity: row.current ? 1 : rowHover.hovered ? 0.05 : 0 }
                HoverHandler { id: rowHover }
                TapHandler { onTapped: { popup.chosen = row.index; popup.runChosen() } }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Theme.gapMedium
                    anchors.rightMargin: Theme.gapMedium
                    Text { Layout.fillWidth: true; elide: Text.ElideRight; color: row.current ? Theme.onSelected : Theme.ink; font.pixelSize: Theme.body; text: row.modelData.name }
                    Text { color: row.current ? Theme.onSelectedDim : Theme.ink3; font.pixelSize: Theme.small; text: row.modelData.keys || row.modelData.group || "" }
                }
            }
        }
        Text { visible: popup.shown.length === 0; leftPadding: Theme.gapMedium; bottomPadding: Theme.gapSmall; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say("No match") }
    }
}
