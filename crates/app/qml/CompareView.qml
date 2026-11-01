pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

Rectangle {
    id: view

    property int earlier: -1
    property int later: -1
    property int earlierPlace: 0
    property int laterPlace: 0
    signal done()
    signal changed()

    readonly property var report: Hub.textures.comparison || ({})
    readonly property var textures: Hub.textures.shared || []
    property int chosen: 0
    readonly property var texture: textures[chosen] || null
    property string problem

    property int mode: 0
    property size earlierSize
    property size laterSize
    readonly property bool sameSize: earlierSize.width === laterSize.width && earlierSize.height === laterSize.height
    onSameSizeChanged: if (!sameSize && mode === 2) mode = 0
    // 0 fits the picture to its box; otherwise how many times larger.
    property real zoom: 0
    readonly property alias zoomChoice: zoomChoices
    // Scroll position of both pictures when zoomed, as a share of the way: 0 to 1.
    property real panX: 0.5
    property real panY: 0.5

    function reload() {
        Hub.textures.compare(earlier, later)
        chosen = Math.min(chosen, Math.max(0, textures.length - 1))
    }
    Component.onDestruction: Hub.textures.endCompare()

    function keep(earlierOne) {
        if (!texture)
            return
        problem = Hub.textures.keep(texture.at, earlierOne)
        changed()
    }

    // `{ said }` or `{ problem }`.
    function keepAll(earlierOnes) {
        const done = Hub.textures.keepAll(earlierOnes) || ({})
        problem = done.problem || ""
        changed()
        return done
    }

    function kilobytes(n) {
        return n >= 1024 * 1024 ? Theme.fill("{} MB", (n / 1048576).toFixed(1)) : Theme.fill("{} KB", Math.max(1, Math.round(n / 1024)))
    }

    function ordinal(n) {
        if (n === 1)
            return Theme.say("1st")
        const last = n % 10, lastTwo = n % 100
        return last === 1 && lastTwo !== 11 ? Theme.fill("{}st", n) : last === 2 && lastTwo !== 12 ? Theme.fill("{}nd", n) : last === 3 && lastTwo !== 13 ? Theme.fill("{}rd", n) : Theme.fill("{}th", n)
    }

    color: Theme.win
    // Shortcuts, not key handlers: the keys must work wherever the
    // keyboard was when the comparison opened.
    Shortcut { sequences: ["Up", "Left"]; onActivated: view.chosen = Math.max(0, view.chosen - 1) }
    Shortcut { sequences: ["Down", "Right"]; onActivated: view.chosen = Math.min(view.textures.length - 1, view.chosen + 1) }
    Shortcut { sequence: "Escape"; onActivated: view.done() }
    Shortcut { sequence: "1"; onActivated: view.keep(true) }
    Shortcut { sequence: "2"; onActivated: view.keep(false) }

    component Picture: Rectangle {
        id: box
        property string file
        property bool kept
        property string named
        readonly property alias loaded: image.status
        readonly property size pixels: image.sourceSize
        radius: Theme.cornerLarge
        color: Theme.well
        border.width: kept ? 2 : 1
        border.color: kept ? Theme.focus : Theme.line
        clip: true
        Flickable {
            id: scroller
            anchors.fill: parent
            anchors.margins: Theme.gapHair
            interactive: view.zoom > 0
            boundsBehavior: Flickable.StopAtBounds
            contentWidth: Math.max(width, image.width)
            contentHeight: Math.max(height, image.height)
            // Set by hand, not bound: a binding would end at the first drag.
            function place() {
                if (moving)
                    return
                contentX = (contentWidth - width) * view.panX
                contentY = (contentHeight - height) * view.panY
            }
            onContentWidthChanged: place()
            onContentHeightChanged: place()
            onWidthChanged: place()
            onHeightChanged: place()
            onContentXChanged: if (moving && contentWidth > width) view.panX = contentX / (contentWidth - width)
            onContentYChanged: if (moving && contentHeight > height) view.panY = contentY / (contentHeight - height)
            Connections {
                target: view
                function onPanXChanged() { scroller.place() }
                function onPanYChanged() { scroller.place() }
            }
            Image {
                x: image.x
                y: image.y
                width: image.width
                height: image.height
                visible: image.status === Image.Ready
                fillMode: Image.Tile
                source: "data:image/svg+xml;utf8," + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect width="16" height="16" fill="' + Theme.ctl + '"/><rect width="8" height="8" fill="' + Theme.seg + '"/><rect x="8" y="8" width="8" height="8" fill="' + Theme.seg + '"/></svg>')
            }
            Image {
                id: image
                x: Math.max(0, (scroller.width - width) / 2)
                y: Math.max(0, (scroller.height - height) / 2)
                readonly property real fitted: Math.min(scroller.width - 20, (scroller.height - 20) * (sourceSize.width / Math.max(1, sourceSize.height)))
                width: fitted * Math.max(1, view.zoom)
                height: width * (sourceSize.height / Math.max(1, sourceSize.width))
                smooth: false
                Accessible.role: Accessible.Graphic
                Accessible.name: box.named
                asynchronous: true
                cache: false
                source: box.file ? Theme.fileUrl(box.file) : ""
            }
        }
        Text { visible: image.status === Image.Error; anchors.centerIn: parent; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say("This picture cannot be shown.") }
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.preferredWidth: Theme.sized(290)
            Layout.fillHeight: true
            color: Theme.panel
            ColumnLayout {
                anchors.fill: parent
                spacing: 0
                RowLayout {
                    Layout.leftMargin: Theme.gapLarge
                    Layout.rightMargin: Theme.gapCard
                    Layout.preferredHeight: Theme.header
                    spacing: Theme.gap
                    Btn { text: Theme.say("‹ Load order"); onClicked: view.done() }
                    Item { Layout.fillWidth: true }
                    Text { color: Theme.ink2; font.pixelSize: Theme.small; text: Theme.fill("{} shared", view.report.total || 0) }
                }
                FilterField {
                    Layout.fillWidth: true
                    Layout.leftMargin: Theme.gapLarge
                    Layout.rightMargin: Theme.gapLarge
                    Layout.bottomMargin: Theme.gap
                    placeholder: Theme.say("Filter textures")
                    onTextChanged: { Hub.textures.sharedFilter = text; view.chosen = 0 }
                }
                ListView {
                    id: list
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds
                    Wheel {}
                    ScrollBar.vertical: Scroller {}
                    model: view.textures
                    currentIndex: view.chosen
                    delegate: Item {
                        id: row
                        required property int index
                        required property var modelData
                        readonly property bool current: index === view.chosen
                        width: list.width
                        height: Math.max(Theme.sized(40), names.implicitHeight + 2 * Theme.gapTiny)
                        Rectangle { anchors.fill: parent; anchors.leftMargin: Theme.gapSmall; anchors.rightMargin: Theme.gapSmall; radius: Theme.corner; color: row.current ? Theme.accent : Theme.hover; opacity: row.current ? 1 : rowHover.hovered ? 0.04 : 0 }
                        HoverHandler { id: rowHover }
                        TapHandler { onTapped: { view.chosen = row.index; view.forceActiveFocus() } }
                        Column {
                            id: names
                            x: Theme.gapWide
                            width: parent.width - 2 * Theme.gapWide - Theme.gapHair
                            anchors.verticalCenter: parent.verticalCenter
                            Text { width: parent.width; elide: Text.ElideLeft; color: row.current ? Theme.onSelected : Theme.ink; font.pixelSize: Theme.body; text: row.modelData.file }
                            Text {
                                width: parent.width
                                elide: Text.ElideRight
                                color: row.current ? Theme.onSelectedDim : Theme.ink3
                                font.pixelSize: Theme.small
                                text: row.modelData.used
                            }
                        }
                    }
                }
            }
            Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: Theme.edge }
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            RowLayout {
                Layout.leftMargin: Theme.gapCard
                Layout.rightMargin: Theme.gapLarge
                Layout.preferredHeight: Theme.header
                spacing: Theme.gap
                Text {
                    Layout.fillWidth: true
                    elide: Text.ElideLeft
                    color: Theme.ink
                    font.pixelSize: Theme.title
                    font.weight: Font.DemiBold
                    text: view.texture ? view.texture.name : Theme.say("These two mods share no textures now.")
                }
                Choice { title: Theme.say("View"); options: view.sameSize ? [Theme.say("Side by side"), Theme.say("Slider"), Theme.say("Difference")] : [Theme.say("Side by side"), Theme.say("Slider")]; current: view.mode; onPicked: index => view.mode = index }
                Choice {
                    id: zoomChoices
                    title: Theme.say("Zoom")
                    options: ["Fit", "2×", "4×", "8×"]
                    current: [0, 2, 4, 8].indexOf(view.zoom)
                    onPicked: index => { view.zoom = [0, 2, 4, 8][index]; view.panX = 0.5; view.panY = 0.5 }
                }
            }
            Text { visible: view.problem !== ""; Layout.fillWidth: true; Layout.leftMargin: Theme.gapWide; Layout.rightMargin: Theme.gapWide; wrapMode: Text.WordWrap; color: Theme.err; font.pixelSize: Theme.body; text: Theme.fill("Failed: {}", view.problem) }

            RowLayout {
                visible: view.texture !== null && view.mode === 0
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.margins: Theme.gapWide
                Layout.topMargin: Theme.gapTiny
                spacing: Theme.gapWide
                Repeater {
                    model: [true, false]
                    ColumnLayout {
                        id: half
                        required property bool modelData
                        readonly property bool kept: !!view.texture && view.texture.keepsEarlier === modelData
                        readonly property var mod: (modelData ? view.report.earlier : view.report.later) || ({})
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        Layout.preferredWidth: 1
                        spacing: Theme.gap
                        Picture {
                            id: shown
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            kept: half.kept
                            onPixelsChanged: if (half.modelData) view.earlierSize = pixels; else view.laterSize = pixels
                            named: (view.texture ? view.texture.name + ", " : "") + (half.mod.name || "") + ", " + pixels.width + " × " + pixels.height + (half.kept ? ", " + Theme.say("kept") : "")
                            file: view.texture ? (half.modelData ? view.texture.earlierShown : view.texture.laterShown) : ""
                        }
                        Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; font.weight: Font.DemiBold; text: half.mod.name || "" }
                        Text {
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            color: Theme.ink2
                            font.pixelSize: Theme.body
                            text: [
                                shown.loaded === Image.Ready ? shown.pixels.width + " × " + shown.pixels.height : "",
                                view.texture ? view.kilobytes(half.modelData ? view.texture.earlierBytes : view.texture.laterBytes) : "",
                                Theme.fill("loads {}", view.ordinal(half.modelData ? view.earlierPlace : view.laterPlace)),
                            ].filter(s => s).join(", ")
                        }
                        RowLayout {
                            Layout.preferredHeight: Theme.sized(26)
                            Text { visible: half.kept; color: Theme.ok; font.pixelSize: Theme.body; font.weight: Font.Medium; text: Theme.say("✓ Kept") }
                            Btn { visible: !half.kept; text: Theme.say("Keep this one"); onClicked: view.keep(half.modelData) }
                        }
                    }
                }
            }

            ColumnLayout {
                id: differing
                visible: view.texture !== null && view.mode === 2
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.margins: Theme.gapWide
                Layout.topMargin: Theme.gapTiny
                spacing: Theme.gap
                readonly property var found: visible && view.texture ? Hub.textures.difference(view.texture.at) || ({}) : ({})
                Picture { Layout.fillWidth: true; Layout.fillHeight: true; named: Theme.say("Where the two pictures differ"); file: differing.found.file || "" }
                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: differing.found.problem ? Theme.err : Theme.ink2
                    font.pixelSize: Theme.body
                    text: differing.found.problem ? differing.found.problem
                        : differing.found.total ? (differing.found.differing === 0 ? Theme.say("The two pictures are the same")
                            : Theme.fill("{} of {} pixels differ ({}%), shown as in {}", Theme.count(differing.found.differing), Theme.count(differing.found.total), (100 * differing.found.differing / differing.found.total).toFixed(1), (view.report.later ? view.report.later.name : "")))
                        : ""
                }
            }

            Item {
                id: slider
                visible: view.texture !== null && view.mode === 1
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.margins: Theme.gapWide
                Layout.topMargin: Theme.gapTiny
                property real cut: 0.5
                Picture { id: under; anchors.fill: parent; anchors.bottomMargin: captions.height; named: Theme.say("The earlier mod's picture"); file: view.texture ? view.texture.earlierShown : "" }
                Item {
                    x: under.width * slider.cut
                    width: under.width - x
                    height: under.height
                    clip: true
                    Picture { x: -parent.x; width: under.width; height: under.height; named: Theme.say("The later mod's picture"); file: view.texture ? view.texture.laterShown : "" }
                }
                Item {
                    x: under.width * slider.cut - width / 2
                    width: Theme.sized(28)
                    height: under.height
                    Rectangle { anchors.horizontalCenter: parent.horizontalCenter; width: 2; height: parent.height; color: Theme.focus }
                    Rectangle { anchors.centerIn: parent; width: Theme.gapCard; height: Theme.sized(34); radius: Theme.corner; color: Theme.focus }
                    DragHandler {
                        target: null
                        xAxis.enabled: true
                        yAxis.enabled: false
                        onCentroidChanged: if (active) slider.cut = Math.max(0, Math.min(1, slider.mapFromItem(null, centroid.scenePosition).x / under.width))
                    }
                    HoverHandler { cursorShape: Qt.SplitHCursor }
                }
                RowLayout {
                    id: captions
                    anchors.bottom: parent.bottom
                    width: parent.width
                    height: Math.max(implicitHeight, Theme.sized(28))
                    spacing: Theme.gap
                    Text { color: Theme.ink; font.pixelSize: Theme.body; text: "◀ " + ((view.report.earlier || {}).name || "") }
                    Text { visible: !!view.texture && view.texture.keepsEarlier; color: Theme.ok; font.pixelSize: Theme.body; text: Theme.say("✓ Kept") }
                    Btn { visible: !!view.texture && !view.texture.keepsEarlier; text: Theme.say("Keep this one"); onClicked: view.keep(true) }
                    Item { Layout.fillWidth: true }
                    Text { visible: !!view.texture && !view.texture.keepsEarlier; color: Theme.ok; font.pixelSize: Theme.body; text: Theme.say("✓ Kept") }
                    Btn { visible: !!view.texture && view.texture.keepsEarlier; text: Theme.say("Keep this one"); onClicked: view.keep(false) }
                    Text { color: Theme.ink; font.pixelSize: Theme.body; text: ((view.report.later || {}).name || "") + " ▶" }
                }
            }

            Item { visible: view.texture === null; Layout.fillHeight: true; Layout.fillWidth: true }

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: Theme.sized(28)
                color: Theme.panel
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Theme.gapCard
                    anchors.rightMargin: Theme.gapCard
                    spacing: Theme.gapWide
                    Text { color: Theme.ink2; font.pixelSize: Theme.small; text: Theme.say("← → next texture") }
                    Text { color: Theme.ink2; font.pixelSize: Theme.small; text: Theme.say("1 2 keep left or right") }
                    Item { Layout.fillWidth: true }
                    Text { color: Theme.ink2; font.pixelSize: Theme.small; text: view.zoom > 0 ? Theme.say("Dragging moves both sides together") : Theme.say("Zoom in to look closer") }
                }
            }
        }
    }
}
