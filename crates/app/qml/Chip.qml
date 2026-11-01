pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

Rectangle {
    id: chip
    property string text
    property string icon
    property color tone: Theme.ink2
    property bool onAccent: false
    property string tip
    property real maxWidth: Infinity
    property bool pressable: false
    signal clicked()

    implicitHeight: pressable ? Theme.control : Theme.tag
    implicitWidth: Math.min(maxWidth, content.implicitWidth + Theme.gapCard)
    radius: height / 2
    color: onAccent ? Theme.selectedChip : Qt.rgba(tone.r, tone.g, tone.b, hovering.hovered && pressable ? 0.28 : 0.16)
    Behavior on color { ColorAnimation { duration: Theme.quick } }
    activeFocusOnTab: pressable
    FocusRing { corner: parent.radius; onAccent: chip.onAccent }
    Accessible.role: pressable ? Accessible.Button : Accessible.StaticText
    Accessible.ignored: !visible
    Accessible.name: text !== "" ? text : tip
    Accessible.onPressAction: if (pressable) chip.clicked()
    Keys.onReturnPressed: chip.clicked()
    Keys.onSpacePressed: chip.clicked()
    Row {
        id: content
        x: Theme.gapCard / 2
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.gapTiny
        Icon { visible: chip.icon !== ""; anchors.verticalCenter: parent.verticalCenter; name: chip.icon; size: Theme.sized(11); color: label.color }
        Text {
            id: label
            visible: chip.text !== ""
            width: Math.min(implicitWidth, chip.maxWidth - Theme.gapCard - label.x)
            elide: Text.ElideRight
            color: chip.onAccent ? Theme.onSelected : chip.tone
            font.pixelSize: Theme.caption
            font.weight: Font.Medium
            text: chip.text
        }
    }
    HoverHandler { id: hovering; cursorShape: chip.pressable ? Qt.PointingHandCursor : Qt.ArrowCursor }
    TapHandler { enabled: chip.pressable; onTapped: chip.clicked() }
    Tip { text: chip.tip; over: hovering.hovered || (chip.pressable && chip.activeFocus) }
}
