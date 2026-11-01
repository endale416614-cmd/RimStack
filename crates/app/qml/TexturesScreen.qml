pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

Item {
    id: screen
    Appear { of: screen }

    readonly property TexturesData tools: Hub.textures
    readonly property var report: tools.report || ({})
    readonly property var shown: report.rows || []
    readonly property string usual: report.usual || ""
    readonly property var remaining: report.remaining || []
    readonly property var stale: report.stale || []
    readonly property var made: report.made || []
    readonly property int madeFiles: report.madeFiles || 0
    property int selectedId: -1
    readonly property var mod: shown.find(m => m.id === selectedId) || null
    property bool confirmRemove: false

    Binding { target: Hub.textures; property: "wanted"; value: screen.visible }

    readonly property real nameWidth: Theme.sized(460)
    readonly property real countWidth: Math.max(countHeading.advanceWidth, widestCount.advanceWidth)
    readonly property real pngWidth: Math.max(pngHeading.advanceWidth, widestSize.advanceWidth)
    readonly property real ddsWidth: Math.max(ddsHeading.advanceWidth, widestSize.advanceWidth, notConverted.advanceWidth)
    TextMetrics { id: countHeading; font.pixelSize: Theme.caption; font.weight: Font.Medium; text: Theme.say("Textures") }
    TextMetrics { id: pngHeading; font.pixelSize: Theme.caption; font.weight: Font.Medium; text: Theme.say("PNG") }
    TextMetrics { id: ddsHeading; font.pixelSize: Theme.caption; font.weight: Font.Medium; text: Theme.say("DDS") }
    TextMetrics { id: widestCount; font.pixelSize: Theme.body; text: Theme.count(99999) }
    TextMetrics { id: widestSize; font.pixelSize: Theme.body; text: screen.megabytes(99999) }
    TextMetrics { id: notConverted; font.pixelSize: Theme.small; text: Theme.say("Not converted") }

    function megabytes(n) {
        if (n <= 0)
            return ""
        if (n < 0.1)
            return Theme.fill("{} KB", Math.max(1, Math.round(n * 1024)))
        return n < 10 ? Theme.fill("{} MB", n.toFixed(1)) : Theme.fill("{} MB", Theme.count(Math.round(n)))
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            Rectangle {
                Layout.fillWidth: true
                Layout.margins: Theme.gapLarge
                Layout.bottomMargin: Theme.gap
                implicitHeight: summary.implicitHeight + 2 * Theme.gap
                color: "transparent"
                readonly property int total: screen.report.total || 0
                readonly property int have: screen.report.have || 0
                ColumnLayout {
                    id: summary
                    x: 2
                    y: 8
                    width: parent.width - 4
                    spacing: Theme.gap
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Theme.gap
                        Flow {
                            Layout.fillWidth: true
                            spacing: Theme.gap
                            Row {
                                spacing: Theme.gap
                                Text { id: have; color: Theme.ink; font.pixelSize: Theme.heading; font.weight: Font.DemiBold; font.features: { "tnum": 1 }; text: Theme.fill("{} of {}", Theme.count(summary.parent.have), Theme.count(summary.parent.total)) }
                                Text { anchors.baseline: have.baseline; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say("textures have a DDS"); Tip { text: Theme.meaning("DDS") } }
                            }
                            Chip {
                                maxWidth: parent.width
                                visible: !!screen.report.sizeSaid
                                text: screen.report.sizeSaid || ""
                            }
                            Chip {
                                visible: (screen.report.waiting || 0) > 0
                                pressable: true
                                tone: Theme.warn
                                text: Theme.plural(screen.report.waiting || 0, "{} mod to convert", "{} mods to convert")
                                tip: Theme.say("Shows the first of them")
                                onClicked: { screen.selectedId = screen.report.firstWaiting; list.positionViewAtBeginning() }
                            }
                            Chip { visible: (screen.report.outOfDate || 0) > 0; tone: Theme.warn; icon: "triangle-alert"; text: Theme.fill("{} out of date", Theme.count(screen.report.outOfDate)) }
                        }
                        Btn {
                            Layout.alignment: Qt.AlignTop
                            text: Theme.say("Check all mods")
                            tip: Theme.say("Reads the header of every DDS file of the installed mods: not a DDS, cut short, a format the game has no reader for, or compressed with a size that is not a multiple of four.")
                            enabled: !screen.tools.auditing
                            onClicked: screen.tools.audit()
                        }
                        Choice { Layout.alignment: Qt.AlignTop; title: Theme.say("Mods"); options: ["Active mods", "All mods"]; current: screen.tools.activeOnly ? 0 : 1; onPicked: index => Hub.textures.activeOnly = index === 0 }
                    }
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: 6
                        radius: Math.min(width, height) / 2
                        color: Theme.chip
                        Accessible.role: Accessible.ProgressBar
                        Accessible.name: Theme.say("Textures with a DDS")
                        Accessible.description: have.text
                        Rectangle { width: summary.parent.total > 0 ? parent.width * summary.parent.have / summary.parent.total : 0; Behavior on width { NumberAnimation { duration: Theme.calm; easing.type: Easing.OutCubic } } height: parent.height; radius: Math.min(width, height) / 2; color: summary.parent.have === summary.parent.total ? Theme.ok : Theme.accent }
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Theme.gapWide
                        Flow {
                            Layout.fillWidth: true
                            spacing: Theme.gap
                            Text {
                                visible: text !== ""
                                width: Math.min(implicitWidth, parent.width)
                                height: Theme.control
                                verticalAlignment: Text.AlignVCenter
                                elide: Text.ElideRight
                                color: Theme.ink2
                                font.pixelSize: Theme.small
                                text: screen.tools.busy ? (screen.tools.progress || Theme.say("Counting textures…")) : screen.tools.progress
                            }
                            Btn { visible: screen.tools.busy; text: Theme.say("Cancel"); tip: Theme.say("Stops after the mod it is on; what is done stays done"); onClicked: screen.tools.cancel() }
                            Btn {
                                primary: screen.remaining.length > 0
                                enabled: !screen.tools.busy && screen.remaining.length > 0
                                text: Theme.say("Convert remaining")
                                tip: Theme.say("Writes a .dds beside each .png that has none, inside each mod's own folder. The .png files are not changed.")
                                onClicked: screen.tools.convert(screen.remaining)
                            }
                            Btn {
                                enabled: !screen.tools.busy && screen.stale.length > 0
                                text: Theme.say("Rebuild out of date")
                                tip: Theme.say("Converts again where the .png changed after its .dds was made.")
                                onClicked: screen.tools.convert(screen.stale)
                            }
                        }
                        Btn {
                            Layout.alignment: Qt.AlignTop
                            enabled: !screen.tools.busy && screen.made.length > 0
                            danger: true
                            text: Theme.say("Remove all converted…")
                            tip: Theme.say("Deletes the .dds files RimStack made, inside the mods' folders. Files it did not make are not touched.")
                            onClicked: screen.confirmRemove = true
                        }
                    }
                }
            }

            Rectangle {
                visible: screen.confirmRemove
                Layout.fillWidth: true
                Layout.leftMargin: Theme.gapLarge
                Layout.rightMargin: Theme.gapLarge
                Layout.bottomMargin: Theme.gap
                implicitHeight: Theme.sized(40)
                radius: Theme.corner
                color: Theme.dangerBg
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Theme.gapLarge
                    anchors.rightMargin: Theme.gap
                    spacing: Theme.gap
                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        color: Theme.ink
                        font.pixelSize: Theme.body
                        text: Theme.plural(screen.madeFiles, "Delete {} DDS file made by RimStack ({})? The PNG files stay.", "Delete {} DDS files made by RimStack ({})? The PNG files stay.", Theme.plural(screen.made.length, "{} mod", "{} mods"))
                    }
                    Btn { text: Theme.say("Cancel"); onClicked: screen.confirmRemove = false }
                    Btn {
                        danger: true
                        text: Theme.say("Delete them")
                        onClicked: {
                            screen.confirmRemove = false
                            screen.tools.remove(screen.made)
                        }
                    }
                }
            }

            Item {
                Layout.fillWidth: true
                Layout.preferredHeight: Theme.sized(24)
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Theme.gapWide
                    anchors.rightMargin: Theme.gapWide
                    spacing: Theme.gapLarge
                    Text { Layout.preferredWidth: screen.nameWidth; Layout.minimumWidth: Theme.sized(80); color: Theme.ink3; font.pixelSize: Theme.caption; font.weight: Font.Medium; text: Theme.say("Mod") }
                    Text { Layout.preferredWidth: screen.countWidth; horizontalAlignment: Text.AlignRight; color: Theme.ink3; font.pixelSize: Theme.caption; font.weight: Font.Medium; text: Theme.say("Textures") }
                    Text { Layout.preferredWidth: screen.pngWidth; horizontalAlignment: Text.AlignRight; color: Theme.ink3; font.pixelSize: Theme.caption; font.weight: Font.Medium; text: Theme.say("PNG") }
                    Text { Layout.preferredWidth: screen.ddsWidth; horizontalAlignment: Text.AlignRight; color: Theme.ink3; font.pixelSize: Theme.caption; font.weight: Font.Medium; text: Theme.say("DDS") }
                    Item { Layout.fillWidth: true }
                }
                Rectangle { anchors.bottom: parent.bottom; x: Theme.gapSmall; width: parent.width - 2 * Theme.gapSmall; height: 1; color: Theme.line }
            }

            ListView {
                id: list
                Layout.fillWidth: true
                Layout.fillHeight: true
                topMargin: Theme.gapTiny
                clip: true
                reuseItems: true
                boundsBehavior: Flickable.StopAtBounds
                Wheel {}
                ScrollBar.vertical: Scroller {}
                model: screen.shown
                RowKeys {
                    parent: list
                    view: list
                    title: Theme.say("Mods with textures")
                    count: list.count
                    current: screen.shown.findIndex(m => m.id === screen.selectedId)
                    onChosen: index => screen.selectedId = screen.shown[index].id
                }
                delegate: Item {
                    id: row
                    required property var modelData
                    readonly property bool current: modelData.id === screen.selectedId
                    readonly property color quiet: current ? Theme.onSelectedDim : Theme.ink2
                    readonly property color faint: current ? Theme.onSelectedDim : Theme.ink3
                    readonly property string verb: modelData.outOfDate > 0 ? Theme.say("Rebuild") : Theme.say("Convert")
                    readonly property bool settled: modelData.notConverted === 0 && modelData.outOfDate === 0
                    Accessible.role: Accessible.ListItem
                    Accessible.name: Theme.fill("{}, {}, {}", modelData.name, Theme.plural(modelData.textures, "{} texture", "{} textures"), Theme.say(modelData.state))
                    width: list.width
                    height: Theme.row
                    opacity: settled && !current ? 0.6 : 1
                    Rectangle {
                        anchors.fill: parent
                        anchors.leftMargin: Theme.gapSmall
                        anchors.rightMargin: Theme.gapSmall
                        radius: Theme.corner
                        color: row.current ? Theme.accent : Theme.hover
                        opacity: row.current ? 1 : rowHover.hovered ? 0.04 : 0
                    }
                    HoverHandler { id: rowHover }
                    TapHandler { onTapped: screen.selectedId = row.current ? -1 : row.modelData.id }
                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: Theme.gapWide
                        anchors.rightMargin: Theme.gapWide
                        spacing: Theme.gapLarge
                        RowLayout {
                            id: cell
                            // A layout inside a layout takes spare room unless told not to;
                            // the heading's cell does not.
                            Layout.fillWidth: false
                            Layout.preferredWidth: screen.nameWidth
                            Layout.minimumWidth: Theme.sized(80)
                            spacing: Theme.gap
                            Text { Layout.maximumWidth: screen.nameWidth - badge.width - cell.spacing; elide: Text.ElideRight; color: row.current ? Theme.onSelected : Theme.ink; font.pixelSize: Theme.body; text: row.modelData.name }
                            Chip {
                                id: badge
                                visible: row.modelData.state !== screen.usual
                                maxWidth: Theme.sized(190)
                                Layout.alignment: Qt.AlignVCenter
                                onAccent: row.current
                                tone: row.modelData.complete ? Theme.ok : row.modelData.stale ? Theme.warn : Theme.ink2
                                icon: row.modelData.complete ? "check" : row.modelData.stale ? "triangle-alert" : ""
                                text: row.modelData.said
                            }
                            Item { Layout.fillWidth: true }
                        }
                        Text { Layout.preferredWidth: screen.countWidth; horizontalAlignment: Text.AlignRight; color: row.quiet; font.pixelSize: Theme.body; text: Theme.count(row.modelData.textures) }
                        Text { Layout.preferredWidth: screen.pngWidth; horizontalAlignment: Text.AlignRight; color: row.quiet; font.pixelSize: Theme.body; text: screen.megabytes(row.modelData.pngMb) }
                        Item {
                            Layout.preferredWidth: screen.ddsWidth
                            implicitHeight: made.implicitHeight
                            Text { id: made; anchors.right: parent.right; visible: row.modelData.ddsMb > 0; color: row.quiet; font.pixelSize: Theme.body; text: screen.megabytes(row.modelData.ddsMb) }
                            Text { anchors.right: parent.right; anchors.verticalCenter: made.verticalCenter; visible: !made.visible; color: row.faint; font.pixelSize: Theme.small; text: Theme.say("Not converted") }
                        }
                        Btn {
                            visible: row.modelData.notConverted + row.modelData.outOfDate > 0 && !row.modelData.skipped
                            enabled: !screen.tools.busy
                            text: row.verb
                            spoken: Theme.fill("{}: {}", row.verb, row.modelData.name)
                            onClicked: screen.tools.convert([row.modelData.id])
                        }
                        Item { Layout.fillWidth: true }
                    }
                }

                Column {
                    visible: screen.shown.length === 0
                    anchors.centerIn: parent
                    width: Math.min(parent.width - 4 * Theme.gapPage, Theme.sized(460))
                    spacing: Theme.gapSmall
                    Text { width: parent.width; horizontalAlignment: Text.AlignHCenter; color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: screen.tools.busy ? Theme.say("Counting textures…") : Theme.say("No textures") }
                    Text { visible: !screen.tools.busy; width: parent.width; horizontalAlignment: Text.AlignHCenter; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.body; lineHeight: 1.3; text: (screen.report.all || 0) === 0 ? "" : Theme.say("None in active mods") }
                }
            }
        }

        Rectangle { visible: details.visible; Layout.fillHeight: true; implicitWidth: 1; color: Theme.edge }

        Rectangle {
            id: details
            visible: screen.mod !== null || ddsAudit.found !== null
            Layout.preferredWidth: Math.min(Theme.sized(360), screen.width * 0.3)
            Layout.fillHeight: true
            color: Theme.panel

            Column {
                id: side
                x: Theme.gapCard
                y: Theme.gapCard
                width: parent.width - 2 * Theme.gapCard

                Column {
                    width: parent.width
                    visible: screen.mod !== null
                    Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.heading; font.weight: Font.DemiBold; text: screen.mod ? screen.mod.name : "" }
                    Text { width: parent.width; topPadding: Theme.gapHair; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.body; text: screen.mod ? Theme.fill(screen.mod.active ? "{} textures, {}" : "{} textures, {}, not active", Theme.count(screen.mod.textures), screen.mod.said.toLowerCase()) : "" }

                    GroupTitle { text: Theme.say("Converted files") }
                    Repeater {
                        model: screen.mod ? [
                            [Theme.say("Created by RimStack"), [Theme.plural(screen.mod.converted + screen.mod.outOfDate, "{} file", "{} files"), screen.mod.madeMb > 0 ? screen.megabytes(screen.mod.madeMb) : ""].filter(part => part !== "").join(", "), false],
                            [Theme.say("Not made by RimStack"), Theme.plural(screen.mod.shipped, "{} file", "{} files"), false],
                            [Theme.say("Source PNG changed since"), Theme.plural(screen.mod.outOfDate, "{} file", "{} files"), screen.mod.outOfDate > 0],
                            [Theme.say("No DDS yet"), Theme.plural(screen.mod.notConverted, "{} file", "{} files"), false],
                        ] : []
                        RowLayout {
                            id: fact
                            required property var modelData
                            width: side.width
                            height: Theme.sized(24)
                            Text { Layout.fillWidth: true; color: fact.modelData[2] ? Theme.warn : Theme.ink; font.pixelSize: Theme.body; text: fact.modelData[0] }
                            Text { color: Theme.ink2; font.pixelSize: Theme.body; text: fact.modelData[1] }
                        }
                    }
                    Flow {
                        width: parent.width
                        topPadding: Theme.gap
                        spacing: Theme.gapSmall
                        Btn {
                            enabled: !screen.tools.busy && !!screen.mod && screen.mod.notConverted + screen.mod.outOfDate > 0
                            text: !!screen.mod && screen.mod.outOfDate > 0 ? Theme.say("Rebuild") : Theme.say("Convert")
                            onClicked: screen.tools.convert([screen.mod.id])
                        }
                        Btn {
                            enabled: !screen.tools.busy && !!screen.mod && screen.mod.converted + screen.mod.outOfDate > 0
                            text: Theme.say("Remove converted")
                            tip: Theme.say("Deletes only the .dds files RimStack made and that have not changed since.")
                            onClicked: screen.tools.remove([screen.mod.id])
                        }
                        Btn {
                            enabled: !screen.tools.busy
                            text: !!screen.mod && screen.mod.skipped ? Theme.say("Stop skipping") : Theme.say("Skip this mod")
                            onClicked: screen.tools.skip(screen.mod.id, !screen.mod.skipped)
                        }
                    }
                }

                GroupTitle { visible: ddsAudit.found !== null; width: parent.width; wrapMode: Text.WordWrap; text: Theme.say("All mods: DDS files the game would refuse") }
                Column {
                    id: ddsAudit
                    readonly property var found: screen.tools.refused || null
                    visible: found !== null
                    width: parent.width
                    spacing: Theme.gapSmall
                    Row {
                        spacing: Theme.gap
                        Btn { text: Theme.say("Close"); onClicked: screen.tools.closeAudit() }
                        Btn {
                            visible: !!ddsAudit.found && ddsAudit.found.length > 0
                            text: Theme.say("Set all aside…")
                            tip: Theme.say("Renames each to .dds.rimstack-bad, so the game loads the PNG beside it.")
                            onClicked: Notices.ask({
                                title: Theme.fill("Set {} DDS files aside", ddsAudit.found.length),
                                lines: [Theme.say("Each is renamed to .dds.rimstack-bad inside its own mod's folder, so the game loads the PNG beside it. The first of them:"), { path: ddsAudit.found[0].file }],
                                label: Theme.say("Set aside"),
                                run: () => screen.tools.setRefusedAside() })
                        }
                    }
                    Text {
                        visible: ddsAudit.found !== null
                        color: Theme.ink
                        font.pixelSize: Theme.body
                        text: ddsAudit.found === null ? "" : ddsAudit.found.length === 0 ? Theme.say("None") : Theme.plural(ddsAudit.found.length, "{} file", "{} files")
                    }
                    Repeater {
                        model: ddsAudit.found === null ? [] : ddsAudit.found.slice(0, 60)
                        Column {
                            id: refused
                            required property var modelData
                            width: ddsAudit.width
                            Text { width: parent.width; elide: Text.ElideLeft; color: Theme.ink; font.pixelSize: Theme.small; text: refused.modelData.mod + " — " + refused.modelData.name }
                            Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink3; font.pixelSize: Theme.small; text: refused.modelData.why }
                        }
                    }
                }

                GroupTitle { text: Theme.say("Quality") }
                Text {
                    width: parent.width
                    wrapMode: Text.WordWrap
                    color: Theme.ink
                    font.pixelSize: Theme.body
                    lineHeight: 1.3
                    text: Theme.say("BC7 with transparency, DXT1 without; with mipmaps")
                    Tip { text: Theme.say("A .dds file is written beside each .png and the game loads it instead: quicker, and less memory. The .png is not changed. Steam may delete the .dds files when it updates a mod; they then show as not converted.") }
                }
            }
        }
    }
}
