// Changes nothing until asked.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

Rectangle {
    id: page

    readonly property var info: Hub.settings.report
    required property int activeCount
    required property int inactiveCount
    required property string problem
    readonly property var databases: Hub.settings.databases
    readonly property var languages: Hub.settings.languages
    readonly property string language: info.languageInUse || ""
    signal start(bool withData, bool withRimSort)

    readonly property bool withData: databases.some(database => database.on)
    property bool withRimSort: true
    color: Theme.win

    component Block: Column {
        id: block
        property string title
        default property alias lines: inside.data
        width: parent.width
        topPadding: Theme.gapWide
        Text { x: Theme.gapLarge; bottomPadding: Theme.gapSmall; color: Theme.ink2; font.pixelSize: Theme.small; font.weight: Font.DemiBold; text: block.title }
        Rectangle {
            width: parent.width
            height: inside.implicitHeight
            radius: Theme.cornerLarge
            color: Theme.panel
            Column { id: inside; width: parent.width }
        }
    }

    component Found: Item {
        id: found
        property string what
        property string where
                property bool last: false
        // `where` is a sentence, not a folder: one line of plain text.
        property bool sentence: false
        property bool note: false
        width: parent.width
        height: Math.max(Theme.sized(36), said.implicitHeight + 2 * Theme.gapSmall)
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: Theme.gapLarge
            anchors.rightMargin: Theme.gapLarge
            spacing: Theme.gap
            Text { Layout.preferredWidth: Theme.sized(18); color: found.note ? Theme.ink3 : found.where ? Theme.ok : Theme.err; font.pixelSize: Theme.body; font.weight: Font.DemiBold; text: found.note ? "•" : found.where ? "✓" : "!" }
            Text { visible: !found.sentence; Layout.preferredWidth: Theme.sized(220); elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: found.what }
            PathText { visible: !found.sentence; path: found.where || Theme.say("Not found"); tone: found.where ? Theme.ink2 : Theme.err }
            Text { id: said; visible: found.sentence; Layout.fillWidth: true; wrapMode: Text.WordWrap; color: found.note || found.where ? Theme.ink : Theme.err; font.pixelSize: Theme.body; text: found.what + (found.where ? ", " + found.where : "") }
        }
        Rectangle { visible: !found.last; anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.line }
    }

    component Tick: Item {
        id: choice
        property string text
        property string detail
        property bool on: true
        property bool last: false
        signal toggled()
        width: parent.width
        height: Math.max(Theme.sized(36), said.implicitHeight + 2 * Theme.gapSmall)
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: Theme.gapLarge
            anchors.rightMargin: Theme.gapLarge
            spacing: Theme.gap
            Rectangle {
                implicitWidth: Theme.sized(15)
                implicitHeight: implicitWidth
                radius: 3
                color: choice.on ? Theme.accent : Theme.well
                border.width: 1
                border.color: choice.on ? Theme.accent : Theme.line
                activeFocusOnTab: true
                Accessible.role: Accessible.CheckBox
                Accessible.name: choice.text
                Accessible.checkable: true
                Accessible.checked: choice.on
                Accessible.onToggleAction: choice.toggled()
                Keys.onSpacePressed: choice.toggled()
                FocusRing { corner: 3 }
                Text { visible: choice.on; anchors.centerIn: parent; color: Theme.onSelected; font.pixelSize: Theme.caption; font.weight: Font.DemiBold; text: "✓" }
            }
            Text {
                id: said
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                textFormat: Text.StyledText
                color: Theme.ink
                font.pixelSize: Theme.body
                text: choice.text + (choice.detail ? " <font color=\"" + Theme.ink2 + "\">" + choice.detail + "</font>" : "")
            }
        }
        TapHandler { onTapped: choice.toggled() }
        Rectangle { visible: !choice.last; anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.line }
    }

    component Pick: Item {
        id: pick
        property string what
        property bool last: false
        default property alias control: held.data
        width: parent.width
        height: Math.max(Theme.sized(44), held.implicitHeight + 2 * Theme.gap, named.implicitHeight + 2 * Theme.gap)
        TextMetrics { id: measure; font.pixelSize: Theme.body; text: pick.what }
        Text { id: named; x: Theme.gapLarge; width: Math.min(Math.max(Theme.sized(190), measure.advanceWidth), pick.width / 2); anchors.verticalCenter: parent.verticalCenter; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.body; text: pick.what }
        Flow { id: held; anchors.left: named.right; anchors.leftMargin: Theme.gapLarge; anchors.right: parent.right; anchors.rightMargin: Theme.gapLarge; anchors.verticalCenter: parent.verticalCenter; spacing: Theme.gapSmall }
        Rectangle { visible: !pick.last; anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.line }
    }

    property int step: {
        return Hub.number("--step", 0)
    }
    readonly property var steps: [Theme.say("Found"), Theme.say("Community data"), Theme.say("Sorting"), Theme.say("Appearance"), Theme.say("Start")]
    readonly property int last: steps.length - 1

    RowLayout {
        anchors.centerIn: parent
        width: Math.min(Theme.sized(940), parent.width - 4 * Theme.gapPage)
        height: Math.min(Theme.sized(580), parent.height - 2 * Theme.gapPage)
        spacing: Theme.gapWide * 3

        Column {
            Layout.alignment: Qt.AlignTop
            spacing: Theme.gapSmall
            Image { width: Theme.sized(112); height: width; sourceSize.width: 2 * width; sourceSize.height: 2 * height; source: "qrc:/qt/qml/RimStack/Ui/assets/rimstack.svg" }
            Text { topPadding: Theme.gap; color: Theme.brand; font.pixelSize: Theme.display; font.weight: Font.DemiBold; text: Theme.say("RimStack") }
            Text { color: Theme.ink; font.pixelSize: Theme.title; text: Theme.say("A mod manager for RimWorld") }
            Item { width: 1; height: Theme.gapPage }
            Repeater {
                model: page.steps
                Row {
                    id: mark
                    required property string modelData
                    required property int index
                    spacing: Theme.gap
                    Rectangle {
                        width: Theme.sized(22)
                        height: Theme.sized(22)
                        radius: Math.min(width, height) / 2
                        color: mark.index === page.step ? Theme.accent : mark.index < page.step ? Theme.ok : Theme.ctl
                        Text { anchors.centerIn: parent; color: mark.index <= page.step ? Theme.onSelected : Theme.ink2; font.pixelSize: Theme.small; font.weight: Font.DemiBold; text: mark.index < page.step ? "✓" : mark.index + 1 }
                    }
                    Text { anchors.verticalCenter: parent.verticalCenter; color: mark.index === page.step ? Theme.ink : Theme.ink2; font.pixelSize: Theme.body; font.weight: mark.index === page.step ? Font.DemiBold : Font.Normal; text: Theme.say(mark.modelData) }
                    HoverHandler { cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: page.step = mark.index }
                }
            }
        }

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
        Flickable {
            anchors.fill: parent
            anchors.bottomMargin: buttons.height + Theme.gapWide
            contentHeight: content.implicitHeight
            interactive: contentHeight > height
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            Wheel {}
        Column {
            id: content
            width: parent.width

            Text { color: Theme.ink; font.pixelSize: Theme.heading; font.weight: Font.DemiBold; text: Theme.say(page.steps[page.step]) }
            Text {
                topPadding: Theme.gapHair
                color: Theme.ink2
                font.pixelSize: Theme.body
                text: [Theme.say("Found automatically; changeable in Settings"), Theme.say("Fetched from GitHub on Start, and again only on request"), Theme.say("What Sort follows beside each mod's own rules"), Theme.say("Changeable in Settings at any time"), Theme.say("Nothing is written to the game before Save")][page.step]
            }

            Block {
                visible: page.step === 0
                title: Theme.say("Interface")
                Pick {
                    last: true
                    what: Theme.say("Language")
                    Repeater {
                        model: page.languages
                        Btn {
                            required property string modelData
                            primary: page.language === modelData
                            text: modelData
                            onClicked: Hub.settings.useLanguage(modelData)
                        }
                    }
                }
            }
            Block {
                visible: page.step === 0
                title: Theme.say("Game")
                Found { what: "RimWorld" + (page.info.gameVersion ? " " + page.info.gameVersion : ""); where: page.info.game || "" }
                Found { what: Theme.say("Workshop mods"); where: page.info.workshop || "" }
                Found { what: Theme.say("Config and saves"); where: page.info.config || ""; last: true }
            }
            Block {
                visible: page.step === 0
                title: Theme.say("Current mod list")
                Found {
                    last: true
                    sentence: true
                    what: page.problem !== "" ? Theme.fill("Mods could not be read: {}", page.problem) : Theme.fill("{} active mods, {} inactive", Theme.count(page.activeCount), Theme.count(page.inactiveCount))
                    where: page.problem !== "" ? "" : Theme.say("kept as they are")
                }
            }
            Block {
                visible: page.step === 0
                title: Theme.say("Programs")
                Found { visible: !!page.info.git; what: "Git"; where: page.info.git ? Theme.fill("version {}", page.info.git) : ""; sentence: true }
                Found { visible: !page.info.git; note: true; sentence: true; what: Theme.say("Git is not installed: a mod kept as a Git checkout cannot be checked or updated") }
                Found { visible: !!page.info.steamcmd; what: "SteamCMD"; where: Theme.say("installed"); sentence: true; last: true }
                Found { visible: !page.info.steamcmd; note: true; sentence: true; last: true; what: Theme.say("SteamCMD is not installed: Downloads can install it, to fetch a mod without subscribing") }
            }
            Block {
                visible: page.step === 1 && !!page.info.rimsortRules
                title: Theme.say("Import from RimSort")
                Tick { last: true; text: Theme.say("Own load order rules"); detail: Theme.say("(RimSort's file is only read)"); on: page.withRimSort; onToggled: page.withRimSort = !page.withRimSort }
            }
            Block {
                visible: page.step === 1
                title: Theme.say("Community data")
                Repeater {
                    model: page.databases
                    Tick {
                        required property var modelData
                        required property int index
                        last: index === page.databases.length - 1
                        text: modelData.name
                        detail: Theme.say(modelData.use) + (modelData.updated ? ", " + Theme.fill("downloaded {}", Theme.day(modelData.updated)) : "")
                        on: modelData.on
                        onToggled: {
                            Hub.settings.useDatabase(modelData.id, !modelData.on)
                        }
                    }
                }
            }
            Block {
                visible: page.step === 2
                title: Theme.say("Sort follows")
                Found { note: true; sentence: true; what: Theme.say("Each mod's own rules, and the code it uses or patches in another mod: always") }
                Tick {
                    text: Theme.say("Community rules")
                    detail: Theme.say("load-order rules kept by players, for mods whose files do not say enough")
                    on: !!page.info.communityRules
                    onToggled: Hub.settings.keep("communityRules", page.info.communityRules ? "off" : "on")
                }
                Tick {
                    last: true
                    text: Theme.say("Patch and def hints")
                    detail: Theme.say("a patch is placed after what it patches, read from the files")
                    on: !!page.info.fileHints
                    onToggled: Hub.settings.keep("fileHints", page.info.fileHints ? "off" : "on")
                }
            }
            Block {
                visible: page.step === 2
                title: Theme.say("Either way")
                Found { note: true; sentence: true; what: Theme.say("Sort shows every move and the rule behind it before anything changes") }
                Found { note: true; sentence: true; last: true; what: Theme.say("Hints left out of Sort still show in a mod's Load order tab") }
            }
            Block {
                visible: page.step === 3 && page.info.official.length > 0
                title: Theme.say("As it will look")
                Item {
                    width: parent.width
                    height: glance.height + 2 * Theme.gap
                    Column {
                        id: glance
                        x: Theme.gap
                        y: Theme.gap
                        width: parent.width - 2 * Theme.gap
                        Repeater {
                            model: page.info.official
                            Rectangle {
                                id: sample
                                required property var modelData
                                required property int index
                                readonly property bool chosen: index === 1
                                width: glance.width
                                height: Theme.stored.comfortable ? Theme.sized(46) : Theme.row
                                radius: Theme.corner
                                color: chosen ? Theme.accent : "transparent"
                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: Theme.gapMedium
                                    anchors.rightMargin: Theme.gapMedium
                                    spacing: Theme.gap
                                    Text { color: sample.chosen ? Theme.onSelectedDim : Theme.ink3; font.pixelSize: Theme.small; text: sample.index + 1 }
                                    Icon { name: Theme.categoryIcons[1] || ""; size: Theme.sized(13); color: sample.chosen ? Theme.onSelected : Theme.categoryColours[1] || Theme.ink3 }
                                    Rectangle {
                                        visible: Theme.stored.comfortable
                                        implicitWidth: Theme.sized(64)
                                        implicitHeight: Theme.sized(40)
                                        radius: Theme.corner
                                        color: Theme.ctl
                                        clip: true
                                        Image { anchors.fill: parent; anchors.margins: Theme.gapLine; visible: status === Image.Ready; fillMode: Image.PreserveAspectCrop; asynchronous: true; sourceSize.width: 3 * width; source: sample.modelData.picture ? Theme.fileUrl(sample.modelData.picture) : "" }
                                    }
                                    Text { Layout.fillWidth: true; elide: Text.ElideRight; color: sample.chosen ? Theme.onSelected : Theme.ink; font.pixelSize: Theme.body; text: sample.modelData.name }
                                    Text { color: sample.chosen ? Theme.onSelectedDim : Theme.ink2; font.pixelSize: Theme.body; text: sample.modelData.author }
                                    Text { color: sample.chosen ? Theme.onSelectedDim : Theme.ink2; font.pixelSize: Theme.body; text: page.info.gameVersion || "" }
                                }
                            }
                        }
                    }
                }
            }
            Block {
                visible: page.step === 3
                title: Theme.say("Appearance")
                Pick {
                    what: Theme.say("Theme")
                    Choice { title: Theme.say("Theme"); options: Theme.themeLabels; current: Math.max(0, Theme.themeNames.indexOf(Theme.stored.theme)); onPicked: index => Theme.stored.theme = Theme.themeNames[index] }
                }
                Pick {
                    what: Theme.say("Text size")
                    Choice {
                        readonly property var sizes: [1, 1.1, 1.25, 1.5]
                        title: Theme.say("Text size")
                        options: ["100%", "110%", "125%", "150%"]
                        current: Math.max(0, sizes.findIndex(size => Math.abs(size - Theme.stored.textScale) < 0.03))
                        onPicked: index => Theme.stored.textScale = sizes[index]
                    }
                }
                Pick {
                    what: Theme.say("Mod rows")
                    Choice { title: Theme.say("Mod rows"); options: ["Compact", "Larger", "Comfortable"]; current: Theme.stored.comfortable ? 2 : Theme.stored.largeRows ? 1 : 0; onPicked: index => { Theme.stored.comfortable = index === 2; Theme.stored.largeRows = index === 1 } }
                }
                Pick {
                    last: true
                    what: Theme.say("Workshop pages open in")
                    Choice { title: Theme.say("Workshop pages open in"); options: ["Browser", "Steam"]; current: Theme.stored.workshopInSteam ? 1 : 0; onPicked: index => Theme.stored.workshopInSteam = index === 1 }
                }
            }
            Block {
                visible: page.step === page.last
                title: Theme.say("After Start")
                Found { note: true; sentence: true; what: Theme.say("Problems are counted above the load order, each with its fix"); where: "" }
                Found { note: true; sentence: true; what: Theme.say("Sort shows every move, and why, before anything changes"); where: "" }
                Found { note: true; sentence: true; what: Theme.say("Save writes ModsConfig.xml and copies the file as it was first"); where: ""; last: true }
            }
        }
        }
            Row {
                id: buttons
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                spacing: Theme.gap
                Btn { visible: page.step > 0; large: true; text: Theme.say("Back"); onClicked: page.step -= 1 }
                Btn { visible: page.step < page.last; large: true; text: Theme.say("Skip to Start"); onClicked: page.start(page.withData, page.withRimSort && !!page.info.rimsortRules) }
                Btn { id: next; visible: page.step < page.last; primary: true; large: true; text: Theme.say("Next"); onClicked: page.step += 1 }
                Btn { id: begin; visible: page.step === page.last; primary: true; large: true; icon: "check"; text: Theme.say("Start"); onClicked: page.start(page.withData, page.withRimSort && !!page.info.rimsortRules) }
            }
        }
    }
    Component.onCompleted: next.forceActiveFocus()
}
