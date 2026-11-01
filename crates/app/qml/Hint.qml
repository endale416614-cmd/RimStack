pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

Popup {
    id: hint
    property int step
    property string text
    property bool atRight: false
    readonly property bool due: Mods.hintStep === step
    visible: due && parent !== null && parent.visible
    onDueChanged: {
        if (due && parent && !parent.visible) {
            if (step < 3)
                Mods.hintStep = step + 1
            else
                Mods.endHints()
        }
    }
    closePolicy: Popup.NoAutoClose
    x: atRight ? parent.width - width : 0
    y: parent ? parent.height + 9 : 0
    width: Theme.sized(280)
    height: words.implicitHeight + footer.implicitHeight + 2 * Theme.gapMedium + Theme.gapLarge
    padding: 0
    background: Rectangle {
        radius: Theme.cornerLarge
        color: Theme.accent
        Rectangle { x: hint.atRight ? parent.width - Theme.gapWide - Theme.gapMedium : Theme.gapWide; y: -height / 2; width: Theme.gapMedium; height: width; rotation: 45; color: Theme.accent }
    }
    contentItem: Item {
        Text { id: words; x: Theme.gapLarge; y: Theme.gapMedium; width: parent.width - 2 * Theme.gapLarge; wrapMode: Text.WordWrap; color: Theme.accentInk; font.pixelSize: Theme.body; lineHeight: 1.25; text: hint.text }
        RowLayout {
            id: footer
            x: Theme.gapLarge
            width: parent.width - 2 * Theme.gapLarge
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.gapMedium
            Text { Layout.fillWidth: true; color: Theme.onSelectedDim; font.pixelSize: Theme.small; text: hint.step + " / 3" }
            Btn { text: Theme.say("Skip"); onClicked: Mods.endHints() }
            Btn { text: hint.step < 3 ? Theme.say("Next") : Theme.say("Done"); onClicked: hint.step < 3 ? Mods.hintStep = hint.step + 1 : Mods.endHints() }
        }
    }
}
