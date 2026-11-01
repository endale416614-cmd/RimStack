pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

Column {
    id: toasts
    spacing: Theme.gapSmall
    Repeater {
        model: Notices.shown
        Rectangle {
            id: toast
            required property var modelData
            readonly property bool reached: !!Window.activeFocusItem && Window.activeFocusItem.parent === content
            readonly property color tone: modelData.tone === "err" ? Theme.err : modelData.tone === "ok" ? Theme.ok : Theme.moved
            width: Math.min(Theme.sized(420), content.implicitWidth + Theme.gapCard + Theme.gap)
            height: Math.max(Theme.sized(40), words.implicitHeight + 2 * Theme.gapMedium)
            radius: Theme.cornerLarge
            color: Theme.bar
            border.width: 1
            border.color: Theme.line
            Accessible.role: Accessible.AlertMessage
            Accessible.name: modelData.text
            Rectangle { x: 1; y: Theme.gap; width: 3; height: parent.height - 2 * Theme.gap; radius: Math.min(width, height) / 2; color: toast.tone }
            RowLayout {
                id: content
                anchors.fill: parent
                anchors.leftMargin: Theme.gapCard
                anchors.rightMargin: Theme.gap
                spacing: Theme.gap
                Text { id: words; Layout.fillWidth: true; Layout.maximumWidth: Theme.sized(300); wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.body; text: toast.modelData.text }
                Btn { visible: !!toast.modelData.run; text: toast.modelData.label || ""; onClicked: { toast.modelData.run(); Notices.dismiss(toast.modelData.key) } }
                Btn { icon: "x"; tip: Theme.say("Dismiss"); onClicked: Notices.dismiss(toast.modelData.key) }
            }
            HoverHandler { id: held }
            Timer { interval: toast.modelData.run ? 8000 : 5000; running: !held.hovered && !toast.reached; onTriggered: Notices.dismiss(toast.modelData.key) }
        }
    }
}
