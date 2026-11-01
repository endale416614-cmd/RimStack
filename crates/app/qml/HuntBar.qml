// A round's list is only put on the Mods screen; the game gets it on Save.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

Rectangle {
    id: bar
    readonly property SearchData search: Hub.search
    signal useList(var ids)
    signal saveAndPlay()

    function tryRound() {
        if (search.asking)
            useList(search.list)
    }
    function start() {
        search.start()
        tryRound()
    }
    function answer(broken) {
        search.answer(broken)
        tryRound()
    }
    function end(withoutFound) {
        useList(search.end(withoutFound))
    }

    readonly property bool asking: search.asking
    visible: search.active
    implicitHeight: visible ? Math.max(Theme.sized(44), words.implicitHeight + 2 * Theme.gapMedium) : 0
    color: Theme.banner
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Theme.gapCard
        anchors.rightMargin: Theme.gapLarge
        spacing: Theme.gap
        Icon { name: "search"; color: Theme.moved }
        Text {
            id: words
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            textFormat: Text.StyledText
            color: Theme.ink
            font.pixelSize: Theme.body
            text: !bar.search.active ? ""
                : bar.search.found !== "" ? Theme.fill("{} is the mod the game breaks with.", "<b>" + Theme.plain(bar.search.found) + "</b>")
                : bar.search.stuck !== "" ? Theme.fill("One of these, which need each other: {}", "<b>" + Theme.plain(bar.search.stuck) + "</b>")
                : "<b>" + Theme.say("Finding the broken mod.") + "</b> " + Theme.plural(bar.search.rounds, "About {} round left, {} suspects.", "About {} rounds left, {} suspects.", bar.search.suspects)
                    + " " + Theme.say("Save, play, then say how it went.")
        }
        Icon {
            visible: bar.asking
            name: "info"
            color: Theme.ink2
            HoverHandler { id: how }
            Tip { over: how.hovered; text: Theme.say("Half of the suspects are switched off in the list now. Do not load or save a colony during a round; the main menu or a new test colony shows whether it breaks.") }
        }
        Btn { visible: bar.asking; primary: Mods.dirty; icon: "play"; text: Theme.say("Save and play"); tip: Theme.say("Writes this round's list to ModsConfig.xml and starts the game. The newest save was copied to RimStack's data folder, save-before-search, when the search began."); onClicked: bar.saveAndPlay() }
        Btn { visible: bar.asking; text: Theme.say("Still broken"); onClicked: bar.answer(true) }
        Btn { visible: bar.asking; text: Theme.say("Works now"); onClicked: bar.answer(false) }
        Btn { visible: bar.search.found !== ""; primary: true; text: Theme.say("My list without it"); tip: Theme.say("The list the search began with, less this mod. Nothing is written before Save."); onClicked: bar.end(true) }
        Btn { text: bar.search.found !== "" ? Theme.say("My list as it was") : Theme.say("Stop"); tip: Theme.say("The list the search began with. Nothing is written before Save."); onClicked: bar.end(false) }
    }
    Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.edge }
}
