pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

Rectangle {
    id: record
    // { time, mods, errors, ids, added, removed, reordered } of the report's launches.
    property var launch: null
    property string outcome
    signal useList(var ids)
    signal closed()
    color: Theme.win
    Column {
        x: Theme.gapWide
        y: Theme.gapCard
        width: Math.min(parent.width - 2 * Theme.gapWide, Theme.sized(620))
        spacing: Theme.gapSmall
        Row {
            spacing: Theme.gap
            Text { anchors.verticalCenter: parent.verticalCenter; color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: record.launch ? Theme.moment(record.launch.time) : "" }
            Chip { anchors.verticalCenter: parent.verticalCenter; tone: record.launch && record.launch.errors > 0 ? Theme.err : Theme.ink2; text: record.outcome }
        }
        Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say("The game keeps the logs of its last two launches only. Of this one RimStack kept the load order.") }
        GroupTitle { text: Theme.say("Its load order") }
        Text {
            readonly property var differs: record.launch ? [
                record.launch.removed > 0 ? Theme.plural(record.launch.removed, "{} mod no longer in the list", "{} mods no longer in the list") : "",
                record.launch.added > 0 ? Theme.plural(record.launch.added, "{} mod added since", "{} mods added since") : "",
                record.launch.reordered ? Theme.say("order changed") : ""].filter(part => part) : []
            width: parent.width
            wrapMode: Text.WordWrap
            color: Theme.ink
            font.pixelSize: Theme.body
            text: record.launch ? Theme.plural(record.launch.mods, "{} mod", "{} mods") + ". " + (differs.length === 0 ? Theme.say("The same as the list now") : differs.join(", ")) : ""
        }
        Row {
            topPadding: Theme.gap
            spacing: Theme.gapSmall
            Btn { enabled: !!record.launch && (record.launch.added + record.launch.removed > 0 || record.launch.reordered); text: Theme.say("Use its list"); tip: Theme.say("Makes this launch's load order the list on the Mods screen. Save writes it."); onClicked: record.useList(record.launch.ids) }
            Btn { text: Theme.say("Back to the log"); onClicked: record.closed() }
        }
    }
}
