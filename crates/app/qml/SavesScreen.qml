pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

Item {
    id: screen
    Appear { of: screen }

    signal useList(var ids)
    property bool allMissing: false
    property bool allAdded: false

    readonly property SavesData reader: Hub.saves
    readonly property var saves: reader.saves || []
    // With the RimSaves mod, saves sit in folders; "" shows every folder.
    readonly property string folder: reader.folder
    property int selected: 0
    property string listProblem
    readonly property var save: saves[selected] || null

    Binding { target: Hub.saves; property: "wanted"; value: screen.visible }

    function when(seconds) {
        return Theme.moment(seconds)
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            RowLayout {
                Layout.leftMargin: Theme.gapCard
                Layout.rightMargin: Theme.gapCard
                Layout.preferredHeight: Theme.header
                spacing: Theme.gap
                Text { color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: Theme.say("Saves") }
                Text { color: Theme.ink3; font.pixelSize: Theme.body; text: screen.reader.heading }
                Rectangle {
                    visible: (screen.reader.folders || []).length > 1
                    Layout.leftMargin: Theme.gap
                    implicitWidth: folderRow.width + 4
                    implicitHeight: folderRow.height + 4
                    radius: Theme.cornerLarge
                    color: Theme.seg
                    Row {
                        id: folderRow
                        anchors.centerIn: parent
                        spacing: Theme.gapLine
                        Repeater {
                            model: [""].concat(screen.reader.folders || [])
                            Rectangle {
                                id: folderTab
                                required property string modelData
                                readonly property bool on: modelData === screen.folder
                                width: folderText.implicitWidth + 2 * Theme.gapMedium
                                height: Theme.sized(24)
                                radius: Theme.corner
                                color: on ? Theme.ctlHi : "transparent"
                                Accessible.role: Accessible.PageTab
                                Accessible.name: modelData || Theme.say("All folders")
                                Text {
                                    id: folderText
                                    anchors.centerIn: parent
                                    color: folderTab.on ? Theme.ink : Theme.ink2
                                    font.pixelSize: Theme.small
                                    font.weight: Font.Medium
                                    text: (folderTab.modelData || Theme.say("All")) + (folderTab.modelData !== "" && folderTab.modelData === screen.reader.currentFolder ? " ●" : "")
                                }
                                TapHandler { onTapped: { Hub.saves.folder = folderTab.modelData; screen.selected = 0 } }
                            }
                        }
                    }
                }
                Item { Layout.fillWidth: true }
            }

            FilterField {
                visible: screen.reader.searchable
                Layout.fillWidth: true
                Layout.leftMargin: Theme.gapCard
                Layout.rightMargin: Theme.gapCard
                Layout.bottomMargin: Theme.gapSmall
                implicitHeight: Theme.sized(30)
                icon: "funnel"
                placeholder: Theme.say("Filter saves")
                onTextChanged: { Hub.saves.filter = text; screen.selected = 0 }
            }

            ListView {
                id: list
                Layout.fillWidth: true
                Layout.fillHeight: true
                topMargin: Theme.gapHair
                bottomMargin: Theme.gapMedium
                spacing: Theme.gapHair
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                Wheel {}
                ScrollBar.vertical: Scroller {}
                model: screen.saves
                RowKeys {
                    parent: list
                    view: list
                    title: Theme.say("Saves")
                    count: list.count
                    current: screen.selected
                    onChosen: index => screen.selected = index
                    onEntered: if (screen.save && screen.save.usable) screen.useList(screen.save.ids)
                }
                delegate: Item {
                    id: row
                    required property int index
                    required property var modelData
                    readonly property bool current: index === screen.selected
                    readonly property color quiet: current ? Theme.onSelectedDim : Theme.ink2
                    width: list.width
                    readonly property real endGap: Theme.gapPage + Theme.gapSmall
                    height: Math.max(screen.reader.pictured ? thumb.implicitHeight + 2 * Theme.gapMedium : 0, rowFacts.implicitHeight + 2 * Theme.gapMedium)
                    Accessible.role: Accessible.ListItem
                    Accessible.name: modelData.name + ", " + modelData.said
                    Rectangle {
                        anchors.fill: parent
                        anchors.leftMargin: Theme.gapLarge
                        anchors.rightMargin: Theme.gapCard
                        radius: Theme.corner
                        color: row.current ? Theme.accent : Theme.hover
                        opacity: row.current ? 1 : rowHover.hovered ? 0.05 : 0
                    }
                    Rectangle { visible: !row.current; anchors.bottom: parent.bottom; anchors.bottomMargin: -1; x: Theme.gapPage; width: parent.width - Theme.gapPage - row.endGap; height: 1; color: Theme.line; opacity: 0.6 }
                    HoverHandler { id: rowHover }
                    TapHandler { onTapped: screen.selected = row.index }
                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: Theme.gapPage
                        anchors.rightMargin: row.endGap
                        spacing: Theme.gapLarge
                        Rectangle {
                            id: thumb
                            visible: screen.reader.pictured
                            implicitWidth: Theme.sized(104)
                            implicitHeight: Theme.sized(62)
                            radius: Theme.corner
                            color: Theme.panel
                            border.width: row.modelData.preview ? 0 : 1
                            border.color: Theme.line
                            clip: true
                            Image {
                                anchors.fill: parent
                                visible: status === Image.Ready
                                asynchronous: true
                                cache: false
                                fillMode: Image.PreserveAspectCrop
                                sourceSize.width: Theme.sized(208)
                                source: row.modelData.preview ? Theme.fileUrl(row.modelData.preview) : ""
                            }
                            Icon { visible: !row.modelData.preview; anchors.centerIn: parent; name: "save"; size: Theme.sized(22); color: Theme.ink3 }
                        }
                        Column {
                            id: rowFacts
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            spacing: Theme.gapTiny
                            Text { width: parent.width; elide: Text.ElideRight; color: row.current ? Theme.onSelected : Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: row.modelData.name }
                            Text {
                                width: parent.width
                                elide: Text.ElideRight
                                color: row.quiet
                                font.pixelSize: Theme.small
                                text: screen.when(row.modelData.modified) + "   " + row.modelData.about
                            }
                            Row {
                                spacing: Theme.gapTiny
                                Chip { visible: row.modelData.verdict.unreadable; onAccent: row.current; tone: Theme.err; icon: "circle-alert"; text: Theme.say("cannot be read") }
                                Chip { visible: row.modelData.verdict.same; onAccent: row.current; tone: Theme.ok; icon: "check"; text: Theme.say("same mods as now") }
                                Chip { visible: row.modelData.notInstalled > 0; onAccent: row.current; tone: Theme.err; icon: "circle-alert"; text: Theme.fill("{} not installed", row.modelData.notInstalled); tip: Theme.say("Mods the save was played with that are not installed") }
                                Chip { visible: row.modelData.notActive > 0; onAccent: row.current; tone: Theme.warn; icon: "minus"; text: Theme.fill("{} not active", row.modelData.notActive); tip: Theme.say("Mods the save was played with that are not active now") }
                                Chip { visible: !!row.modelData.otherGame; onAccent: row.current; tone: screen.reader.allFrom !== "" ? Theme.ink2 : Theme.warn; text: Theme.say("other game version") }
                                Chip { visible: (row.modelData.added || []).length > 0; onAccent: row.current; text: Theme.fill("{} added", (row.modelData.added || []).length); tip: Theme.say("Mods active now that the save was not played with") }
                                Chip { visible: !!row.modelData.reordered; onAccent: row.current; text: Theme.say("reordered") }
                            }
                        }
                        Text { color: row.quiet; font.pixelSize: Theme.small; text: row.modelData.megabytes ? Theme.fill("{} MB", row.modelData.megabytes.toFixed(1)) : "" }
                    }
                }

                Column {
                    visible: screen.saves.length === 0
                    anchors.centerIn: parent
                    width: Math.min(parent.width - 4 * Theme.gapPage, Theme.sized(460))
                    spacing: Theme.gapSmall
                    Text { width: parent.width; horizontalAlignment: Text.AlignHCenter; color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: screen.reader.busy ? Theme.say("Reading saves…") : Theme.say("No saves") }
                    Text { visible: !screen.reader.busy; width: parent.width; horizontalAlignment: Text.AlignHCenter; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.body; lineHeight: 1.3; text: screen.folder !== "" ? Theme.fill("RimSaves folder {} is empty", screen.folder) : "" }
                }
            }
        }

        Rectangle { Layout.fillHeight: true; implicitWidth: 1; color: Theme.edge }

        Rectangle {
            Layout.preferredWidth: Math.min(Theme.sized(360), screen.width * 0.36)
            Layout.fillHeight: true
            color: Theme.panel

            Scrolled {
                id: side
                anchors.fill: parent
                padding: Theme.gapCard
                visible: screen.save !== null

                Rectangle {
                    visible: !!screen.save && !!screen.save.preview
                    width: parent.width
                    height: Theme.sized(190)
                    radius: Theme.cornerLarge
                    color: Theme.well
                    clip: true
                    Image {
                        anchors.fill: parent
                        asynchronous: true
                        cache: false
                        fillMode: Image.PreserveAspectCrop
                        sourceSize.width: Theme.sized(720)
                        source: screen.save && screen.save.preview ? Theme.fileUrl(screen.save.preview) : ""
                    }
                }
                Item { visible: !!screen.save && !!screen.save.preview; width: 1; height: Theme.gapLarge }
                Text { width: parent.width; wrapMode: Text.WrapAnywhere; color: Theme.ink; font.pixelSize: Theme.heading; font.weight: Font.DemiBold; text: screen.save ? screen.save.name : "" }
                Text {
                    width: parent.width
                    topPadding: Theme.gapHair
                    wrapMode: Text.WordWrap
                    color: Theme.ink2
                    font.pixelSize: Theme.body
                    text: screen.save ? (screen.reader.rimsaves ? Theme.fill("In {}, {}, {} MB", screen.save.folder, screen.when(screen.save.modified), screen.save.megabytes.toFixed(1)) : Theme.fill("{}, {} MB", screen.when(screen.save.modified), screen.save.megabytes.toFixed(1))) : ""
                }
                Flow {
                    width: parent.width
                    topPadding: Theme.gapMedium
                    spacing: Theme.gapSmall
                    Btn {
                        visible: !!screen.save && screen.save.usable
                        primary: !!screen.save && screen.save.verdict.breaks
                        icon: "list"
                        text: Theme.say("Use this save's mod list")
                        onClicked: screen.useList(screen.save.ids)
                    }
                    Btn { icon: "folder-open"; text: Theme.say("Open folder"); onClicked: Qt.openUrlExternally(Theme.fileUrl(screen.save.dir)) }
                }

                GroupTitle { visible: facts.count > 0; text: Theme.say("Colony") }
                Repeater {
                    id: facts
                    model: screen.save && screen.save.facts ? screen.save.facts : []
                    RowLayout {
                        id: fact
                        required property var modelData
                        width: side.column.width
                        height: Math.max(implicitHeight, Theme.sized(22))
                        Text { Layout.preferredWidth: Theme.sized(110); Layout.maximumWidth: side.column.width / 2; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say(fact.modelData[0]) }
                        Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: fact.modelData[1] }
                    }
                }

                Item { visible: !!screen.save && !!screen.save.ids; width: 1; height: Theme.gapLarge }
                RowLayout {
                    visible: !!screen.save && !!screen.save.ids
                    width: parent.width
                    spacing: Theme.gapSmall
                    FilterField {
                        id: listName
                        Layout.fillWidth: true
                        placeholder: Theme.say("Save its list as…")
                        onAccepted: if (keepList.enabled) keepList.clicked()
                    }
                    Btn {
                        id: keepList
                        enabled: listName.text.trim() !== ""
                        text: Theme.say("Keep")
                        onClicked: {
                            screen.listProblem = Hub.lists.keepIds(listName.text, screen.save.ids)
                            if (screen.listProblem === "")
                                listName.text = ""
                        }
                    }
                }
                Text { visible: screen.listProblem !== ""; width: parent.width; wrapMode: Text.WordWrap; color: Theme.err; font.pixelSize: Theme.body; text: screen.listProblem }

                GroupTitle { visible: !!screen.save && !!screen.save.problem; text: Theme.say("Problem") }
                Text { visible: !!screen.save && !!screen.save.problem; width: parent.width; wrapMode: Text.WordWrap; color: Theme.err; font.pixelSize: Theme.body; text: screen.save ? Theme.fill("Unreadable: {}", (screen.save.problem || "")) : "" }

                GroupTitle { visible: !!screen.save && !!screen.save.ids; text: Theme.say("Differences from the current list") }
                Text {
                    visible: !!screen.save && !!screen.save.ids && screen.save.missing.length === 0 && !screen.save.verdict.same
                    width: parent.width
                    wrapMode: Text.WordWrap
                    color: Theme.ink2
                    font.pixelSize: Theme.body
                    text: Theme.say("Nothing the save needs is missing.")
                }
                Text { visible: !!screen.save && screen.save.verdict.same; width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.body; lineHeight: 1.3; text: Theme.say("Same mods, same order as the load order") }

                GroupTitle { visible: !!screen.save && !!screen.save.otherGame; text: Theme.say("Other game version") }
                Text { visible: !!screen.save && !!screen.save.otherGame; width: parent.width; wrapMode: Text.WordWrap; color: Theme.warn; font.pixelSize: Theme.body; lineHeight: 1.3; text: screen.save ? Theme.fill("Saved with RimWorld {}, not the installed version", (screen.save.gameVersion || "")) : "" }

                Text {
                    visible: !!screen.save && screen.save.missing.length > 0
                    width: parent.width
                    topPadding: Theme.gapTiny
                    wrapMode: Text.WordWrap
                    color: screen.save && screen.save.notInstalled > 0 ? Theme.err : Theme.ink2
                    font.pixelSize: Theme.body
                    text: screen.save ? screen.save.missingSaid : ""
                }
                Repeater {
                    id: missing
                    model: screen.save && screen.save.missing ? (screen.allMissing ? screen.save.missing : screen.save.missing.slice(0, 5)) : []
                    RowLayout {
                        id: gone
                        required property var modelData
                        width: side.column.width
                        height: Math.max(implicitHeight, Theme.sized(22))
                        spacing: Theme.gap
                        Icon { name: gone.modelData.installed ? "minus" : "circle-alert"; color: gone.modelData.installed ? Theme.ink3 : Theme.err }
                        Text { Layout.fillWidth: true; elide: Text.ElideRight; color: gone.modelData.installed ? Theme.ink : Theme.err; font.pixelSize: Theme.body; text: gone.modelData.name }
                        Text { color: Theme.ink3; font.pixelSize: Theme.small; text: gone.modelData.installed ? Theme.say("not active") : Theme.say("not installed") }
                    }
                }
                Btn {
                    visible: !!screen.save && (screen.save.missing || []).length > 5
                    text: screen.allMissing ? Theme.say("Show five") : Theme.fill("{} more", ((screen.save ? screen.save.missing.length : 0) - 5))
                    onClicked: screen.allMissing = !screen.allMissing
                }

                Text {
                    readonly property int count: screen.save && screen.save.added ? screen.save.added.length : 0
                    visible: count > 0
                    width: parent.width
                    topPadding: Theme.gap
                    color: Theme.ink2
                    font.pixelSize: Theme.body
                    text: Theme.plural(count, "{} mod added since", "{} mods added since")
                }
                Repeater {
                    id: added
                    model: screen.save && screen.save.added ? (screen.allAdded ? screen.save.added : screen.save.added.slice(0, 5)) : []
                    RowLayout {
                        id: fresh
                        required property string modelData
                        width: side.column.width
                        height: Math.max(implicitHeight, Theme.sized(22))
                        spacing: Theme.gap
                        Icon { name: "plus"; color: Theme.ink3 }
                        Text { Layout.fillWidth: true; elide: Text.ElideMiddle; textFormat: Text.PlainText; color: Theme.ink; font.pixelSize: Theme.body; text: fresh.modelData }
                    }
                }
                Btn {
                    visible: !!screen.save && (screen.save.added || []).length > 5
                    text: screen.allAdded ? Theme.say("Show five") : Theme.fill("{} more", ((screen.save ? screen.save.added.length : 0) - 5))
                    onClicked: screen.allAdded = !screen.allAdded
                }

                Item { visible: !!screen.save && !!screen.save.reordered; width: 1; height: Theme.gapTiny }
                Text { visible: !!screen.save && !!screen.save.reordered; width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.body; lineHeight: 1.3; text: Theme.say("Shared mods are in a different order now") }

                GroupTitle { text: Theme.say("Backups") }
                Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.body; text: Theme.say("The newest save is copied before each launch; the last 5 are kept") }
                Item { width: 1; height: Theme.gapSmall }
                Btn { text: Theme.say("Open the backups folder"); onClicked: Qt.openUrlExternally(Theme.fileUrl(Hub.settings.report.dataFolder + "/save-backups")) }
            }
        }
    }
}
