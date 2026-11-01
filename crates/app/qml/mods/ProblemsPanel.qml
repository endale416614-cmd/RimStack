pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

Column {
    id: panel
    width: parent.width
    visible: Mods.problemsOpen && Mods.comparing === null && !Hub.mods.previewing
    spacing: Theme.gapLarge
    readonly property var groups: Mods.overview.troubleGroups || []

    RowLayout {
        width: parent.width
        Column {
            Layout.fillWidth: true
            Text { color: Theme.ink; font.pixelSize: Theme.heading; font.weight: Font.DemiBold; text: Theme.say("Problems") }
            Text { topPadding: Theme.gapHair; color: Theme.ink2; font.pixelSize: Theme.body; text: Mods.troubles.length === 0 ? Theme.say("None") : Theme.fill("{} with errors, {} with warnings", Hub.mods.errors, Hub.mods.warnings) }
        }
        Btn { icon: "x"; tip: Theme.say("Close (Esc)"); onClicked: Mods.problemsOpen = false }
    }
    Btn {
        visible: Mods.fixable > 0
        primary: true
        text: Theme.plural(Mods.fixable, "Activate the {} needed mod", "Activate the {} needed mods")
        tip: Theme.say("Activates every needed mod that is installed, each where its rules put it. One Undo takes them all back.")
        onClicked: Mods.fixAll()
    }
    Btn {
        visible: !Theme.demo && !Hub.search.active
        icon: "search"
        text: Theme.say("Find the broken mod")
        tip: Theme.say("Switches half of the mods off, you play and say whether it still breaks, until one mod is left. Your list is kept and comes back at the end.")
        onClicked: Mods.findBrokenRequested()
    }
    Repeater {
        model: panel.groups
        Card {
            id: group
            required property var modelData
            title: Theme.say(modelData.name)
            note: modelData.lines.length
            tone: modelData.error ? Theme.err : Theme.warn
            key: "problems-" + modelData.name
            spacing: Theme.gap
            Btn {
                visible: group.modelData.trusted > 1
                text: Theme.fill("Ignore all {} for RimWorld {}", group.modelData.trusted, (Mods.overview.gameVersion || ""))
                tip: Theme.say("No warning about the game version while RimWorld stays on this version.")
                onClicked: Mods.setVersionAside(Mods.overview.gameVersion || "")
            }
            Text {
                visible: group.modelData.shared
                width: group.width
                wrapMode: Text.WordWrap
                color: Theme.ink2
                font.pixelSize: Theme.body
                lineHeight: 1.25
                text: Theme.fill("Each lists other game versions than RimWorld {}. Many such mods still work.", (Mods.overview.gameVersion || ""))
            }
            Repeater {
                model: group.modelData.shared ? group.modelData.lines : []
                Item {
                    id: named
                    required property var modelData
                    width: group.width
                    height: Theme.control
                    Rectangle { anchors.fill: parent; radius: Theme.corner; color: Theme.hover; opacity: over.hovered ? 0.05 : 0 }
                    Text { x: Theme.gapSmall; width: ignoreBtn.x - 2 * Theme.gapSmall; anchors.verticalCenter: parent.verticalCenter; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: named.modelData.name }
                    HoverHandler { id: over; cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: Mods.showMod(named.modelData.modId) }
                    Btn { id: ignoreBtn; visible: over.hovered; anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; text: Theme.say("Ignore"); tip: Theme.say("No warning about the game version for this mod"); onClicked: Mods.applyFix(named.modelData.modId, named.modelData.fix) }
                }
            }
            Repeater {
                model: group.modelData.shared ? [] : group.modelData.lines
                ProblemLine {
                    required property var modelData
                    width: group.width
                    error: modelData.error
                    name: modelData.name
                    words: modelData.words
                    fix: modelData.fix
                    ignore: modelData.ignore
                    packageId: modelData.packageId
                    modId: modelData.modId
                }
            }
        }
    }
    EmptyState { visible: Mods.troubles.length === 0; width: parent.width; title: Theme.say("Nothing stands in the way") }
}
