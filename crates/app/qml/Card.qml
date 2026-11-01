pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

Rectangle {
    id: card
    property string title
    property string note
    property color tone: Theme.ink2
    property string key
    property bool startsFolded: false
    default property alias body: content.data
    property alias actions: acts.data
    property alias spacing: content.spacing
    readonly property bool folded: key !== "" && (Theme.folded.indexOf(key) >= 0) !== startsFolded

    property real shown: folded ? 0 : 1
    Behavior on shown { NumberAnimation { duration: Theme.quick; easing.type: Easing.OutCubic } }
    width: parent ? parent.width : 0
    implicitHeight: head.height + shown * (content.implicitHeight + 6)
    clip: shown > 0 && shown < 1
    color: "transparent"

    Item {
        id: head
        readonly property bool wrapped: acts.children.length > 0 && name.width + Math.min(said.implicitWidth, Theme.sized(120)) + acts.width + fold.width + Theme.gap + 3 * Theme.gapSmall > width
        readonly property real line: Theme.sized(acts.children.length > 0 && !wrapped ? 34 : 28)
        width: parent.width
        height: line + (wrapped ? Theme.sized(34) : 0)
        activeFocusOnTab: card.key !== ""
        Accessible.role: card.key !== "" ? Accessible.Button : Accessible.Heading
        Accessible.name: card.title
        Accessible.description: name.meaning
        Keys.onReturnPressed: Theme.fold(card.key)
        Keys.onSpacePressed: Theme.fold(card.key)
        Text {
            id: name
            readonly property string meaning: Theme.meaning(card.title)
            y: (head.line - height) / 2
            color: card.tone
            font.pixelSize: Theme.small
            font.weight: Font.DemiBold
            font.underline: head.activeFocus
            text: card.title
            Dots { visible: name.meaning !== ""; y: parent.height + 1; width: parent.width; tone: card.tone }
            HoverHandler { id: pointed }
            Tip { text: name.meaning; over: pointed.hovered || head.activeFocus }
        }
        Text { id: said; y: (head.line - height) / 2; anchors.left: name.right; anchors.leftMargin: Theme.gap; anchors.right: head.wrapped ? fold.left : acts.left; anchors.rightMargin: Theme.gapSmall; elide: Text.ElideRight; color: Theme.ink3; font.pixelSize: Theme.small; text: card.note }
        Row { id: acts; z: 1; x: head.wrapped ? 0 : fold.x - Theme.gapSmall - width; y: head.wrapped ? head.line + (Theme.sized(34) - height) / 2 : (head.line - height) / 2; spacing: Theme.gapSmall }
        Icon { id: fold; visible: card.key !== ""; width: visible ? Theme.icon : 0; anchors.right: parent.right; y: (head.line - height) / 2; name: "chevron-down"; rotation: -90 * (1 - card.shown); color: Theme.ink3 }
        HoverHandler { cursorShape: card.key !== "" ? Qt.PointingHandCursor : Qt.ArrowCursor }
        // A press on one of its buttons is that button's alone.
        TapHandler { enabled: card.key !== ""; onTapped: point => { if (acts.children.length === 0 || (head.wrapped ? point.position.y < head.line : point.position.x < acts.x)) Theme.fold(card.key) } }
    }
    Column {
        id: content
        visible: card.shown > 0
        opacity: card.shown
        y: head.height
        width: parent.width
        spacing: Theme.gapSmall
    }
}
