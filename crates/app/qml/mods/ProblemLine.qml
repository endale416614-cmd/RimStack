pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

GridLayout {
    id: line
    property bool error: true
    property string name
    property string words
    // { action, target, label }: what `Mods.applyFix` is given.
    property var fix: ({})
    property int modId: -1
    // Empty when it cannot be ignored; with the mod's package id, what `Hub.mods.ignoreProblem` is given.
    property string ignore
    property string packageId
    // The action on a line of its own, under the words.
    property bool stacked: width - actions.implicitWidth - mark.width - 2 * columnSpacing < measure.averageCharacterWidth * 20
    columnSpacing: Theme.gap
    rowSpacing: Theme.gapTiny
    columns: 3 - Number(stacked)
    FontMetrics { id: measure; font.pixelSize: Theme.body }
    Icon { id: mark; Layout.alignment: Qt.AlignTop; Layout.topMargin: Theme.gapHair; name: line.error ? "circle-alert" : "triangle-alert"; color: line.error ? Theme.err : Theme.warn }
    Text {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        textFormat: Text.StyledText
        color: Theme.ink
        font.pixelSize: Theme.body
        lineHeight: 1.25
        text: (line.name !== "" ? "<b>" + Theme.plain(line.name) + "</b>  " : "") + Theme.plain(line.words)
        HoverHandler { enabled: line.name !== "" && line.modId >= 0; cursorShape: Qt.PointingHandCursor }
        TapHandler { enabled: line.name !== "" && line.modId >= 0; onTapped: Mods.showMod(line.modId) }
    }
    Row {
        id: actions
        Layout.alignment: Qt.AlignTop
        Layout.row: Number(line.stacked)
        Layout.column: 2 - Number(line.stacked)
        spacing: Theme.gapSmall
        visible: !Hub.mods.previewing
        Btn {
            visible: !!line.fix.action
            text: Theme.say(line.fix.label || "")
            onClicked: Mods.applyFix(line.modId, line.fix)
        }
        Btn {
            visible: !!line.ignore
            compact: true
            icon: "eye-off"
            spoken: Theme.say("Ignore this problem")
            tip: Theme.say("No warning about this problem again; the mod's details can bring it back")
            onClicked: {
                // The line goes with the problem; the undo keeps its own copy.
                const [packageId, key] = [line.packageId, line.ignore]
                Hub.mods.ignoreProblem(packageId, key, true)
                Notices.offer(Theme.say("The problem is ignored"), Theme.say("Undo"), () => Hub.mods.ignoreProblem(packageId, key, false))
            }
        }
    }
}
