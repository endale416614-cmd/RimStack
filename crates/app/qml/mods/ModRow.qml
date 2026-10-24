pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

Item {
    id: row
    required property int index
    required property bool isHeader
    required property bool isSub
    required property string loadTime
    required property bool again
    required property bool versionIgnored
    required property string name
    required property string author
    required property string version
    required property int number
    required property int count
    required property bool collapsed
    required property int category
    required property int modIndex
    required property int modId
    // The number this mod had before the sort on show; 0 if it stays.
    required property int movedFrom
    // 0 for none, 1 for a warning, 2 for an error.
    required property int trouble
    required property string tags
    required property string troubleText
    required property int groupSize
    required property bool local
    required property string picture
    // 0 Workshop, 1 local, 2 a Git checkout, 3 the game's own.
    required property int source
    // Its start-up time as a part of the slowest mod's, 0 to 1.
    required property real loadPart
    required property bool fresh
    required property bool outdated
    required property string madeFor
    required property string whyMoved
    required property bool twin
    readonly property bool comfortable: columns && Theme.stored.comfortable

    readonly property var listView: ListView.view
    readonly property var ownModel: listView.model
    readonly property bool columns: listView.columns
    readonly property bool current: !isHeader && modId >= 0 && (modId === Mods.selectedId || Mods.also[modId] === true)
    readonly property color quiet: current ? Theme.onSelectedDim : Theme.ink2

    Accessible.role: isHeader ? Accessible.Heading : Accessible.ListItem
    Accessible.name: isHeader ? name + ", " + count
        : [name, columns ? Theme.fill("{} of {}", number, ownModel.count) : "", troubleText, fresh ? Theme.say("new") : ""].filter(part => part !== "").join(", ")
    Accessible.selectable: !isHeader
    Accessible.selected: current

    width: listView.width - listView.barRoom
    height: isSub ? Theme.sized(24) : again ? Theme.sized(22) : isHeader ? Theme.sized(32) : comfortable ? Math.max(Theme.sized(52), Theme.row + 10) : Theme.row

    property real offset: 0
    transform: Translate { y: row.offset }
    NumberAnimation { id: slideIn; target: row; property: "offset"; to: 0; duration: Theme.quick; easing.type: Easing.OutCubic }
    function slideFrom(places) {
        if (Theme.stored.reduceMotion || places === 0)
            return
        offset = Math.max(-400, Math.min(400, places * height))
        slideIn.restart()
    }
    function settle() {
        if (isHeader || !columns)
            return
        if (Mods.sorting && movedFrom > 0)
            slideFrom(movedFrom - number)
        else if (Mods.slid.modId >= 0 && Mods.slid.modId === modId)
            slideFrom(Mods.slid.by)
    }
    Component.onCompleted: settle()
    ListView.onReused: { offset = 0; settle() }
    onModIdChanged: settle()

    // "82 ms" or "2.6 s", as the model writes it.
    readonly property real loadSeconds: loadTime === "" ? 0 : parseFloat(loadTime) / (loadTime.endsWith("ms") ? 1000 : 1)
    readonly property color categoryColour: Theme.categoryColours[category] || Theme.ink3
    readonly property int movedBy: movedFrom > 0 ? Math.abs(movedFrom - number) : 0

    Rectangle {
        x: 2
        y: row.isHeader && !row.isSub && !row.again ? 10 : 0
        width: 3
        height: parent.height - y
        radius: row.isHeader && !row.isSub && !row.again ? 1.5 : 0
        color: row.categoryColour
        opacity: row.again ? 0.3 : 0.6
    }

    // The edge line keeps the tint from passing for the hover highlight.
    Item {
        visible: row.outdated && !row.current
        anchors.fill: parent
        anchors.leftMargin: Theme.gap
        anchors.rightMargin: Theme.gapSmall
        Rectangle { anchors.fill: parent; color: Theme.warn; opacity: 0.14 }
        Rectangle { width: 2; height: parent.height; color: Theme.warn }
    }

    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: Theme.gap
        anchors.rightMargin: Theme.gapSmall
        radius: Theme.corner
        readonly property bool conflicts: row.columns && !row.isHeader && !!Mods.conflictMarks[row.modId]
        color: row.current ? Theme.accent : conflicts ? Theme.warn : row.movedFrom > 0 ? Theme.moved : Theme.hover
        opacity: row.current ? 1 : conflicts ? 0.12 : row.movedFrom > 0 ? 0.1 : hover.hovered && !row.isHeader ? 0.04 : 0
    }
    HoverHandler { id: hover }
    Tip { text: !row.isHeader && nameText.truncated ? row.name : ""; over: hover.hovered && !quick.hovered }

    Row {
        visible: row.isSub
        anchors.verticalCenter: parent.verticalCenter
        x: 40
        spacing: Theme.gapSmall
        Text { width: Theme.sized(10); color: Theme.ink3; font.pixelSize: Theme.caption; text: row.collapsed ? "▸" : "▾" }
        Text { color: Theme.ink2; font.pixelSize: Theme.small; text: row.name; Tip { text: row.collapsed ? Theme.say("Click to show its mods") : Theme.say("The mods whose first tag this is. Click to fold them away.") } }
        Text { color: Theme.ink3; font.pixelSize: Theme.small; text: row.groupSize > row.count ? Theme.fill("{} of {}", Theme.count(row.count), Theme.count(row.groupSize)) : Theme.count(row.count) }
        Text {
            visible: row.again
            color: Theme.ink3
            font.pixelSize: Theme.small
            text: Theme.say("the rest of it")
            Tip { text: Theme.say("This group has mods further up as well: a rule keeps them apart, and Sort follows rules before groups.") }
        }
    }
    Rectangle {
        visible: row.isSub && hover.hovered
        anchors.right: parent.right
        anchors.rightMargin: Theme.gapCard
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.sized(24)
        height: Theme.sized(24)
        radius: Theme.corner
        color: subOff.hovered ? Theme.ctlHi : Theme.ctl
        Accessible.role: Accessible.Button
        Accessible.ignored: !visible
        Accessible.name: Theme.fill("Remove the tag {} from these mods", row.name)
        Icon { anchors.centerIn: parent; name: "x"; size: Theme.sized(12); color: Theme.ink }
        HoverHandler { id: subOff; cursorShape: Qt.PointingHandCursor }
        Accessible.onPressAction: Mods.removeSub(row.ownModel, row.index)
        TapHandler { onTapped: Mods.removeSub(row.ownModel, row.index) }
        Tip { text: Theme.say("Takes this tag off these mods; they stay where they are"); over: subOff.hovered }
    }
    Row {
        visible: row.isHeader && !row.isSub && !row.again
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.gapSmall
        x: 16
        spacing: Theme.gapSmall
        Text { width: Theme.sized(10); color: Theme.ink2; font.pixelSize: Theme.caption; text: row.collapsed ? "▸" : "▾" }
        Icon { name: Theme.categoryIcons[row.category] || ""; color: row.categoryColour; size: 14; anchors.verticalCenter: parent.verticalCenter }
        Text { color: Theme.ink; font.pixelSize: Theme.small; font.weight: Font.DemiBold; text: Theme.say(row.name); Tip { text: row.collapsed ? Theme.say("Click to show its mods") : Theme.say("Click to fold its mods away") } }
        Text { color: Theme.ink3; font.pixelSize: Theme.small; text: Theme.count(row.count) }
    }
    Item {
        visible: row.again && !row.isSub
        anchors.fill: parent
        anchors.leftMargin: Theme.gapWide
        anchors.rightMargin: Theme.gapWide
        Row {
            id: stray
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.gapSmall
            Text { width: Theme.sized(10); color: Theme.ink3; font.pixelSize: Theme.caption; text: row.collapsed ? "▸" : "▾" }
            Text { color: Theme.ink3; font.pixelSize: Theme.caption; text: Theme.say(row.name) + "  " + Theme.count(row.count) + "  " + Theme.say("out of category order") }
        }
        Row {
            anchors.left: stray.right
            anchors.leftMargin: Theme.gap
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            height: 1
            clip: true
            spacing: Theme.gapTiny
            Repeater {
                model: row.again ? Math.ceil(row.width / 8) : 0
                Rectangle { width: 4; height: 1; color: Theme.line }
            }
        }
        Tip { text: Theme.say("This category has mods further up as well. Sort brings a category's mods together.") }
    }

    RowLayout {
        id: cells
        visible: !row.isHeader
        anchors.fill: parent
        anchors.leftMargin: Theme.gapWide
        anchors.rightMargin: Theme.gapWide
        spacing: Theme.gapLarge
        readonly property var mark: row.columns ? (Mods.conflictMarks[row.modId] || null) : null
        Text {
            visible: row.columns
            Layout.preferredWidth: Theme.sized(28)
            horizontalAlignment: Text.AlignRight
            color: row.current ? Theme.onSelectedDim : Theme.ink3
            font.pixelSize: Theme.small
            font.features: { "tnum": 1 }
            text: row.number
        }
        RowLayout {
            id: nameCell
            Layout.fillWidth: true
            spacing: Theme.gapSmall
            Icon {
                name: Theme.categoryIcons[row.category] || ""
                size: Theme.sized(13)
                color: row.current ? Theme.onSelected : row.categoryColour
                Tip { text: Theme.say(Theme.categoryNames[row.category] || "") }
            }
            Rectangle {
                visible: row.comfortable && row.width >= Theme.sized(420)
                implicitWidth: Theme.sized(64)
                implicitHeight: Theme.sized(40)
                radius: Theme.corner
                color: Theme.ctl
                // Read at the size shown and off the window's thread, so
                // scrolling does not wait for it.
                Image {
                    anchors.fill: parent
                    anchors.margins: Theme.gapLine
                    visible: status === Image.Ready
                    asynchronous: true
                    fillMode: Image.PreserveAspectCrop
                    sourceSize.width: 192
                    source: row.comfortable && row.picture !== "" ? Theme.fileUrl(row.picture) : ""
                }
            }
            Icon {
                visible: row.versionIgnored
                name: "eye-off"
                size: Theme.sized(12)
                // An icon is drawn from SVG, which has no eight-digit colours.
                color: row.current ? Theme.onSelected : Theme.ink3
                Tip { text: Theme.say("Made for another game version; the warning is ignored") }
            }
            Text {
                id: nameText
                Layout.fillWidth: true
                Layout.maximumWidth: implicitWidth + 1
                elide: Text.ElideMiddle
                color: row.current ? Theme.onSelected : Theme.ink
                font.pixelSize: Theme.body
                text: row.name
            }
            Chip { visible: row.outdated; onAccent: row.current; tone: Theme.warn; text: row.madeFor; tip: Theme.say("Made for another game version") }
            Chip { visible: row.fresh; onAccent: row.current; tone: Theme.ok; text: Theme.say("New"); tip: Theme.say("Installed since the game was last started from RimStack") }
            Row {
                id: tagRow
                visible: nameText.implicitWidth + implicitWidth + 40 < nameCell.width
                spacing: Theme.gapSmall
                Repeater {
                    model: row.tags === "" ? [] : row.tags.split(", ").slice(0, 2)
                    Chip {
                        required property string modelData
                        maxWidth: Theme.sized(110)
                        onAccent: row.current
                        text: modelData
                    }
                }
            }
            Chip {
                visible: !tagRow.visible && row.tags !== "" && nameCell.width > nameText.implicitWidth + 44
                onAccent: row.current
                readonly property var all: row.tags.split(", ")
                readonly property bool headed: row.columns && Mods.columns.shownBy === "" && Mods.prefs.headings !== 0
                icon: "tag"
                maxWidth: Theme.sized(120)
                text: headed ? all.length : all[0] + (all.length > 1 ? "  +" + (all.length - 1) : "")
                tip: row.tags
            }
            Text {
                visible: row.twin && !row.columns
                color: row.quiet
                font.pixelSize: Theme.small
                text: [row.version, row.source === 2 ? Theme.say("Git") : row.source === 1 ? Theme.say("local") : Theme.say("Workshop")].filter(part => part !== "").join(", ")
            }
            Item { Layout.fillWidth: true }
            Icon {
                visible: !row.columns && row.source === 2
                name: "git-branch"
                size: Theme.sized(12)
                color: row.current ? Theme.onSelected : Theme.ink3
                Tip { text: Theme.say("A Git checkout") }
            }
        }
        Repeater {
            model: row.columns && !row.isHeader ? row.listView.fitting : []
            Item {
                id: cell
                required property string modelData
                Layout.preferredWidth: row.listView.columnWidth(modelData)
                Layout.fillHeight: true
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width
                    elide: Text.ElideRight
                    horizontalAlignment: cell.modelData === "load" ? Text.AlignRight : Text.AlignLeft
                    color: cell.modelData !== "load" ? row.quiet : row.current ? Theme.onSelected : row.loadSeconds >= 0.1 ? Theme.ink2 : Theme.ink3
                    font.pixelSize: cell.modelData === "load" ? Theme.small : Theme.body
                    font.features: { "tnum": 1 }
                    text: cell.modelData === "author" ? row.author : cell.modelData === "version" ? row.version : row.loadTime
                }
                Rectangle {
                    visible: cell.modelData === "load" && row.loadPart > 0 && row.loadTime !== ""
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: Theme.gapLine
                    width: Math.max(2, row.loadPart * parent.width)
                    height: 2
                    radius: 1
                    color: row.current ? Theme.onSelectedDim : Theme.ink3
                }
                Tip { text: cell.modelData === "load" && row.loadTime !== "" ? Theme.fill("{} at the game's last start", row.loadTime) : "" }
            }
        }
        Item {
            visible: row.columns
            Layout.preferredWidth: row.listView.wide ? Theme.sized(124) : Theme.sized(44)
            Layout.fillHeight: true
            Chip {
                id: saying
                readonly property int kind: row.movedFrom > 0 ? 1 : cells.mark ? 2 : row.trouble > 0 ? 3 : 0
                readonly property bool bare: !row.listView.wide
                visible: kind !== 0
                anchors.right: parent.right
                anchors.rightMargin: quick.chosen ? Theme.sized(26) : 0
                anchors.verticalCenter: parent.verticalCenter
                maxWidth: parent.width - anchors.rightMargin
                onAccent: row.current
                tone: kind === 1 ? Theme.moved : kind === 3 && row.trouble === 2 ? Theme.err : Theme.warn
                icon: kind === 1 ? (row.movedFrom > row.number ? "arrow-up" : "arrow-down") : kind === 2 ? "image" : row.trouble === 2 ? "circle-alert" : "triangle-alert"
                text: bare && kind !== 1 ? "" : kind === 1 ? Theme.count(row.movedBy) : kind !== 2 ? row.troubleText : cells.mark.loses ? Theme.say("loses textures") : Theme.say("wins textures")
                tip: kind === 1 ? Theme.fill(row.whyMoved.length > 0 ? "Sort moves it from {} to {}: {}" : "Sort moves it from {} to {}", row.movedFrom, row.number, row.whyMoved)
                    : kind === 2 ? (cells.mark.loses ? Theme.say("The selected mod loads later and its textures are used") : Theme.say("This mod loads later than the selected one and its textures are used"))
                    : row.troubleText
            }
        }
    }

    Row {
        id: quick
        readonly property bool hovered: toggleHover.hovered || folderHover.hovered || moreHover.hovered
        readonly property bool chosen: row.current && Mods.selectedCount === 1
        visible: (hover.hovered || chosen) && !row.isHeader && row.modIndex >= 0 && Mods.drag.held === null && !Hub.mods.previewing && Mods.picking === ""
        anchors.right: parent.right
        anchors.rightMargin: Theme.gapCard
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.gapHair
        Rectangle {
            visible: hover.hovered
            width: Theme.sized(24)
            height: Theme.sized(24)
            radius: Theme.corner
            color: toggleHover.hovered ? Theme.ctlHi : Theme.ctl
            Accessible.role: Accessible.Button
            Accessible.ignored: !quick.visible
            Accessible.name: row.columns ? Theme.say("Deactivate") : Theme.say("Activate")
            Icon { anchors.centerIn: parent; name: row.columns ? "minus" : "plus"; size: Theme.sized(13); color: Theme.ink }
            HoverHandler { id: toggleHover; cursorShape: Qt.PointingHandCursor }
            Accessible.onPressAction: Mods.moveAcross(row.ownModel, row.modIndex, row.modId)
            TapHandler { onTapped: Mods.moveAcross(row.ownModel, row.modIndex, row.modId) }
            Tip { text: row.columns ? Theme.say("Deactivate") : Theme.say("Activate"); over: toggleHover.hovered }
        }
        Rectangle {
            visible: hover.hovered && row.modId >= 0
            width: Theme.sized(24)
            height: Theme.sized(24)
            radius: Theme.corner
            color: folderHover.hovered ? Theme.ctlHi : Theme.ctl
            Accessible.role: Accessible.Button
            Accessible.ignored: !quick.visible || !visible
            Accessible.name: Theme.say("Open folder")
            Icon { anchors.centerIn: parent; name: "folder-open"; size: Theme.sized(13); color: Theme.ink }
            HoverHandler { id: folderHover; cursorShape: Qt.PointingHandCursor }
            Accessible.onPressAction: Mods.openFolder(row.modId)
            TapHandler { onTapped: Mods.openFolder(row.modId) }
            Tip { text: Theme.say("Open folder"); over: folderHover.hovered }
        }
        Rectangle {
            visible: quick.chosen || hover.hovered
            width: Theme.sized(24)
            height: Theme.sized(24)
            radius: Theme.corner
            color: moreHover.hovered ? Theme.ctlHi : Theme.ctl
            Accessible.role: Accessible.Button
            Accessible.ignored: !quick.visible
            Accessible.name: Theme.say("Menu")
            Icon { anchors.centerIn: parent; name: "ellipsis"; size: Theme.sized(13); color: Theme.ink }
            HoverHandler { id: moreHover; cursorShape: Qt.PointingHandCursor }
            Accessible.onPressAction: Mods.rowMenu(row, row.width - 30, row.height)
            TapHandler { onTapped: Mods.rowMenu(row, row.width - 30, row.height) }
            Tip { text: Theme.say("Menu (right-click)"); over: moreHover.hovered }
        }
    }

    TapHandler {
        onTapped: {
            if (quick.hovered || subOff.hovered)
                return
            if (row.isSub)
                Hub.mods.toggleSub(row.ownModel === Mods.active, row.index)
            else if (row.isHeader)
                Hub.mods.toggleCategory(row.ownModel === Mods.active, row.category)
            else if (Mods.picking !== "" && row.columns && row.modId >= 0 && row.modId !== Mods.selectedId)
                Mods.pick(row.modId)
            else if (point.modifiers & Qt.ControlModifier)
                Mods.selectAlso(row.modId, row.ownModel === Mods.active)
            else if (point.modifiers & Qt.ShiftModifier)
                Mods.selectThrough(row.modId, row.ownModel === Mods.active)
            else
                Mods.select(row.modId, row.ownModel === Mods.active)
            row.listView.forceActiveFocus()
        }
        onDoubleTapped: if (!row.isHeader && !quick.hovered && !Hub.mods.previewing) Mods.moveAcross(row.ownModel, row.modIndex, row.modId)
    }

    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: (point) => {
            if (row.isSub)
                Mods.menuRequested(row, point.position.x, point.position.y, [
                    { text: row.collapsed ? Theme.say("Show its mods") : Theme.say("Fold its mods away"), run: () => Hub.mods.toggleSub(row.ownModel === Mods.active, row.index) },
                    { text: Theme.say("Take this tag off these mods"), icon: "x", run: () => Mods.removeSub(row.ownModel, row.index) },
                ])
            else if (row.isHeader)
                Mods.categoryMenu(row, point.position.x, point.position.y, row.category)
            else if (row.modIndex >= 0) {
                // Shift or Ctrl extends the selection before its menu opens, as a left click does.
                if (!row.current && (point.modifiers & Qt.ShiftModifier))
                    Mods.selectThrough(row.modId, row.ownModel === Mods.active)
                else if (!row.current && (point.modifiers & Qt.ControlModifier))
                    Mods.selectAlso(row.modId, row.ownModel === Mods.active)
                Mods.rowMenu(row, point.position.x, point.position.y)
            }
        }
    }

    DragHandler {
        id: dragger
        enabled: !row.isHeader && !Hub.mods.previewing && (!row.columns || Mods.columns.shownBy === "")
        target: null
        onActiveChanged: {
            if (dragger.active)
                Mods.drag.held = { model: row.ownModel, modIndex: row.modIndex, modId: row.modId, name: row.name }
            else
                Mods.drag.finish()
        }
        onCentroidChanged: {
            if (dragger.active) {
                Mods.drag.point = centroid.scenePosition
            }
        }
    }

    DropArea {
        anchors.fill: parent
        enabled: !row.isHeader
        onEntered: Mods.drag.target = { model: row.ownModel, modIndex: row.modIndex, item: row }
        onExited: if (Mods.drag.target && Mods.drag.target.item === row) Mods.drag.target = null
        Rectangle {
            visible: parent.containsDrag
            x: 6
            y: -1.5
            width: parent.width - 12
            height: 3
            radius: Math.min(width, height) / 2
            color: Theme.focus
            Rectangle { x: -3; y: -3; width: 9; height: 9; radius: Math.min(width, height) / 2; color: Theme.focus }
        }
    }
}
