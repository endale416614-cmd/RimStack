pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import RimStack.Ui

Item {
    id: tip
    property string text
    // Set by an item that already knows whether the pointer is on it;
    // left alone, the tip watches for itself.
    property bool over: own.hovered
    anchors.fill: parent

    // A flag the timer sets: a tip tied to "timer not running" would show a moment early.
    property bool shown: false
    onOverChanged: {
        if (over && text !== "")
            Notices.hint = text
        else if (Notices.hint === text)
            Notices.hint = ""
        shown = false
        if (over && text !== "")
            rest.restart()
        else
            rest.stop()
    }

    HoverHandler { id: own }
    Timer {
        id: rest
        interval: 600
        onTriggered: tip.shown = tip.over
    }
    Loader {
        active: tip.shown && tip.text !== ""
        sourceComponent: ToolTip {
            parent: tip
            visible: true
            delay: 0
            timeout: 15000
            padding: Theme.gap
            margins: Theme.gap
            contentWidth: Math.min(words.implicitWidth, Theme.sized(340))
            contentItem: Text { id: words; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.small; lineHeight: 1.25; text: tip.text }
            background: Rectangle { radius: Theme.corner; color: Theme.bar; border.width: 1; border.color: Theme.line }
        }
    }
}
