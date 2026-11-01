pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

Column {
    id: side
    Appear { of: side }
    width: parent.width

    readonly property var model: Mods.selectedActive ? Mods.active : Mods.inactive
    readonly property int most: 12
    // The selected mods in the order of their list: { name, colour, load, trouble }.
    // One hidden by the filter or under a folded heading has no row to read.
    readonly property var chosen: {
        const found = []
        for (const id of Mods.selectedIds) {
            const position = model.indexOf(id)
            const row = position < 0 ? -1 : model.rowOf(position)
            if (row < 0)
                continue
            const shown = model.at(row)
            found.push({
                position: position,
                name: shown.name,
                colour: Theme.categoryColours[shown.category] || Theme.ink3,
                load: shown.loadTime || "",
                trouble: shown.trouble,
            })
        }
        return found.sort((a, b) => a.position - b.position)
    }
    readonly property int troubled: chosen.filter(m => m.trouble > 0).length
    readonly property int first: chosen.length > 0 ? chosen[0].position + 1 : 0
    readonly property int last: chosen.length > 0 ? chosen[chosen.length - 1].position + 1 : 0
    readonly property bool inARow: last - first + 1 === chosen.length
    property bool open: false
    onChosenChanged: if (chosen.length <= most) open = false

    Text { color: Theme.ink; font.pixelSize: Theme.heading; font.weight: Font.DemiBold; text: Theme.plural(Mods.selectedCount, "{} mod selected", "{} mods selected") }
    Text {
        topPadding: Theme.gapHair
        color: Theme.ink2
        font.pixelSize: Theme.body
        text: !Mods.selectedActive ? Theme.say("Inactive")
            : side.chosen.length === 0 ? Theme.say("In the load order")
            : side.inARow ? Theme.fill("In the load order, {} to {}", side.first, side.last)
            : Theme.fill("In the load order, between {} and {}", side.first, side.last)
    }

    GroupTitle { text: Theme.say("Selected") }
    Repeater {
        model: side.open ? side.chosen : side.chosen.slice(0, side.most)
        RowLayout {
            id: one
            required property var modelData
            width: side.width
            height: Theme.sized(24)
            spacing: Theme.gap
            Rectangle { implicitWidth: 7; implicitHeight: 7; radius: 3.5; color: one.modelData.colour }
            Text { Layout.fillWidth: true; elide: Text.ElideMiddle; color: Theme.ink; font.pixelSize: Theme.body; text: one.modelData.name }
            Icon { visible: one.modelData.trouble > 0; name: one.modelData.trouble === 2 ? "circle-alert" : "triangle-alert"; size: Theme.sized(12); color: one.modelData.trouble === 2 ? Theme.err : Theme.ink3 }
            Text { visible: text !== ""; color: Theme.ink3; font.pixelSize: Theme.small; font.features: { "tnum": 1 }; text: one.modelData.load }
        }
    }
    Item { visible: more.visible; width: 1; height: Theme.gapSmall }
    Btn {
        id: more
        visible: side.chosen.length > side.most
        text: side.open ? Theme.fill("Show {}", side.most) : Theme.fill("{} more", (side.chosen.length - side.most))
        onClicked: side.open = !side.open
    }

    GroupTitle { visible: facts.visible; text: Theme.say("Together") }
    Grid {
        id: facts
        visible: side.troubled > 0 || Mods.selectedCount > side.chosen.length
        width: parent.width
        columns: 2
        columnSpacing: Theme.gapLarge
        rowSpacing: Theme.gapTiny
        Text { visible: side.troubled > 0; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say("With a problem") }
        Text { visible: side.troubled > 0; color: Theme.ink; font.pixelSize: Theme.body; text: Mods.versionWarned.length === 0 ? side.troubled + "" : Mods.versionWarned.length === side.troubled ? Theme.fill("{}, all for the game version", side.troubled) : Theme.fill("{}, {} for the game version", side.troubled, Mods.versionWarned.length) }
        Text { visible: Mods.selectedCount > side.chosen.length; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say("Not in view") }
        Text { visible: Mods.selectedCount > side.chosen.length; color: Theme.ink; font.pixelSize: Theme.body; text: Mods.selectedCount - side.chosen.length }
    }
}
