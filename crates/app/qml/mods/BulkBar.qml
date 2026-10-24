pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

Rectangle {
    id: bar
    readonly property bool wanted: Mods.selectedCount > 1 && Mods.comparing === null && !Hub.mods.previewing
    visible: height > 0
    height: wanted ? Theme.header : 0
    clip: true
    Behavior on height { NumberAnimation { duration: Theme.quick; easing.type: Easing.OutCubic } }
    color: Theme.bar
    Rectangle { width: parent.width; height: 1; color: Theme.edge }

    RowLayout {
        id: actions
        anchors.horizontalCenter: parent.horizontalCenter
        y: (Theme.header - height) / 2
        spacing: Theme.gapSmall
        Text { color: Theme.ink; font.pixelSize: Theme.body; font.weight: Font.DemiBold; text: Theme.fill("{} selected", Mods.selectedCount) }
        Item { implicitWidth: 6 }
        Btn { primary: true; icon: Mods.selectedActive ? "minus" : "plus"; text: Mods.selectedActive ? Theme.say("Deactivate") : Theme.say("Activate"); onClicked: Mods.moveSelected() }
        Btn { icon: "tag"; text: Theme.say("Tag…"); tip: Theme.say("Adds a tag to every selected mod"); onClicked: Mods.bulkRequested("tag") }
        Btn { text: Theme.say("Group…"); tip: Theme.say("Makes a tag the first of every selected mod: inside a category they are kept side by side under it"); onClicked: Mods.bulkRequested("sub") }
        Btn { visible: Mods.versionWarned.length > 0; icon: "eye-off"; text: Theme.fill("Ignore version warning  {}", Mods.versionWarned.length); tip: Theme.say("No warning about the game version for the selected mods that have one"); onClicked: Mods.trustSelected(true) }
        Rectangle { visible: Mods.selectedActive && Mods.columns.shownBy === ""; implicitWidth: 1; implicitHeight: Theme.sized(16); color: Theme.line }
        Btn { visible: Mods.selectedActive && Mods.columns.shownBy === ""; icon: "arrow-up"; text: Theme.say("To top"); tip: Theme.say("To the top of their category"); onClicked: Mods.selectedToEdge(true) }
        Btn { visible: Mods.selectedActive && Mods.columns.shownBy === ""; icon: "arrow-down"; text: Theme.say("To bottom"); tip: Theme.say("To the bottom of their category"); onClicked: Mods.selectedToEdge(false) }
        Btn { visible: !Mods.selectedActive && Mods.upkept.unsubscribe.length > 0; icon: "minus"; danger: true; text: Theme.say("Unsubscribe…"); tip: Theme.say("Unsubscribes from every selected Workshop mod; Steam then deletes their folders"); onClicked: Mods.unsubscribeSelected() }
        Btn { visible: Mods.upkept.bin.length > 0; icon: "trash-2"; danger: true; text: Theme.say("Move to the bin…"); tip: Theme.say("Moves the folders of the selected mods to the bin"); onClicked: Mods.binSelected() }
        Item { implicitWidth: Theme.gapWide }
        Btn { text: Theme.say("Clear selection"); tip: Theme.say("Clear the selection (Esc)"); onClicked: Mods.select(-1, false) }
    }
}
