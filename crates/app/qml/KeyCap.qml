import QtQuick
import RimStack.Ui

Rectangle {
    id: cap
    property string text
    implicitWidth: Math.max(implicitHeight, label.implicitWidth + Theme.gapMedium)
    implicitHeight: Theme.sized(20)
    radius: 4
    color: Theme.chip
    border.width: 1
    border.color: Theme.line
    Accessible.ignored: true
    Text {
        id: label
        anchors.centerIn: parent
        color: Theme.ink
        font.pixelSize: Theme.caption
        font.weight: Font.Medium
        text: cap.text
    }
}
