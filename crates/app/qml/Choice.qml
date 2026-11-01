pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

Rectangle {
    id: choice
    property var options: []
    property int current: 0
    property string title
    property bool stretch: false
    signal picked(int index)
    readonly property real wordsWidth: allWords.advanceWidth
    TextMetrics { id: allWords; font.pixelSize: Theme.small; font.weight: Font.DemiBold; text: choice.options.map(option => Theme.say(option)).join("") }
    readonly property bool stacked: stretch && (width - 4 - wordsWidth) / Math.max(1, options.length) < 6
    function move(by) {
        const next = current + by
        if (next >= 0 && next < options.length)
            picked(next)
    }
    implicitWidth: row.implicitWidth + 4
    implicitHeight: (stacked ? options.length : 1) * Theme.segment + 4
    radius: Theme.corner
    color: Theme.seg
    border.width: 1
    border.color: Theme.fieldLine
    activeFocusOnTab: true
    Accessible.role: Accessible.Grouping
    Accessible.name: title
    Keys.onLeftPressed: move(-1)
    Keys.onRightPressed: move(1)
    Keys.onUpPressed: event => { if (stacked) move(-1); else event.accepted = false }
    Keys.onDownPressed: event => { if (stacked) move(1); else event.accepted = false }
    FocusRing {}
    Rectangle {
        readonly property Item at: segments.count > 0 ? segments.itemAt(choice.current) : null
        id: thumb
        property bool settled: false
        Component.onCompleted: Qt.callLater(() => thumb.settled = true)
        x: thumb.at ? 2 + thumb.at.x : 2
        y: thumb.at ? 2 + thumb.at.y : 2
        width: thumb.at ? thumb.at.width : 0
        height: Theme.segment
        radius: Theme.corner - 1
        color: Theme.accent
        Behavior on x { enabled: thumb.settled; NumberAnimation { duration: Theme.quick; easing.type: Easing.OutCubic } }
        Behavior on width { enabled: thumb.settled; NumberAnimation { duration: Theme.quick; easing.type: Easing.OutCubic } }
    }
    Grid {
        id: row
        x: 2
        y: 2
        columns: choice.stacked ? 1 : Math.max(1, choice.options.length)
        Repeater {
            id: segments
            model: choice.options
            Rectangle {
                id: option
                required property string modelData
                required property int index
                readonly property bool on: index === choice.current
                readonly property real wordWidth: optionWords.advanceWidth
                TextMetrics { id: optionWords; font: allWords.font; text: Theme.say(option.modelData) }
                width: !choice.stretch ? wordWidth + 2 * Theme.gapLarge : choice.stacked ? choice.width - 4 : wordWidth + (choice.width - 4 - choice.wordsWidth) / Math.max(1, choice.options.length)
                height: Theme.segment
                radius: Theme.corner - 1
                color: "transparent"
                Accessible.role: Accessible.RadioButton
                Accessible.name: modelData
                Accessible.checked: on
                Accessible.onPressAction: choice.picked(option.index)
                Text { id: label; anchors.centerIn: parent; Behavior on color { ColorAnimation { duration: Theme.quick } } width: option.width - (choice.stacked ? 2 * Theme.gapMedium : Theme.gapSmall); horizontalAlignment: choice.stacked ? Text.AlignLeft : Text.AlignHCenter; elide: Text.ElideRight; color: option.on ? Theme.accentInk : Theme.ink2; font.pixelSize: Theme.small; font.weight: option.on ? Font.DemiBold : Font.Normal; text: Theme.say(option.modelData) }
                TapHandler { onTapped: choice.picked(option.index) }
            }
        }
    }
}
