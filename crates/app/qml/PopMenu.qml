// Entries: { text, note, icon, off, danger, run }, { line: true } for a divider,
// or { title } for a caption.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import RimStack.Ui

Popup {
    // It leaves at once: while a menu fades out the window's shortcuts do not answer.
    enter: PopIn {}
    id: menu
    property var entries: []
    // The entry the arrow keys are on; -1 for none.
    property int at: -1
    function step(by) {
        let next = at
        for (let tries = 0; tries < entries.length; tries++) {
            next = (next + by + entries.length) % entries.length
            const entry = entries[next]
            if (!entry.line && !entry.title && !entry.off) {
                at = next
                return
            }
        }
    }
    onOpened: { at = -1; contentItem.forceActiveFocus() }

    // Clamped to the window here: the entries are not laid out yet when openAt runs.
    property point wanted
    x: Math.max(Theme.gapTiny, Math.min(wanted.x, parent.width - width - Theme.gapTiny))
    y: Math.max(Theme.gapTiny, Math.min(wanted.y, parent.height - height - Theme.gapTiny))
    function openAt(item, x, y, entries) {
        menu.entries = entries
        wanted = item.mapToItem(parent, x, y)
        open()
    }

    parent: Overlay.overlay
    padding: Theme.gapTiny
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }
    FontMetrics { id: noteFont; font.pixelSize: Theme.small }
    contentItem: Column {
        Keys.onDownPressed: menu.step(1)
        Keys.onUpPressed: menu.step(-1)
        Keys.onReturnPressed: if (menu.at >= 0) { const chosen = menu.entries[menu.at]; menu.close(); chosen.run() }
        Repeater {
            model: menu.entries
            Item {
                id: entry
                required property var modelData
                required property int index
                readonly property bool line: !!modelData.line
                readonly property bool off: !!modelData.off
                readonly property bool caption: !!modelData.title
                readonly property bool noted: !!modelData.note
                implicitWidth: line ? 1 : caption ? heading.implicitWidth + 2 * Theme.gapMedium : words.x + Math.max(words.implicitWidth, noteText.width * Number(noted)) + Theme.gapWide
                width: Math.max(implicitWidth, parent.width)
                height: line ? Theme.gap + Theme.gapLine : caption ? Theme.tag + Theme.gapTiny : Theme.strip + Number(noted) * (noteText.implicitHeight + Theme.gap)
                Text { id: heading; visible: entry.caption; x: Theme.gapMedium; anchors.verticalCenter: parent.verticalCenter; color: Theme.ink3; font.pixelSize: Theme.caption; font.weight: Font.Medium; text: Theme.say(entry.modelData.title || "") }
                Rectangle { visible: entry.line; y: Theme.gapTiny; x: Theme.gapTiny; width: parent.width - 2 * Theme.gapTiny; height: 1; color: Theme.line }
                Rectangle {
                    visible: !entry.line && !entry.caption
                    anchors.fill: parent
                    radius: Theme.corner
                    color: (over.hovered || menu.at === entry.index) && !entry.off ? Theme.accent : "transparent"
                    opacity: entry.off ? 0.4 : 1
                    Accessible.role: Accessible.MenuItem
                    Accessible.name: entry.modelData.text || ""
                    Accessible.description: entry.modelData.note || ""
                    Item { id: strip; width: parent.width; height: Theme.strip }
                    Rectangle { x: Theme.gapMedium; anchors.verticalCenter: strip.verticalCenter; visible: !!entry.modelData.colour; width: Theme.gapMedium; height: width; radius: width / 2; color: entry.modelData.colour || "transparent" }
                    Icon { x: Theme.gap; anchors.verticalCenter: strip.verticalCenter; visible: !!entry.modelData.icon; name: entry.modelData.icon || ""; color: words.color }
                    Text {
                        id: words
                        x: 2 * Theme.gap + Theme.icon
                        anchors.verticalCenter: strip.verticalCenter
                        color: (over.hovered || menu.at === entry.index) && !entry.off ? Theme.accentInk : entry.modelData.danger ? Theme.err : Theme.ink
                        font.pixelSize: Theme.body
                        text: Theme.say(entry.modelData.text || "")
                    }
                    Text {
                        id: noteText
                        visible: entry.noted
                        x: words.x
                        y: strip.height
                        width: Math.min(implicitWidth, noteFont.averageCharacterWidth * 44)
                        wrapMode: Text.WordWrap
                        lineHeight: 1.25
                        color: words.color
                        opacity: 0.75
                        font.pixelSize: Theme.small
                        text: entry.modelData.note || ""
                    }
                    HoverHandler { id: over }
                    TapHandler {
                        enabled: !entry.off
                        onTapped: {
                            menu.close()
                            entry.modelData.run()
                        }
                    }
                }
            }
        }
    }
}
