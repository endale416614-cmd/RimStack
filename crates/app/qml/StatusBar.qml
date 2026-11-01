pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

Rectangle {
    id: strip
    // The newest launch on record: { time, errors }.
    property var lastLaunch: null
    property var tasks: []
    readonly property alias trayMenu: trayMenu
    readonly property alias historyMenu: historyMenu
    signal useList(var ids)
    signal keys()
    signal logs()

    implicitHeight: Theme.strip
    color: Theme.bar
    Rectangle { width: parent.width; height: 1; color: Theme.edge }
    Row {
        id: leftSide
        x: Theme.gapCard
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.gapWide
        Text {
            visible: text !== ""
            color: Mods.dirty ? Theme.warn : Theme.ink2
            font.pixelSize: Theme.small
            text: Theme.demo ? Theme.say("Demo data") : Mods.dirty ? Theme.fill("Not saved: {}", Hub.mods.unsaved) : ""
        }
        Text {
            visible: Notices.hint !== "" && Notices.hint !== last.tip
            width: Math.max(0, Math.min(implicitWidth, rightSide.x - leftSide.x - x - Theme.gapWide))
            elide: Text.ElideRight
            color: Theme.ink
            font.pixelSize: Theme.small
            text: Notices.hint.split("\n")[0]
        }
        Item {
            id: last
            readonly property var l: strip.lastLaunch
            readonly property bool counted: !!l && l.errors !== null && l.errors !== undefined
            readonly property string tip: Theme.say("Opens Logs on this launch")
            visible: !!l && (Notices.hint === "" || Notices.hint === tip)
            anchors.verticalCenter: parent.verticalCenter
            implicitWidth: said.width
            implicitHeight: Math.max(said.height, Theme.control)
            width: implicitWidth
            height: implicitHeight
            activeFocusOnTab: true
            Accessible.role: Accessible.Button
            Accessible.name: started.text + " " + errors.text
            Accessible.onPressAction: strip.logs()
            Keys.onReturnPressed: strip.logs()
            Keys.onSpacePressed: strip.logs()
            Row {
            id: said
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.gapSmall
            Text { id: started; color: lastHover.hovered || last.activeFocus ? Theme.ink : Theme.ink2; font.pixelSize: Theme.small; font.underline: last.activeFocus; text: last.l ? Theme.fill("Last start of the game {}", Theme.moment(last.l.time)) : "" }
            Text {
                id: errors
                visible: last.counted
                color: last.counted && last.l.errors > 0 ? Theme.err : Theme.ink2
                font.pixelSize: Theme.small
                text: !last.counted ? "" : last.l.errors === 0 ? Theme.say("no errors") : Theme.plural(last.l.errors, "{} different error", "{} different errors")
            }
            Icon { anchors.verticalCenter: parent.verticalCenter; name: "chevron-right"; size: Theme.sized(11); color: Theme.ink3 }
            }
            HoverHandler { id: lastHover; cursorShape: Qt.PointingHandCursor }
            TapHandler { onTapped: strip.logs() }
            Tip { text: last.tip; over: lastHover.hovered || last.activeFocus }
        }
    }
    Row {
        id: rightSide
        anchors.right: parent.right
        anchors.rightMargin: Theme.gapMedium
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.gapLarge
        Text {
            visible: strip.tasks.length === 0 && Hub.textures.progress !== ""
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.ink2
            font.pixelSize: Theme.small
            text: Hub.textures.progress
        }
        Row {
            id: tray
            visible: strip.tasks.length > 0
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.gapSmall
            Accessible.role: Accessible.Button
            Accessible.name: Theme.say("Background work")
            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: "refresh-cw"
                size: Theme.sized(12)
                color: Theme.moved
                RotationAnimation on rotation { running: tray.visible && !Theme.stored.reduceMotion; from: 0; to: 360; duration: 1400; loops: Animation.Infinite }
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.ink
                font.pixelSize: Theme.small
                text: strip.tasks.length === 1 ? strip.tasks[0].text : Theme.fill("{} tasks running", strip.tasks.length)
            }
            HoverHandler { cursorShape: Qt.PointingHandCursor }
            TapHandler { onTapped: trayMenu.open() }
            Popup {
                enter: PopIn {}
                exit: PopOut {}
                id: trayMenu
                x: parent.width - width
                y: -height - Theme.gapMedium
                width: Theme.sized(380)
                padding: Theme.gapLarge
                background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }
                contentItem: Column {
                    spacing: Theme.gap
                    Text { color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: Theme.say("Background work") }
                    Text { visible: strip.tasks.length === 0; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say("Nothing running") }
                    Repeater {
                        model: strip.tasks
                        RowLayout {
                            id: task
                            required property var modelData
                            width: trayMenu.availableWidth
                            spacing: Theme.gap
                            Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.body; text: task.modelData.text }
                            Btn { visible: !!task.modelData.cancel; text: Theme.say("Cancel"); tip: Theme.say("Stops after the mod it is on; what is done stays done"); onClicked: task.modelData.cancel() }
                        }
                    }
                }
            }
        }
        Btn {
            anchors.verticalCenter: parent.verticalCenter
            height: Theme.strip - 2
            visible: !!Hub.settings.release && !!Hub.settings.release.version
            icon: "external-link"
            text: Theme.fill("Get {}", (Hub.settings.release || ({})).version)
            tip: Theme.say("A newer version is published; opens its page")
            onClicked: Qt.openUrlExternally(Hub.settings.release.page)
        }
        Btn {
            anchors.verticalCenter: parent.verticalCenter
            height: Theme.strip - 2
            text: Theme.say("? Keys")
            spoken: Theme.say("Keyboard shortcuts (F1)")
            tip: Theme.say("Keyboard shortcuts (F1)")
            onClicked: strip.keys()
        }
        Btn {
            anchors.verticalCenter: parent.verticalCenter
            height: Theme.strip - 2
            visible: !Theme.demo
            icon: "history"
            text: Theme.say("History")
            tip: Theme.say("What RimStack changed, and how to take it back")
            onClicked: historyMenu.open()
            HistoryMenu {
                id: historyMenu
                x: parent.width - width
                y: -height - 8
                onUseList: ids => strip.useList(ids)
            }
        }
    }
}
