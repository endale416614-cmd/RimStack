pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

Column {
    id: moves
    width: parent.width
    visible: Hub.mods.previewing && Mods.comparing === null
    spacing: Theme.gapHair
    // { name, moves } per kind of reason; each move { name, from, to, modId, why }.
    readonly property var groups: visible ? Hub.mods.sortMoves() : []
    readonly property int moved: groups.reduce((sum, group) => sum + group.moves.length, 0)

    Text { color: Theme.ink; font.pixelSize: Theme.heading; font.weight: Font.DemiBold; text: Theme.say("Sort preview") }
    Text { color: Theme.ink2; font.pixelSize: Theme.body; bottomPadding: Theme.gapMedium; text: Theme.plural(moves.moved, "{} mod moves", "{} mods move") }
    Repeater {
        model: moves.groups
        Column {
            id: group
            required property var modelData
            width: moves.width
            spacing: Theme.gapHair
            bottomPadding: Theme.gapMedium
            Row {
                spacing: Theme.gapSmall
                bottomPadding: Theme.gapHair
                Text { color: Theme.ink2; font.pixelSize: Theme.small; font.weight: Font.DemiBold; text: Theme.say(group.modelData.name) }
                Text { color: Theme.ink3; font.pixelSize: Theme.small; text: group.modelData.moves.length }
            }
            Repeater {
                model: group.modelData.moves
                Rectangle {
                    id: move
                    required property var modelData
                    readonly property bool up: modelData.to < modelData.from
                    readonly property bool chosen: modelData.modId === Mods.selectedId
                    width: moves.width
                    height: Math.max(Theme.sized(40), said.implicitHeight + Theme.gapMedium)
                    radius: Theme.corner
                    color: chosen ? Theme.seg : hovering.hovered ? Theme.chip : "transparent"
                    Accessible.role: Accessible.Button
                    Accessible.name: modelData.name + ", " + modelData.why
                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: Theme.gapSmall
                        anchors.rightMargin: Theme.gapSmall
                        spacing: Theme.gap
                        Icon { name: move.up ? "arrow-up" : "arrow-down"; size: Theme.sized(13); color: Theme.moved }
                        Column {
                            id: said
                            Layout.fillWidth: true
                            Text { width: parent.width; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: move.modelData.name }
                            Text { width: parent.width; wrapMode: Text.WordWrap; maximumLineCount: 3; elide: Text.ElideRight; color: Theme.ink2; font.pixelSize: Theme.caption; text: move.modelData.why }
                        }
                        Text { color: Theme.ink2; font.pixelSize: Theme.small; font.features: { "tnum": 1 }; text: move.modelData.from + " → " + move.modelData.to }
                    }
                    HoverHandler { id: hovering; cursorShape: Qt.PointingHandCursor }
                    TapHandler {
                        onTapped: {
                            Mods.select(move.modelData.modId, true)
                            Mods.reveal(Mods.active, Mods.active.rowOf(move.modelData.to - 1), true)
                        }
                    }
                    Tip { text: move.modelData.why; over: hovering.hovered }
                }
            }
        }
    }
    Item { width: 1; height: Theme.gapCard }
}
