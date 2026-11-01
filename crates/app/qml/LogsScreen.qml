pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

Item {
    id: screen
    Appear { of: screen }

    readonly property LogsData reader: Hub.logs
    signal showMod(int modId)
    signal useList(var ids)
    signal findBroken()

    readonly property var facts: reader.head || ({})
    // The game keeps two logs: this launch's and the one before.
    readonly property bool previous: reader.previous
    // A recorded launch on show in place of a log: its place in `launches`, or -1.
    property int recorded: {
        return Hub.number("--recorded", -1)
    }
    readonly property bool filtered: filter.text.trim() !== ""
    readonly property var launches: reader.launches || []
    readonly property bool outside: onLast && !!facts.outside
    readonly property var errors: reader.errors || []
    property int selected: 0
    readonly property var error: errors[selected] || null
    property int view: {
        return Hub.number("--view", 0)
    }
    readonly property var culprits: reader.culprits || []
    readonly property var lines: reader.lines || []

    function showLine(number) {
        filter.text = ""
        view = 2
        const at = Math.max(0, Math.min(number - 1, lines.length - 1))
        logView.positionViewAtIndex(at, ListView.Center)
        logView.marked = number
    }

    Binding { target: Hub.logs; property: "wanted"; value: screen.visible }
    readonly property string kept: reader.kept
    readonly property bool onLast: !previous && kept === ""
    readonly property bool onBefore: previous && kept === ""
    function show(before, keptLog) {
        recorded = -1
        selected = 0
        Hub.logs.previous = before
        Hub.logs.kept = keptLog
    }

    function when(seconds) {
        return Theme.moment(seconds)
    }

    component Entry: Item {
        id: entry
        property string title
        property string outcome
        property string outcomeTip
        property bool bad: false
        property bool clean: false
        property bool current: false
        property bool choosable: true
        signal chosen()
        readonly property bool stacked: entryChip.visible && entryTitle.implicitWidth + entryGrid.columnSpacing + entryChip.implicitWidth > entryGrid.width
        width: parent ? parent.width : 0
        implicitHeight: Math.max(Theme.sized(34), entryGrid.implicitHeight + 2 * Theme.gapSmall)
        height: implicitHeight
        Accessible.ignored: !visible
        Accessible.role: Accessible.ListItem
        Accessible.name: title + (outcome !== "" ? ", " + outcome : "")
        Accessible.onPressAction: if (choosable) entry.chosen()
        Rectangle {
            anchors.fill: parent
            anchors.leftMargin: Theme.gapSmall
            anchors.rightMargin: Theme.gapSmall
            radius: Theme.corner
            color: entry.current ? Theme.accent : Theme.hover
            opacity: entry.current ? 1 : entryHover.hovered && entry.choosable ? 0.04 : 0
        }
        HoverHandler { id: entryHover }
        TapHandler { enabled: entry.choosable; onTapped: entry.chosen() }
        Rectangle { x: entryDot.x + Theme.gapTiny; width: 1; height: parent.height; color: entry.current ? Theme.onSelectedDim : Theme.line }
        Rectangle {
            id: entryDot
            x: Theme.gapWide + Theme.gapLine
            anchors.verticalCenter: parent.verticalCenter
            width: 9
            height: 9
            radius: Math.min(width, height) / 2
            color: entry.current ? Theme.onSelected : entry.bad ? Theme.err : entry.clean ? Theme.ok : Theme.ink3
            border.width: 2
            border.color: entry.current ? Theme.accent : Theme.panel
        }
        GridLayout {
            id: entryGrid
            anchors.fill: parent
            anchors.leftMargin: entryDot.x + entryDot.width + Theme.gapMedium
            anchors.rightMargin: Theme.gapCard
            columns: entry.stacked ? 1 : 2
            columnSpacing: Theme.gap
            rowSpacing: Theme.gapTiny
            Text { id: entryTitle; Layout.fillWidth: true; Layout.alignment: entry.stacked ? Qt.AlignBottom : Qt.AlignVCenter; wrapMode: Text.WordWrap; maximumLineCount: 2; elide: Text.ElideRight; color: entry.current ? Theme.onSelected : entry.choosable ? Theme.ink : Theme.ink2; font.pixelSize: Theme.body; text: entry.title }
            Chip { id: entryChip; Layout.alignment: entry.stacked ? Qt.AlignTop : Qt.AlignVCenter; visible: entry.outcome !== ""; maxWidth: entryGrid.width; onAccent: entry.current; tone: entry.bad ? Theme.err : entry.clean ? Theme.ok : Theme.ink3; text: entry.outcome; tip: entry.outcomeTip }
        }
    }

    PopMenu { id: lineMenu }

    component WrapSwitch: Item {
        id: wrap
        property string pref
        property string tip
        implicitWidth: wrapRow.implicitWidth
        implicitHeight: wrapRow.implicitHeight
        Row {
            id: wrapRow
            spacing: Theme.gap
            Toggle { id: wrapToggle; silent: true; title: Theme.say("Wrap long lines"); on: Theme.stored[wrap.pref]; onFlipped: Theme.stored[wrap.pref] = !Theme.stored[wrap.pref] }
            Text { anchors.verticalCenter: parent.verticalCenter; color: Theme.ink2; font.pixelSize: Theme.small; text: wrapToggle.title }
        }
        Tip { text: wrap.tip }
    }

    LaunchRecord {
        visible: screen.recorded >= 0
        z: 5
        x: launchPane.width
        width: parent.width - x
        height: parent.height
        launch: screen.launches[screen.recorded] || null
        outcome: launch ? launch.outcome : ""
        onUseList: ids => screen.useList(ids)
        onClosed: screen.recorded = -1
    }
    RowLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            id: launchPane
            Layout.preferredWidth: Math.min(Theme.sized(250), screen.width * 0.27)
            Layout.fillHeight: true
            color: Theme.panel
            RowKeys {
                readonly property int fixed: beforeEntry.visible ? 2 : 1
                title: Theme.say("Launches")
                count: fixed + screen.launches.length
                current: screen.recorded >= 0 ? fixed + screen.recorded
                    : screen.kept !== "" ? fixed + Math.max(0, screen.launches.findIndex(launch => launch.log === screen.kept))
                    : screen.onBefore && beforeEntry.visible ? 1 : 0
                onChosen: index => {
                    const launch = screen.launches[index - fixed]
                    if (index >= fixed)
                        recordedList.positionViewAtIndex(index - fixed, ListView.Contain)
                    if (index < fixed)
                        screen.show(index === 1, "")
                    else if (launch.log)
                        screen.show(false, launch.log)
                    else
                        screen.recorded = index - fixed
                }
            }
            ColumnLayout {
                anchors.fill: parent
                spacing: 0
                Text { Layout.leftMargin: Theme.gapCard; Layout.preferredHeight: Theme.header; verticalAlignment: Text.AlignVCenter; color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: Theme.say("Launches") }
                Entry {
                    Layout.fillWidth: true
                    title: screen.outside ? Theme.say("Started outside RimStack") : Theme.say("Last launch")
                    outcome: screen.onLast ? screen.facts.outcome || "" : ""
                    outcomeTip: Theme.say("Different errors in the log; one that repeats counts once") + (screen.outside ? ". " + Theme.say("The game was started without Play: in a round of the search for the broken mod, or from Steam.") : "")
                    bad: screen.onLast && screen.facts.errors > 0
                    clean: screen.onLast && screen.facts.errors === 0
                    current: screen.onLast && screen.recorded < 0
                    onChosen: screen.show(false, "")
                }
                Entry {
                    id: beforeEntry
                    Layout.fillWidth: true
                    visible: screen.kept !== "" || screen.previous || !!screen.facts.hasOther
                    title: Theme.say("Previous launch")
                    outcome: screen.onBefore ? screen.facts.outcome || "" : ""
                    outcomeTip: Theme.say("Different errors in the log; one that repeats counts once")
                    bad: screen.onBefore && screen.facts.errors > 0
                    clean: screen.onBefore && screen.facts.errors === 0
                    current: screen.onBefore && screen.recorded < 0
                    onChosen: screen.show(true, "")
                }
                GroupTitle {
                    Layout.leftMargin: Theme.gapWide
                    visible: screen.launches.length > 0
                    text: Theme.say("Recorded")
                    Tip { text: Theme.say("The game keeps the logs of its last two launches. RimStack records how each launch it starts went.") }
                }
                ListView {
                    id: recordedList
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.topMargin: Theme.gapTiny
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds
                    Wheel {}
                    model: screen.launches
                    delegate: Entry {
                        required property var modelData
                        required property int index
                        current: screen.recorded === index || (screen.kept !== "" && screen.kept === modelData.log && screen.recorded < 0)
                        onChosen: modelData.log ? screen.show(false, modelData.log) : screen.recorded = index
                        title: screen.when(modelData.time)
                        outcome: modelData.outcome
                        bad: modelData.errors > 0
                        clean: modelData.errors === 0
                    }
                }
            }
            Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: Theme.edge }
        }

        ColumnLayout {
            id: middle
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumWidth: 0
            Layout.preferredWidth: 0
            spacing: 0
            clip: true

            GridLayout {
                id: head
                readonly property real room: middle.width - Layout.leftMargin - Layout.rightMargin
                readonly property bool tight: room < openFile.fullWidth + copyPath.fullWidth + findBroken.fullWidth + viewChoice.implicitWidth + 3 * controls.spacing
                readonly property bool wrapped: room < written.implicitWidth + controls.implicitWidth + Theme.gapWide
                columns: wrapped ? 1 : 2
                Layout.fillWidth: true
                Layout.leftMargin: Theme.gapCard
                Layout.rightMargin: Theme.gapLarge
                Layout.topMargin: wrapped ? Theme.gapSmall : 0
                Layout.bottomMargin: wrapped ? Theme.gapSmall : 0
                Layout.minimumHeight: Theme.header
                columnSpacing: Theme.gap
                rowSpacing: Theme.gapSmall
                Column {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    Text { id: written; color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: screen.facts.written ? screen.when(screen.facts.written) : screen.previous ? Theme.say("Previous launch") : Theme.say("Last launch") }
                    Text { width: parent.width; elide: Text.ElideRight; color: Theme.ink3; font.pixelSize: Theme.small; text: screen.facts.about || "" }
                }
                RowLayout {
                id: controls
                spacing: Theme.gap
                Btn { id: openFile; compact: head.tight; icon: "external-link"; text: Theme.say("Open file"); tip: screen.facts.path || ""; enabled: !!screen.facts.path; onClicked: Qt.openUrlExternally(Theme.fileUrl(screen.facts.path)) }
                Btn { id: copyPath; compact: head.tight; icon: "copy"; text: Theme.say("Copy path"); enabled: !!screen.facts.path; onClicked: Theme.copy(screen.facts.path) }
                Btn {
                    id: findBroken
                    visible: !Theme.demo && !Hub.search.active
                    icon: "search"
                    compact: head.tight
                    text: Theme.say("Find the broken mod")
                    tip: Theme.say("Find the broken mod") + ". " + Theme.say("Switches half of the mods off, you play and say whether it still breaks, until one mod is left. Your list is kept and comes back at the end.")
                    onClicked: screen.findBroken()
                }
                Choice { id: viewChoice; title: Theme.say("View"); options: ["Errors", "Mods", "Log"]; current: screen.view; onPicked: (index) => screen.view = index }
                }
            }

            Rectangle {
                id: since
                visible: !!screen.reader.since && screen.onLast && screen.view === 0
                readonly property var info: screen.reader.since || ({ removed: "", added: "", time: 0, ids: [], same: true })
                readonly property bool same: info.same
                Layout.fillWidth: true
                Layout.leftMargin: Theme.gapLarge
                Layout.rightMargin: Theme.gapLarge
                Layout.bottomMargin: Theme.gap
                implicitHeight: sinceText.implicitHeight + 2 * Theme.gapMedium
                radius: Theme.cornerLarge
                color: Theme.panel
                RowLayout {
                    anchors.fill: parent
                    anchors.margins: Theme.gapMedium
                    anchors.leftMargin: Theme.gapLarge
                    spacing: Theme.gap
                    ColumnLayout {
                        id: sinceText
                        Layout.fillWidth: true
                        spacing: 2
                        Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.body; font.weight: Font.DemiBold; text: Theme.say("Since the last clean launch") }
                        Text { color: Theme.ink2; font.pixelSize: Theme.small; text: screen.when(since.info.time) }
                        Text { visible: since.same; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say("Same load order") }
                        Text { visible: !since.same && since.info.added !== ""; Layout.fillWidth: true; wrapMode: Text.WordWrap; textFormat: Text.PlainText; color: Theme.ink; font.pixelSize: Theme.body; text: Theme.say("Added") + " " + since.info.added }
                        Text { visible: !since.same && since.info.removed !== ""; Layout.fillWidth: true; wrapMode: Text.WordWrap; textFormat: Text.PlainText; color: Theme.ink; font.pixelSize: Theme.body; text: Theme.say("Removed") + " " + since.info.removed }
                        Text { visible: !since.same && since.info.reordered; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say("Order changed") }
                    }
                    Btn { visible: !since.same; text: Theme.say("Use"); tip: Theme.say("Makes the load order of the last clean launch the list on the Mods screen. Save writes it."); onClicked: screen.useList(since.info.ids) }
                }
            }

            RowLayout {
                visible: screen.view !== 1
                Layout.fillWidth: true
                Layout.leftMargin: Theme.gapLarge
                Layout.rightMargin: Theme.gapLarge
                Layout.bottomMargin: Theme.gap
                spacing: Theme.gap
                FilterField {
                    id: filter
                    Layout.fillWidth: true
                    placeholder: screen.view === 2 ? Theme.say("Search the log") : Theme.say("Filter by text or mod")
                    onTextChanged: { Hub.logs.filter = text; screen.selected = 0 }
                }
                WrapSwitch {
                    visible: screen.view === 2
                    tip: Theme.say("Wrap long lines in the log")
                    pref: "wrapLog"
                }
            }

            ListView {
                id: culpritList
                visible: screen.view === 1
                Layout.fillWidth: true
                Layout.fillHeight: true
                topMargin: Theme.gapTiny
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                Wheel {}
                ScrollBar.vertical: Scroller {}
                model: screen.view === 1 ? screen.culprits : []
                property int at: 0
                function open(culprit) {
                    screen.view = 0
                    filter.text = culprit.name || screen.reader.noMod
                }
                RowKeys {
                    id: culpritKeys
                    parent: culpritList
                    view: culpritList
                    title: Theme.say("Mods the errors point at")
                    count: culpritList.count
                    current: culpritList.at
                    onChosen: index => culpritList.at = index
                    onEntered: culpritList.open(screen.culprits[culpritList.at])
                }
                delegate: Item {
                    id: culpritRow
                    required property var modelData
                    required property int index
                    Accessible.role: Accessible.ListItem
                    Accessible.name: Theme.plural(modelData.kinds, "{} error, {}", "{} errors, {}", modelData.name || Theme.say("No mod recognised"))
                    width: culpritList.width
                    height: Theme.sized(34)
                    readonly property bool current: culpritList.at === index
                    Rectangle { anchors.fill: parent; anchors.leftMargin: Theme.gapSmall; anchors.rightMargin: Theme.gapSmall; radius: Theme.corner; color: culpritRow.current ? Theme.accent : Theme.hover; opacity: culpritRow.current ? 1 : culpritHover.hovered ? 0.04 : 0 }
                    HoverHandler { id: culpritHover; cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: culpritList.at = culpritRow.index; onDoubleTapped: culpritList.open(culpritRow.modelData) }
                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: Theme.gapWide
                        anchors.rightMargin: Theme.gapWide
                        spacing: Theme.gapLarge
                        Text { Layout.fillWidth: true; elide: Text.ElideRight; color: culpritRow.current ? Theme.onSelected : culpritRow.modelData.name ? Theme.ink : Theme.ink2; font.pixelSize: Theme.body; text: culpritRow.modelData.name || Theme.say("No mod recognised") }
                        Text { color: culpritRow.current ? Theme.onSelectedDim : Theme.ink2; font.pixelSize: Theme.body; text: Theme.plural(culpritRow.modelData.kinds, "{} error", "{} errors") + ", " + Theme.plural(culpritRow.modelData.times, "logged {} time", "logged {} times") }
                    }
                }
            }

            ListView {
                id: logView
                property int marked: 0
                visible: screen.view === 2
                Layout.fillWidth: true
                Layout.fillHeight: true
                topMargin: Theme.gapTiny
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                Wheel {}
                ScrollBar.vertical: Scroller {}
                model: screen.view === 2 ? screen.lines : []
                RowKeys {
                    parent: logView
                    view: logView
                    title: Theme.say("Log")
                    count: logView.count
                    current: Math.max(0, screen.lines.findIndex(l => l.number === logView.marked))
                    onChosen: index => logView.marked = screen.lines[index].number
                }
                delegate: Item {
                    id: logLine
                    required property var modelData
                    readonly property bool bad: modelData.error
                    Accessible.role: Accessible.ListItem
                    Accessible.name: (bad ? Theme.say("Error") + ": " : modelData.warning ? Theme.say("Warning") + ": " : "") + modelData.text
                    width: logView.width
                    height: Math.max(Theme.sized(20), lineText.implicitHeight + 4)
                    Rectangle { visible: logLine.modelData.number === logView.marked; anchors.fill: parent; color: Theme.accent; opacity: 0.25 }
                    Text { id: lineNumber; x: Theme.gap; width: Theme.sized(52); horizontalAlignment: Text.AlignRight; anchors.verticalCenter: parent.verticalCenter; color: Theme.ink3; font.family: "monospace"; font.pixelSize: Theme.small; text: logLine.modelData.number }
                    Rectangle {
                        id: lineMark
                        visible: logLine.bad || logLine.modelData.warning
                        x: lineNumber.x + lineNumber.width + Theme.gapSmall
                        anchors.verticalCenter: parent.verticalCenter
                        width: Theme.sized(16)
                        height: Theme.sized(16)
                        radius: Theme.corner
                        color: logLine.bad ? Theme.dangerBg : "transparent"
                        Text { anchors.centerIn: parent; color: logLine.bad ? Theme.err : Theme.warn; font.family: "monospace"; font.pixelSize: Theme.small; font.weight: Font.Bold; text: logLine.bad ? Theme.say("E") : Theme.say("W") }
                    }
                    Text {
                        id: lineText
                        x: lineMark.x + lineMark.width + Theme.gap
                        width: parent.width - x - Theme.gapWide
                        anchors.verticalCenter: parent.verticalCenter
                        elide: Theme.stored.wrapLog ? Text.ElideNone : Text.ElideRight
                        wrapMode: Theme.stored.wrapLog ? Text.WrapAnywhere : Text.NoWrap
                        textFormat: Text.PlainText
                        color: logLine.bad ? Theme.err : Theme.ink
                        font.family: "monospace"
                        font.pixelSize: Theme.small
                        text: logLine.modelData.text
                    }
                    TapHandler { onTapped: logView.marked = logLine.modelData.number }
                    TapHandler { acceptedButtons: Qt.RightButton; onTapped: (point) => lineMenu.openAt(logLine, point.position.x, point.position.y, [{ text: Theme.say("Copy line"), icon: "copy", run: () => Theme.copy(logLine.modelData.text) }]) }
                }
                Text {
                    visible: logView.count === 0 && !screen.reader.busy
                    anchors.centerIn: parent
                    color: Theme.ink2
                    font.pixelSize: Theme.body
                    text: screen.filtered ? Theme.say("No line matches") : Theme.say("No log")
                }
            }
            Text {
                visible: screen.view === 2 && (screen.facts.cut || "") !== ""
                Layout.leftMargin: Theme.gapWide
                Layout.bottomMargin: Theme.gapSmall
                color: Theme.ink3
                font.pixelSize: Theme.small
                text: screen.facts.cut || ""
            }

            ListView {
                id: list
                visible: screen.view === 0
                Layout.fillWidth: true
                Layout.fillHeight: true
                topMargin: Theme.gapTiny
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                Wheel {}
                ScrollBar.vertical: Scroller {}
                model: screen.errors
                spacing: Theme.gapHair
                RowKeys {
                    parent: list
                    view: list
                    title: Theme.say("Errors")
                    count: list.count
                    current: screen.selected
                    onChosen: index => screen.selected = index
                }
                delegate: Item {
                    id: row
                    required property int index
                    required property var modelData
                    readonly property bool current: index === screen.selected
                    width: list.width
                    height: Math.max(Theme.sized(40), rowText.implicitHeight + Theme.sized(18))
                    Accessible.role: Accessible.ListItem
                    Accessible.name: modelData.head
                    Rectangle {
                        anchors.fill: parent
                        anchors.leftMargin: Theme.gapLarge
                        anchors.rightMargin: Theme.gapCard
                        radius: Theme.corner
                        color: row.current ? Theme.accent : Theme.hover
                        opacity: row.current ? 1 : rowHover.hovered ? 0.05 : 0
                    }
                    Rectangle { visible: !row.current; anchors.bottom: parent.bottom; anchors.bottomMargin: -1; x: Theme.gapPage; width: parent.width - 2 * Theme.gapPage; height: 1; color: Theme.line; opacity: 0.6 }
                    HoverHandler { id: rowHover }
                    TapHandler { onTapped: screen.selected = row.index }
                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: Theme.gapPage
                        anchors.rightMargin: Theme.gapPage + Theme.gapSmall
                        spacing: Theme.gapLarge
                        Rectangle {
                            visible: row.modelData.count > 1
                            implicitWidth: Math.max(Theme.control, times.implicitWidth + 2 * Theme.gapSmall)
                            implicitHeight: Theme.control
                            radius: Theme.corner
                            color: row.current ? Theme.selectedChip : Qt.rgba(Theme.err.r, Theme.err.g, Theme.err.b, 0.16)
                            Text { id: times; anchors.centerIn: parent; color: row.current ? Theme.onSelected : Theme.err; font.pixelSize: Theme.small; font.weight: Font.DemiBold; font.features: { "tnum": 1 }; text: "×" + Theme.count(row.modelData.count) }
                            Tip { text: Theme.say("How many times the log shows it") }
                        }
                        Column {
                            id: rowText
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            spacing: Theme.gapThin
                            Text { width: parent.width; elide: Text.ElideRight; color: row.current ? Theme.onSelected : Theme.ink; font.pixelSize: Theme.body; font.weight: Font.Medium; text: row.modelData.kind }
                            Text { visible: row.modelData.detail !== ""; width: parent.width; elide: Text.ElideRight; color: row.current ? Theme.onSelectedDim : Theme.ink2; font.pixelSize: Theme.small; text: row.modelData.detail }
                        }
                        Chip {
                            visible: row.modelData.more > 0
                            onAccent: row.current
                            pressable: true
                            icon: row.modelData.opened ? "chevron-down" : "chevron-right"
                            text: Theme.fill("{} more alike", row.modelData.more)
                            tip: Theme.say("The errors after this one that differ from it only in their last word")
                            onClicked: screen.reader.toggleRun(row.modelData.run)
                        }
                        Chip {
                            visible: !!row.modelData.mod
                            maxWidth: Math.min(Theme.sized(300), row.width * 0.36)
                            onAccent: row.current
                            pressable: row.modelData.modId >= 0
                            tone: Theme.ink
                            icon: "package"
                            text: row.modelData.mod || ""
                            tip: row.modelData.modId >= 0 ? Theme.say("The mod the error points at. Click to go to it.") : Theme.say("The mod the error points at")
                            onClicked: screen.showMod(row.modelData.modId)
                        }
                    }
                }

                Column {
                    visible: screen.errors.length === 0
                    anchors.centerIn: parent
                    width: Math.min(parent.width - 4 * Theme.gapPage, Theme.sized(480))
                    spacing: Theme.gapSmall
                    Text {
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        color: screen.reader.problem !== "" ? Theme.err : Theme.ink
                        font.pixelSize: Theme.title
                        font.weight: Font.DemiBold
                        text: screen.reader.busy ? Theme.say("Reading the log…") : screen.reader.problem !== "" ? Theme.say("Log could not be read") : screen.filtered ? Theme.say("No error matches") : screen.facts.lines ? Theme.say("No errors") : Theme.say("No log")
                    }
                    Text {
                        visible: !screen.reader.busy
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.WordWrap
                        color: Theme.ink2
                        font.pixelSize: Theme.body
                        lineHeight: 1.3
                        text: screen.reader.problem || (screen.filtered ? ""
                            : screen.facts.lines ? Theme.plural(screen.facts.lines, "{} line", "{} lines")
                            : Theme.say("Written when the game starts"))
                    }
                }
            }
        }

        Rectangle { Layout.fillHeight: true; implicitWidth: 1; color: Theme.edge }

        Rectangle {
            Layout.preferredWidth: Math.min(Theme.sized(360), screen.width * 0.32)
            Layout.fillHeight: true
            color: Theme.panel

            Scrolled {
                anchors.fill: parent
                padding: Theme.gapCard

                Column {
                    id: modSide
                    readonly property var mod: screen.view === 1 ? screen.culprits[culpritList.at] || null : null
                    readonly property var theirs: mod ? screen.reader.errorsOf(mod.name) || [] : []
                    width: parent.width
                    visible: mod !== null
                    spacing: Theme.gap
                    Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.heading; font.weight: Font.DemiBold; text: modSide.mod ? modSide.mod.name || Theme.say("No mod recognised") : "" }
                    Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.body; text: modSide.mod ? Theme.plural(modSide.mod.kinds, "{} error", "{} errors") + ", " + Theme.plural(modSide.mod.times, "logged {} time", "logged {} times") : "" }
                    Flow {
                        width: parent.width
                        spacing: Theme.gapSmall
                        Btn { text: Theme.say("Show its errors"); tip: Theme.say("Errors, narrowed to this mod"); onClicked: culpritList.open(modSide.mod) }
                        Btn { visible: !!modSide.mod && modSide.mod.modId >= 0; text: Theme.say("Go to mod"); onClicked: screen.showMod(modSide.mod.modId) }
                    }
                    GroupTitle { text: Theme.say("Errors") }
                    Repeater {
                        model: modSide.theirs
                        Column {
                            id: theirError
                            required property var modelData
                            width: modSide.width
                            Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.body; text: theirError.modelData.head }
                            Text { visible: theirError.modelData.count > 1; color: Theme.ink3; font.pixelSize: Theme.small; text: Theme.plural(theirError.modelData.count, "logged {} time", "logged {} times") }
                        }
                    }
                }
                Column {
                    id: side
                    width: parent.width
                    visible: screen.error !== null && screen.view !== 1

                    CopyText {
                        width: parent.width
                        font.pixelSize: Theme.heading
                        font.weight: Font.DemiBold
                        text: screen.error ? screen.error.kind : ""
                    }
                    Text {
                        width: parent.width
                        topPadding: Theme.gapHair
                        wrapMode: Text.WordWrap
                        color: Theme.ink2
                        font.pixelSize: Theme.body
                        text: screen.error ? Theme.plural(screen.error.count, "{} time, first at line {}", "{} times, first at line {}", Theme.count(screen.error.line)) : ""
                    }

                    Item { width: 1; height: Theme.gapMedium }
                    Row {
                        spacing: Theme.gapSmall
                        Btn { text: Theme.say("Show in log"); onClicked: screen.showLine(screen.error.line) }
                        Btn { icon: "copy"; text: Theme.say("Copy report"); tip: Theme.say("The error, its stack trace, the game version and the likely mod, as text."); onClicked: Theme.copy(screen.reader.helpText(screen.selected)) }
                    }

                    GroupTitle { text: Theme.say("Mods") }
                    Text {
                        visible: !!screen.error && screen.error.mods.length === 0
                        color: Theme.ink2
                        font.pixelSize: Theme.body
                        text: Theme.say("None recognised")
                        Tip { text: Theme.say("No active mod's code is in the stack trace, and the error names none.") }
                    }
                    Repeater {
                        model: screen.error ? screen.error.mods : []
                        RowLayout {
                            id: culprit
                            required property var modelData
                            width: side.width
                            height: Theme.sized(31)
                            spacing: Theme.gap
                            Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: culprit.modelData.name }
                            Btn { text: Theme.say("Go to mod"); onClicked: screen.showMod(culprit.modelData.modId) }
                        }
                    }

                    GroupTitle { text: Theme.say("Logged") }
                    CopyText { width: parent.width; text: screen.error ? screen.error.head : "" }

                    Item {
                        visible: !!screen.error && screen.error.trace.length > 0
                        readonly property bool stacked: traceTitle.implicitWidth + traceLines.implicitWidth + Theme.gap > width
                        width: parent.width
                        height: traceTitle.height + (stacked ? traceLines.height + Theme.gapTiny : 0)
                        GroupTitle { id: traceTitle; text: Theme.say("Stack trace") }
                        WrapSwitch {
                            id: traceLines
                            anchors.right: parent.stacked ? undefined : parent.right
                            anchors.bottom: parent.bottom
                            anchors.bottomMargin: Theme.gapTiny
                            tip: Theme.say("Wrap long lines in the stack trace")
                            pref: "wrapTrace"
                        }
                    }
                    Rectangle {
                        visible: !!screen.error && screen.error.trace.length > 0
                        width: parent.width
                        height: trace.implicitHeight + 2 * Theme.gapMedium
                        radius: Theme.cornerLarge
                        color: Theme.well
                        clip: true
                        CopyText {
                            id: trace
                            x: Theme.gapMedium
                            y: Theme.gapMedium
                            width: parent.width - 2 * Theme.gapMedium
                            // A line breaks only after a separator: a zero-width space
                            // follows each.
                            wrapMode: Theme.stored.wrapTrace ? TextEdit.Wrap : TextEdit.NoWrap
                            color: Theme.ink2
                            font.family: "monospace"
                            font.pixelSize: Theme.small
                            text: screen.error ? screen.error.traceText : ""
                        }
                    }
                }
            }
        }
    }
}
