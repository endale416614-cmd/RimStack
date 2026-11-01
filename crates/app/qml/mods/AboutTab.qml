pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

Column {
    id: side
    Appear { of: side }
    width: parent.width
    visible: Mods.tab === 0
    topPadding: Theme.gapLarge
    spacing: Theme.gapCard
    readonly property real labelWidth: Math.max(nameContains.advanceWidth, nameCode.advanceWidth, nameLoad.advanceWidth, namePackage.advanceWidth, nameFolder.advanceWidth, nameFiled.advanceWidth, nameGroup.advanceWidth) + Theme.gapSmall + Theme.sized(12) + Theme.gap
    TextMetrics { id: nameContains; font.pixelSize: Theme.body; text: Theme.say("Contains") }
    TextMetrics { id: nameCode; font: nameContains.font; text: Theme.say("Code") }
    TextMetrics { id: nameLoad; font: nameContains.font; text: Theme.say("Load time") }
    TextMetrics { id: namePackage; font: nameContains.font; text: Theme.say("Package id") }
    TextMetrics { id: nameFolder; font: nameContains.font; text: Theme.say("Folder") }
    TextMetrics { id: nameFiled; font: nameContains.font; text: Theme.say("Filed under") }
    TextMetrics { id: nameGroup; font: nameContains.font; text: Theme.say("Group") }
    readonly property var clashes: (Mods.info.losesTo || []).map(c => ({ loses: true, clash: c }))
        .concat((Mods.info.winsOver || []).map(c => ({ loses: false, clash: c })))
    readonly property string linkColour: "<style>a { color: " + Theme.focus + "; }</style>"
    readonly property var problems: Mods.info.problems || []
    component SavedMark: Text {
        opacity: Number(Hub.mods.notesSaved)
        color: Theme.ok
        font.pixelSize: Theme.small
        text: Theme.say("Saved")
        Behavior on opacity { NumberAnimation { duration: Theme.quick } }
    }

    Card {
        visible: side.problems.length > 0 || !!Mods.info.unreadable || !!Mods.info.versionOk || Mods.info.problemsIgnored > 0
        title: Theme.say("Before you play")
        tone: side.problems.some(p => p.error) || Mods.info.unreadable ? Theme.err : Theme.warn
        spacing: Theme.gap
        Repeater {
            model: side.problems
            ProblemLine {
                required property var modelData
                width: side.width
                error: modelData.error
                words: modelData.words
                fix: modelData.fix
                ignore: modelData.ignore
                packageId: modelData.packageId
                modId: Mods.selectedId
            }
        }
        Btn {
            readonly property int alike: Mods.overview.versionOnly || 0
            visible: alike > 1 && !!Mods.info.offerTrust && !Hub.mods.previewing
            text: Theme.fill("Ignore all {} for RimWorld {}", alike, (Mods.overview.gameVersion || ""))
            tip: Theme.say("No warning about the game version while RimWorld stays on this version.")
            onClicked: Mods.setVersionAside(Mods.overview.gameVersion || "")
        }
        Text { visible: !!Mods.info.unreadable; width: parent.width; wrapMode: Text.WordWrap; color: Theme.err; font.pixelSize: Theme.body; text: Theme.fill("Unreadable: {}", (Mods.info.unreadable || "")) }
        RowLayout {
            visible: !!Mods.info.versionOk
            width: parent.width
            spacing: Theme.gap
            Icon { name: "eye-off"; color: Theme.ink3 }
            Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.fill("Made for RimWorld {}; the warning is ignored.", (Mods.info.madeFor || "")) }
            Btn { text: Theme.say("Warn again"); onClicked: Mods.trust(Mods.selectedId, false) }
        }
        RowLayout {
            visible: Mods.info.problemsIgnored > 0
            width: parent.width
            spacing: Theme.gap
            Icon { name: "eye-off"; color: Theme.ink3 }
            Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.plural(Mods.info.problemsIgnored || 0, "{} problem is ignored.", "{} problems are ignored.") }
            Btn { text: Theme.say("Warn again"); onClicked: Hub.mods.warnAgain([Mods.selectedId]) }
        }
    }

    Card {
        visible: !!Mods.info.description
        title: Theme.say("Description")
        CopyText {
            width: parent.width
            textFormat: TextEdit.RichText
            text: side.linkColour + (Mods.info.descriptionRich || "")
            onLinkActivated: (link) => Qt.openUrlExternally(link)
            HoverHandler { enabled: parent.hoveredLink !== ""; cursorShape: Qt.PointingHandCursor }
        }
    }

    // The game keeps the texture of the mod that loads last.
    Card {
        visible: conflictLines.count > 0
        title: Theme.say("Texture conflicts")
        note: conflictLines.count
        tone: Theme.warn
        key: "about-conflicts"
        spacing: Theme.gap
        Repeater {
            id: conflictLines
            model: side.clashes
            Column {
                id: clash
                required property var modelData
                readonly property bool loses: modelData.loses
                readonly property var other: modelData.clash
                width: side.width
                spacing: Theme.gapTiny
                RowLayout {
                    width: parent.width
                    spacing: Theme.gapSmall
                    Chip { tone: clash.loses ? Theme.warn : Theme.ok; icon: clash.loses ? "arrow-down" : "arrow-up"; text: Theme.fill(clash.loses ? "loses {}" : "wins {}", clash.other.textures) }
                    Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: clash.other.name }
                }
                Row {
                    spacing: Theme.gapSmall
                    Btn { text: Theme.say("Compare"); tip: Theme.say("Both mods' versions of each shared texture, side by side"); onClicked: Mods.compareWith(clash.other.modId) }
                    Btn { visible: clash.loses; enabled: !Hub.mods.previewing; text: Theme.say("Win all"); tip: Theme.say("Adds an own rule: this mod loads after the other."); onClicked: Mods.winAll(clash.other.modId) }
                }
            }
        }
    }

    Card {
        visible: !!Mods.info.replacedBy
        title: Theme.say("Use this instead")
        RowLayout {
            width: parent.width
            spacing: Theme.gap
            Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.body; lineHeight: 1.3; text: Mods.info.replacedByVersions ? Theme.fill("{}, for RimWorld {}", Mods.info.replacedBy || "", Mods.info.replacedByVersions) : Mods.info.replacedBy || "" }
            Btn { icon: "external-link"; text: Theme.say("Open"); onClicked: Theme.openWorkshop(Mods.info.replacedById) }
        }
        Row {
            spacing: Theme.gapSmall
            Btn { visible: Mods.selectedActive && !Hub.mods.previewing; icon: "minus"; text: Theme.say("Deactivate this one"); onClicked: Mods.toggleSelected() }
            Btn { visible: !!Mods.info.workshop; danger: true; text: Theme.say("Unsubscribe…"); onClicked: Mods.unsubscribe(Mods.info.name || "", Mods.info.folder, Mods.info.path) }
        }
    }

    Card {
        visible: !!Mods.info.copies
        title: Theme.say("Two copies")
        Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.body; lineHeight: 1.3; text: Mods.info.copies || "" }
        Btn {
            visible: !!Mods.info.binLocalCopy
            danger: true
            icon: "trash-2"
            text: Theme.say("Move the local copy to the bin…")
            tip: Theme.say("The Workshop copy is then the one the game uses.")
            onClicked: Mods.binRequested(Mods.info.name || "", Mods.info.binLocalCopy.path, Mods.info.binLocalCopy.modId)
        }
    }

    Card {
        title: Theme.say("Facts")
        key: "about-facts"
        actions: SavedMark {}
        Repeater {
            model: [
                [Theme.say("Contains"), [Mods.info.contains, Mods.textures.count ? Theme.plural(Mods.textures.count, "{} texture", "{} textures") : ""].filter(s => s).join("; ")],
                [Theme.say("Code"), Mods.info.code],
                [Theme.say("Load time"), Mods.info.loadTime],
                [Theme.say("Package id"), Mods.info.packageId],
            ].filter(pair => pair[1])
            Item {
                id: fact
                required property var modelData
                width: side.width
                implicitHeight: factValue.implicitHeight
                Text { id: factName; width: side.labelWidth; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say(fact.modelData[0]) }
                CopyText { id: factValue; anchors.left: factName.right; anchors.leftMargin: Theme.gapTiny; anchors.right: parent.right; text: fact.modelData[1] }
            }
        }
        RowLayout {
            visible: !!Mods.info.path
            width: parent.width
            spacing: Theme.gap
            Text { Layout.preferredWidth: side.labelWidth; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say("Folder") }
            PathText { path: Mods.info.path || ""; copyable: true; openTip: Theme.say("Open folder") }
        }
        RowLayout {
            width: parent.width
            spacing: Theme.gap
            Defined { Layout.preferredWidth: side.labelWidth; text: Theme.say("Filed under") }
            Icon { name: Theme.categoryIcons[Theme.categoryNames.indexOf(Mods.info.category || "")] || "package"; color: Theme.categoryColours[Theme.categoryNames.indexOf(Mods.info.category || "")] || Theme.ink2 }
            Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: Mods.info.categoryChosen ? Theme.fill("{} (chosen)", Theme.say(Mods.info.category || "")) : Theme.say(Mods.info.category || "") }
            Btn {
                id: fileUnder
                text: Theme.say("Change")
                tip: Theme.say("RimStack files a mod by where it stands, what its folders hold and what its code is about. A category chosen here stands over that.")
                onClicked: Mods.menuRequested(fileUnder, 0, fileUnder.height + 2, Mods.categoryChoices(Mods.selectedId))
            }
        }
        Text { visible: !!Mods.info.held; width: parent.width; leftPadding: side.labelWidth + Theme.gap; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.body; text: Mods.info.held || "" }
        RowLayout {
            width: parent.width
            spacing: Theme.gap
            Defined { Layout.preferredWidth: side.labelWidth; text: Theme.say("Group") }
            FilterField {
                id: subBox
                Layout.fillWidth: true
                placeholder: Theme.say("A tag to group under")
                onTextChanged: {
                    if (subBox.editing)
                        Hub.mods.draftGroup(Mods.selectedId, subBox.text)
                }
                onEditingChanged: Hub.mods.keepDrafts()
                onAccepted: Hub.mods.keepDrafts()
            }
            Binding { target: subBox; property: "text"; value: Mods.info.sub || ""; when: !subBox.editing }
        }
    }

    Card {
        title: Theme.say("Tags and note")
        key: "about-notes"
        actions: SavedMark {}
        FilterField {
            id: tagBox
            width: parent.width
            placeholder: Theme.say("Tags, separated by commas")
            onTextChanged: {
                if (tagBox.editing)
                    Hub.mods.draftNote(Mods.selectedId, tagBox.text, noteBox.text)
            }
            onEditingChanged: Hub.mods.keepDrafts()
            onAccepted: Hub.mods.keepDrafts()
        }
        Binding { target: tagBox; property: "text"; value: Mods.info.tags || ""; when: !tagBox.editing }
        // The first tag is the group; a press on another makes it that.
        Flow {
            width: parent.width
            spacing: Theme.gapSmall
            Repeater {
                model: (Mods.info.tags || "").split(",").map(tag => tag.trim()).filter(tag => tag !== "")
                Chip {
                    required property string modelData
                    required property int index
                    pressable: index > 0
                    tone: index === 0 ? Theme.moved : Theme.ink2
                    icon: index === 0 ? "layers" : ""
                    text: index === 0 ? Theme.fill("{}  group", modelData) : modelData
                    tip: index === 0 ? Theme.meaning(Theme.say("Group")) : Theme.say("Group under this tag: it becomes the first")
                    onClicked: Mods.putInSub(Mods.selectedId, modelData)
                }
            }
        }
        Rectangle {
            width: parent.width
            height: Math.max(Theme.sized(54), noteBox.implicitHeight + 2 * Theme.gapSmall)
            radius: Theme.corner
            color: Theme.well
            border.width: 1
            border.color: noteBox.activeFocus ? Theme.focus : Theme.fieldLine
            Text { x: noteBox.x; y: noteBox.y; visible: noteBox.text === ""; color: Theme.ink3; font.pixelSize: Theme.body; text: Theme.say("Note") }
            TextEdit {
                id: noteBox
                Accessible.name: Theme.say("Note")
                x: Theme.gapMedium
                y: Theme.gapSmall
                width: parent.width - 2 * Theme.gapMedium
                wrapMode: TextEdit.Wrap
                color: Theme.ink
                selectionColor: Theme.accent
                font.pixelSize: Theme.body
                onTextChanged: {
                    if (activeFocus)
                        Hub.mods.draftNote(Mods.selectedId, tagBox.text, noteBox.text)
                }
                onActiveFocusChanged: Hub.mods.keepDrafts()
            }
            Binding { target: noteBox; property: "text"; value: Mods.info.note || ""; when: !noteBox.activeFocus }
        }
    }

    Card {
        id: latest
        readonly property bool workshop: !!Mods.info.workshop
        readonly property bool git: !workshop && !!Mods.info.repository
        readonly property var notes: workshop ? ((Hub.downloads.notes || {})[Mods.info.folder] || []) : git ? Hub.downloads.commitsOf(Mods.selectedId) || [] : []
        visible: !Theme.demo && (workshop || git)
        title: Theme.say("Latest changes")
        key: "about-changes"
        spacing: Theme.gap
        actions: [
            AskSteam {
                visible: latest.workshop
                idle: latest.notes.length > 0 ? Theme.say("Fetch again") : Theme.say("Fetch")
                tip: Theme.say("Reads this mod's change notes from its Workshop page. Only its Workshop id is sent.")
                onClicked: Hub.downloads.fetchNotes(Mods.selectedId)
            },
            Btn { visible: latest.workshop; icon: "external-link"; spoken: Theme.fill("Change notes of {}", Mods.info.name || ""); tip: Theme.say("The mod's change notes on the Workshop."); onClicked: Theme.openChangeNotes(Mods.info.folder) }
        ]
        Text {
            visible: latest.notes.length === 0
            width: parent.width
            wrapMode: Text.WordWrap
            color: Theme.ink3
            font.pixelSize: Theme.body
            text: !latest.workshop ? Theme.say("No commits") : Mods.info.updated ? Theme.fill("Last updated {}; its notes are not fetched", Theme.day(Mods.info.updated)) : Theme.say("Not fetched")
        }
        Repeater {
            model: latest.notes
            Column {
                id: note
                required property var modelData
                width: latest.width
                spacing: Theme.gapHair
                Text { visible: note.modelData.time > 0; color: Theme.ink3; font.pixelSize: Theme.small; text: Theme.moment(note.modelData.time) }
                Text {
                    width: parent.width
                    wrapMode: Text.WordWrap
                    maximumLineCount: 14
                    elide: Text.ElideRight
                    textFormat: Text.PlainText
                    color: note.modelData.time > 0 ? Theme.ink : Theme.err
                    font.pixelSize: Theme.body
                    lineHeight: 1.25
                    text: note.modelData.text !== "" ? note.modelData.text : Theme.say("No note written")
                }
            }
        }
    }

    Connections {
        target: Mods
        function onEditRequested(what) {
            Mods.tab = 0
            if (Theme.folded.indexOf("about-notes") >= 0)
                Theme.fold("about-notes")
            Qt.callLater(() => what === "note" ? noteBox.forceActiveFocus() : tagBox.focusInput())
        }
    }
}
