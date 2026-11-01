pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

Popup {
    id: prompt
    property string title
    default property alias body: content.body
    property alias buttons: actions.data
    property real wanted: Theme.sized(400)
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(parent.width - 2 * Theme.gapPage, prompt.wanted)
    height: Math.min(parent.height - 2 * Theme.gapPage, implicitHeight)
    padding: Theme.gapWide
    modal: true
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }
    enter: PopIn {}
    exit: PopOut {}
    contentItem: ColumnLayout {
        spacing: Theme.gapLarge
        Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: prompt.title }
        Scrolled {
            id: content
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredHeight: contentHeight
            spacing: Theme.gap
        }
        Row { id: actions; Layout.alignment: Qt.AlignRight; spacing: Theme.gap }
    }
}
