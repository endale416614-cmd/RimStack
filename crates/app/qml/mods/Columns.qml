import QtQuick
import RimStack.Ui

QtObject {
    id: columns

    required property ModsPrefs prefs
    required property bool loadTimes
    // "" once load times are known.
    required property string loadTimesHow
    signal shownByAsked()

    readonly property ModsData rules: Hub.mods
    readonly property var labels: ({ author: "Author", version: "Version", load: "Load" })
    readonly property var notes: ({ load: loadTimesHow })
    readonly property var chosen: prefs.columns === "" ? [] : prefs.columns.split(",")
    readonly property var shown: chosen.filter(column => column !== "load" || loadTimes)
    // At 100% text, where a column was dragged.
    readonly property var chosenWidths: rules.columnWidths(prefs.columnSizes)
    readonly property var widths: ({ author: Theme.sized(chosenWidths.author || 142), version: Theme.sized(chosenWidths.version || 84), load: Theme.sized(chosenWidths.load || 56) })

    // "" for the load order itself.
    property string shownBy: ""
    property bool descending: false
    function showBy(column) {
        const now = rules.columnPressed(shownBy, descending, column)
        shownBy = now[0]
        descending = now[1]
        shownByAsked()
    }

    function toggle(column) { prefs.columns = rules.columnsToggled(prefs.columns, column) }
    function move(column, to) { prefs.columns = rules.columnsMoved(prefs.columns, column, to) }
    function size(column, width) { prefs.columnSizes = rules.columnSized(prefs.columnSizes, column, width / Theme.grow) }

    readonly property var entries: Object.keys(labels).map(column => ({
        on: chosen.indexOf(column) >= 0,
        label: Theme.say(labels[column]),
        text: (chosen.indexOf(column) >= 0 ? "✓  " : "     ") + Theme.say(labels[column]),
        note: notes[column] || "",
        run: () => toggle(column)
    }))
}
