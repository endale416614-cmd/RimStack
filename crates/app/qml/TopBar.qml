pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

Rectangle {
    id: bar
    property int section: 0
    property int behind: 0
    property bool gameRunning: false
    property var inTheWay: []
    readonly property alias listsMenu: listsMenu
    readonly property alias beforePlay: beforePlay
    readonly property var sections: ["Mods", "Downloads", "Textures", "Saves", "Logs", "Settings"]
    signal go(int section)
    signal search()
    signal pressPlay()
    signal play()
    signal useList(var ids)
    function pickSection(from) {
        Mods.menuRequested(from, 0, from.height + 4, sections.map((name, at) => ({ text: Theme.say(name), off: at === section, run: () => go(at) })))
    }
    function step(by) {
        const at = section + by
        if (at >= 0 && at < sections.length)
            go(at)
    }
    readonly property bool tight: width < Theme.sized(960)
    readonly property real sectionsWidth: {
        let all = 0
        for (let at = 0; at < tabs.count; at++)
            all += tabs.itemAt(at) ? tabs.itemAt(at).natural + nav.spacing : 0
        return all
    }
    readonly property bool folded: leftSide.x + leftSide.width + Theme.gapMedium + sectionsWidth + Theme.gapCard + Theme.field + Theme.gapSmall + rightSide.spacing + playButton.width + Theme.gapCard > width

    implicitHeight: Theme.topBar
    color: Theme.bar

    Row {
        id: leftSide
        x: Theme.gapLarge
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.gap
        Image {
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.sized(20)
            height: width
            sourceSize: Qt.size(40, 40)
            source: "qrc:/qt/qml/RimStack/Ui/assets/rimstack.svg"
        }
        Text {
            visible: !bar.tight
            anchors.verticalCenter: parent.verticalCenter
            rightPadding: Theme.gapSmall
            color: Theme.brand
            font.pixelSize: Theme.title
            font.weight: Font.DemiBold
            text: Theme.say("RimStack")
        }
        Btn {
            id: listsButton
            visible: !Theme.demo
            anchors.verticalCenter: parent.verticalCenter
            icon: "list"
            menu: true
            text: Mods.prefs.listName !== "" ? Theme.fill("List: {}", Mods.prefs.listName) : Theme.say("Lists")
            tip: Theme.say("Load orders kept under a name, and the order as it was before each Sort")
            Rectangle {
                visible: Mods.dirty
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: -3
                width: 8
                height: 8
                radius: Math.min(width, height) / 2
                color: Theme.warn
                Tip { text: Theme.say("The list has changes that are not saved") }
            }
            onClicked: listsMenu.visible = !listsMenu.visible
            ListsMenu {
                id: listsMenu
                y: listsButton.height + 6
                onUseList: ids => bar.useList(ids)
                onNamed: name => Mods.prefs.listName = name
            }
        }
    }

    Rectangle {
        id: sectionsBox
        x: Math.max(leftSide.x + leftSide.width + Theme.gapMedium, Math.min((parent.width - width) / 2, rightSide.x - width - Theme.gapMedium))
        anchors.verticalCenter: parent.verticalCenter
        width: nav.width + 4
        height: nav.height + 4
        radius: Theme.cornerLarge
        color: Theme.seg
        border.width: 1
        border.color: Theme.fieldLine
        activeFocusOnTab: true
        Accessible.role: Accessible.PageTabList
        Accessible.name: Theme.say("Sections")
        Keys.onLeftPressed: bar.step(-1)
        Keys.onRightPressed: bar.step(1)
        Keys.onReturnPressed: if (bar.folded) bar.pickSection(sectionsBox)
        Keys.onSpacePressed: if (bar.folded) bar.pickSection(sectionsBox)
        FocusRing { corner: Theme.cornerLarge }
        Rectangle {
            readonly property Item at: tabs.count > 0 ? tabs.itemAt(bar.section) : null
            id: thumb
            property bool settled: false
            Component.onCompleted: Qt.callLater(() => thumb.settled = true)
            x: thumb.at ? nav.x + thumb.at.x : nav.x
            y: nav.y
            width: thumb.at ? thumb.at.width : 0
            height: Theme.strip
            radius: Theme.corner
            color: Theme.accent
            Behavior on x { enabled: thumb.settled; NumberAnimation { duration: Theme.calm; easing.type: Easing.OutCubic } }
            Behavior on width { enabled: thumb.settled; NumberAnimation { duration: Theme.calm; easing.type: Easing.OutCubic } }
        }
        Row {
            id: nav
            anchors.centerIn: parent
            spacing: Theme.gapLine
            Repeater {
                id: tabs
                model: bar.sections
                Rectangle {
                    id: tab
                    required property string modelData
                    required property int index
                    readonly property bool on: index === bar.section
                    readonly property int badge: index === 1 ? bar.behind : 0
                    readonly property real inset: bar.tight ? Theme.gap : Theme.gapCard
                    readonly property real natural: tabLabel.implicitWidth + 2 * inset + (badge > 0 ? tabBadge.width + Theme.gapSmall : 0)
                    visible: on || !bar.folded
                    width: natural + (bar.folded ? Theme.sized(12) + Theme.gapTiny : 0)
                    height: Theme.strip
                    radius: Theme.corner
                    color: "transparent"
                    Accessible.role: Accessible.PageTab
                    Accessible.name: Theme.say(modelData)
                    Accessible.selected: on
                    Tip {
                        text: Theme.fill("{} · Ctrl + {}", Theme.say(tab.modelData), tab.index + 1) + "\n" + [Theme.say("The load order and every installed mod"), Theme.say("Workshop updates, looked-up mods and Git checkouts"),
                               Theme.say("Faster-loading DDS textures and the files the game would refuse"), Theme.say("Saves and how their mods differ from the load order"),
                               Theme.say("The game's log: errors and the mods they point at"), Theme.say("Folders, appearance, sorting and upkeep")][tab.index]
                    }
                    Rectangle {
                        id: tabBadge
                        visible: tab.badge > 0
                        anchors.right: parent.right
                        anchors.rightMargin: Theme.gap
                        anchors.verticalCenter: parent.verticalCenter
                        width: Math.max(Theme.tag, tabBadgeText.implicitWidth + 2 * Theme.gapSmall)
                        height: Theme.sized(16)
                        radius: height / 2
                        color: tab.on ? Theme.accentInk : Theme.accent
                        Text { id: tabBadgeText; anchors.centerIn: parent; color: tab.on ? Theme.accent : Theme.onSelected; font.pixelSize: Theme.caption; font.weight: Font.DemiBold; text: tab.badge }
                    }
                    Text {
                        id: tabLabel
                        x: tab.inset
                        anchors.verticalCenter: parent.verticalCenter
                        color: tab.on ? Theme.accentInk : Theme.ink2
                        font.pixelSize: Theme.body
                        font.weight: tab.on ? Font.DemiBold : Font.Medium
                        text: Theme.say(tab.modelData)
                    }
                    Icon { visible: bar.folded; anchors.right: parent.right; anchors.rightMargin: Theme.gap; anchors.verticalCenter: parent.verticalCenter; name: "chevron-down"; size: Theme.sized(12); color: tabLabel.color }
                    TapHandler { onTapped: bar.folded ? bar.pickSection(tab) : bar.go(tab.index) }
                }
            }
        }
    }

    Row {
        id: rightSide
        anchors.right: parent.right
        anchors.rightMargin: Theme.gapCard
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.gapLarge
        Rectangle {
            id: searchBox
            readonly property real room: bar.width - Theme.gapCard - playButton.width - rightSide.spacing - (leftSide.x + leftSide.width + Theme.gapMedium + sectionsBox.width + Theme.gapMedium)
            readonly property bool small: room < Theme.sized(130)
            anchors.verticalCenter: parent.verticalCenter
            width: small ? Theme.field + 6 : Math.min(Theme.sized(250), room)
            height: Theme.field
            radius: Theme.corner
            color: Theme.well
            border.width: 1
            border.color: Theme.fieldLine
            activeFocusOnTab: true
            Accessible.role: Accessible.Button
            Accessible.name: Theme.say("Search mods, commands and settings")
            Keys.onReturnPressed: bar.search()
            Keys.onSpacePressed: bar.search()
            FocusRing {}
            Icon { id: searchIcon; x: Theme.gap; anchors.verticalCenter: parent.verticalCenter; name: "search"; size: Theme.icon - 1; color: Theme.ink3 }
            Text { visible: !searchBox.small; x: searchIcon.x + searchIcon.width + Theme.gap; width: keys.x - x - Theme.gap; elide: Text.ElideRight; anchors.verticalCenter: parent.verticalCenter; color: Theme.ink3; font.pixelSize: Theme.body; text: Theme.say("Search") }
            Text { id: keys; visible: !searchBox.small; anchors.right: parent.right; anchors.rightMargin: Theme.gap; anchors.verticalCenter: parent.verticalCenter; color: Theme.ink3; font.pixelSize: Theme.caption; text: Theme.say("Ctrl + K") }
            HoverHandler { id: searchHover; cursorShape: Qt.IBeamCursor }
            TapHandler { onTapped: bar.search() }
            Tip { text: Theme.say("Finds mods in both lists, commands and settings (Ctrl + K)"); over: searchHover.hovered }
        }
        Btn {
            id: playButton
            anchors.verticalCenter: parent.verticalCenter
            primary: true
            large: true
            icon: "play"
            enabled: !bar.gameRunning
            text: bar.gameRunning ? Theme.say("Running") : Theme.say("Play")
            tip: bar.gameRunning ? Theme.say("RimWorld is running. It reads the list of mods only when it starts.") : bar.inTheWay.length > 0 ? bar.inTheWay.join(". ") : Theme.say("Starts RimWorld through Steam. The newest save is copied first.")
            onClicked: bar.pressPlay()
            Rectangle {
                visible: bar.inTheWay.length > 0
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: -Theme.sized(6)
                width: Math.max(height, inTheWayCount.implicitWidth + Theme.sized(8))
                height: Theme.sized(17)
                radius: height / 2
                color: Mods.errors > 0 ? Theme.err : Theme.warn
                border.width: 2
                border.color: Theme.bar
                Text { id: inTheWayCount; anchors.centerIn: parent; color: Theme.win; font.pixelSize: Theme.caption; font.weight: Font.Bold; text: Theme.count(bar.inTheWay.length) }
            }
            Popup {
                enter: PopIn {}
                exit: PopOut {}
                id: beforePlay
                x: parent.width - width
                y: parent.height + 8
                width: Theme.sized(320)
                padding: Theme.gapLarge
                background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }
                contentItem: Column {
                    spacing: Theme.gap
                    Text { color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: Theme.say("Before you play") }
                    Repeater {
                        model: bar.inTheWay
                        Row {
                            id: way
                            required property string modelData
                            spacing: Theme.gap
                            Rectangle { id: dot; width: 7; height: 7; radius: Math.min(width, height) / 2; anchors.verticalCenter: parent.verticalCenter; color: Theme.warn }
                            Text { width: beforePlay.availableWidth - dot.width - way.spacing; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.body; text: way.modelData }
                        }
                    }
                    Btn { visible: Mods.dirty; width: parent.width; primary: true; text: Theme.say("Save and play"); onClicked: { beforePlay.close(); Mods.save(() => bar.play()) } }
                    Btn { visible: Mods.sortPending > 0 && !Theme.demo; width: parent.width; icon: "arrow-up-down"; text: Theme.say("Sort first"); tip: Theme.say("Shows the order the rules ask for, to keep or discard. Nothing is written before Save."); onClicked: { beforePlay.close(); bar.go(0); Mods.previewSort() } }
                    Btn { visible: Mods.errors > 0; width: parent.width; text: Theme.say("Show the problems"); onClicked: { beforePlay.close(); bar.go(0); Mods.problemsRequested() } }
                    Btn { visible: !Theme.demo && !Hub.search.active; width: parent.width; icon: "search"; text: Theme.say("Find the broken mod"); onClicked: { beforePlay.close(); Mods.findBrokenRequested() } }
                    Btn { width: parent.width; text: Mods.dirty ? Theme.say("Play with the saved list") : Theme.say("Play anyway"); onClicked: { beforePlay.close(); bar.play() } }
                }
            }
        }
    }

    Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.edge }
}
