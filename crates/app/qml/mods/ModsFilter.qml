pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import RimStack.Ui

FilterField {
    id: filter
    z: 5
    icon: "funnel"
    keys: Theme.say("Ctrl + F")
    chips: true
    placeholder: Theme.say("Filter active and inactive mods")
    found: Mods.filtered === "" ? "" : Theme.fill("{} active, {} inactive", Theme.count(Mods.active.matching), Theme.count(Mods.inactive.matching))
    readonly property alias suggesting: suggestions.visible
    onTypingChanged: suggestions.shut = false
    onEditingChanged: suggestions.shut = false
    onTapped: suggestions.shut = false
    onEscaped: suggestions.shut = true
    Popup {
        id: suggestions
        property bool shut: false
        visible: offers.count > 0 && filter.editing && filter.visible && !shut
        onClosed: shut = true
        closePolicy: Popup.CloseOnPressOutsideParent
        y: filter.height + 4
        width: Math.min(filter.width, Theme.sized(420))
        padding: 6
        background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }
        readonly property string widestKey: "is:notinsave"
        TextMetrics { id: keyFont; font.family: "monospace"; font.pixelSize: Theme.small; text: suggestions.widestKey }
        contentItem: Column {
            id: suggestList
            Repeater {
                id: offers
                model: [
                    ["is:error", Theme.plural(Hub.mods.errors, "{} mod", "{} mods")],
                    ["is:warning", Theme.plural(Hub.mods.warnings, "{} mod", "{} mods")],
                    ["is:conflict", Theme.plural(Hub.mods.conflicts, "{} mod sharing textures", "{} mods sharing textures")],
                    ["is:changed", Theme.say("moved or activated since the last save")],
                    ["is:new", Theme.say("installed since the game was last started")],
                    ["is:updated", Hub.downloads.behindActive > 0 ? Theme.fill("{} with an update", Hub.downloads.behindActive) : Theme.say("after checking for updates")],
                    ["is:active", Theme.say("in the active list only")],
                    ["is:inactive", Theme.say("in the inactive list only")],
                    ["is:workshop", Theme.say("from the Workshop")],
                    ["is:local", Theme.say("in the game's Mods folder")],
                    ["is:git", Theme.say("a git checkout")],
                    ["is:notinsave", Theme.say("active, and not in the newest save")],
                    ["is:recent", Theme.say("updated by Steam in the last week")],
                    ["is:duplicate", Theme.say("installed more than once")],
                    ["has:code", Theme.say("ships compiled code")],
                    ["patches:", Theme.say("patches this method of the game")],
                    ["tag:", Theme.say("has this tag")],
                    ["author:", Theme.say("by this author")],
                ].filter(offer => offer[0].startsWith(filter.partial) && offer[0] !== filter.partial && filter.terms.indexOf(offer[0]) < 0)
                Rectangle {
                    id: offer
                    required property var modelData
                    width: suggestList.width
                    height: Theme.sized(26)
                    radius: Theme.corner
                    color: offerHover.hovered ? Theme.chip : "transparent"
                    Accessible.role: Accessible.Button
                    Accessible.name: Theme.fill("Filter by {}", modelData[0])
                    Text { id: offerKey; width: keyFont.advanceWidth; anchors.left: parent.left; anchors.leftMargin: Theme.gap; anchors.verticalCenter: parent.verticalCenter; color: Theme.ink; font: keyFont.font; text: offer.modelData[0] }
                    Text { anchors.left: offerKey.right; anchors.leftMargin: Theme.sized(16); anchors.right: parent.right; anchors.rightMargin: Theme.gap; anchors.verticalCenter: parent.verticalCenter; elide: Text.ElideRight; color: Theme.ink2; font.pixelSize: Theme.small; text: offer.modelData[1] }
                    HoverHandler { id: offerHover }
                    TapHandler {
                        onTapped: {
                            filter.take(offer.modelData[0])
                            filter.focusInput()
                        }
                    }
                }
            }
        }
    }
}
