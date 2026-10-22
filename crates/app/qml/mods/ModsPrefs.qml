import QtCore
import QtQuick

Settings {
    category: "Mods"
    property real inactive: 290
    property real details: 360
    property bool inactiveHidden: false
    property string listName: ""
    property string lastMod: ""
    property int lastTab: 0
    property string lastFilter: ""
    property real lastScroll: 0
    property string columns: "author,version,load"
    property bool inactiveByName: false
    // The load order under category headings: 1 always, 0 never (one
    // numbered list), -1 when the order is close to category order.
    property int headings: -1
    property string columnSizes: ""
    // The inactive list's part of the room the two lists share, beside the details.
    property real inactiveShare: 0.5
}
