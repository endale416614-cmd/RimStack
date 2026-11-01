pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

Prompt {
    id: sheet
    wanted: Theme.sized(760)
    title: Theme.say("Keyboard")
    readonly property var groups: [
        [Theme.say("Everywhere"), [[Theme.say("Ctrl + K"), Theme.say("Search mods, commands and settings")], [Theme.say("Ctrl + 1 to 6"), Theme.say("Mods, Downloads, Textures, Saves, Logs, Settings")], ["?", Theme.say("This page")], [Theme.say("Left, Right"), Theme.say("Move a choice or the section")], [Theme.say("Tab"), Theme.say("Next control")], [Theme.say("Esc"), Theme.say("Close, cancel, clear the selection")]]],
        [Theme.say("Load order"), [[Theme.say("Ctrl + S"), Theme.say("Save")], [Theme.say("Ctrl + Z"), Theme.say("Undo")], [Theme.say("Ctrl + Shift + Z"), Theme.say("Redo")], [Theme.say("Ctrl + F"), Theme.say("Filter the load order")]]],
        [Theme.say("A list of mods"), [[Theme.say("Up, Down"), Theme.say("Select the mod above or below")], [Theme.say("Enter"), Theme.say("Activate or deactivate")], [Theme.say("Alt + Up, Alt + Down"), Theme.say("Move the mod one place")], [Theme.say("Ctrl + click"), Theme.say("Add or remove one from the selection")], [Theme.say("Shift + click"), Theme.say("Select through")], ["T", Theme.say("Type its tags")], ["N", Theme.say("Type its note")], [Theme.say("Menu"), Theme.say("The mod's menu")]]],
        [Theme.say("Texture comparison"), [[Theme.say("Up, Down"), Theme.say("Previous or next texture")], ["1, 2", Theme.say("Keep the first or the second mod's")], [Theme.say("Esc"), Theme.say("Back to the lists")]]],
    ]
    FontMetrics { id: words; font.pixelSize: Theme.small }
    GridLayout {
        id: groups
        width: parent.width
        columns: Math.max(1, Math.min(2, Math.floor((width + columnSpacing) / (words.averageCharacterWidth * 48 + columnSpacing))))
        columnSpacing: Theme.gapPage
        rowSpacing: Theme.gapCard
        Repeater {
            model: sheet.groups
            ColumnLayout {
                id: group
                required property var modelData
                Layout.alignment: Qt.AlignTop
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                spacing: Theme.gapThin
                Text { bottomPadding: Theme.gapThin; color: Theme.ink2; font.pixelSize: Theme.small; font.weight: Font.DemiBold; text: Theme.say(group.modelData[0]) }
                GridLayout {
                    Layout.fillWidth: true
                    flow: GridLayout.TopToBottom
                    rows: group.modelData[1].length
                    columnSpacing: Theme.gapWide
                    rowSpacing: Theme.gapThin
                    Repeater {
                        model: group.modelData[1]
                        Row {
                            id: key
                            required property var modelData
                            Layout.alignment: Qt.AlignTop
                            spacing: Theme.gapTiny
                            Accessible.role: Accessible.StaticText
                            Accessible.name: key.modelData[0]
                            Repeater {
                                model: key.modelData[0].split(", ")
                                Row {
                                    id: combo
                                    required property string modelData
                                    required property int index
                                    spacing: Theme.gapTiny
                                    Text { visible: combo.index > 0; anchors.verticalCenter: parent.verticalCenter; color: Theme.ink3; font.pixelSize: Theme.small; text: "," }
                                    Repeater {
                                        model: combo.modelData.split(" + ")
                                        KeyCap { required property string modelData; text: modelData }
                                    }
                                }
                            }
                        }
                    }
                    Repeater {
                        model: group.modelData[1]
                        Text {
                            required property var modelData
                            Layout.fillWidth: true
                            Layout.preferredWidth: words.averageCharacterWidth * 24
                            Layout.alignment: Qt.AlignTop
                            wrapMode: Text.WordWrap
                            color: Theme.ink2
                            font.pixelSize: Theme.small
                            text: Theme.say(modelData[1])
                        }
                    }
                }
            }
        }
    }
    buttons: [ Btn { primary: true; text: Theme.say("Close"); onClicked: sheet.close() } ]
}
