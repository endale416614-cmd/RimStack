pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

Rectangle {
    id: btn
    property alias text: btnLabel.text
    property bool primary: false
    property bool large: false
    property bool danger: false
    property string icon
    property bool menu: false
    property string tip
    property string spoken
    property color tone: "transparent"
    readonly property bool toned: tone.a > 0 && !primary
    property bool compact: false
    readonly property bool bare: compact && icon !== ""
    signal clicked()
    readonly property real fullWidth: btnLabel.implicitWidth + Number(icon !== "") * (Theme.icon + btnContent.spacing) + Theme.sized(20)
    implicitWidth: btnContent.implicitWidth + (large ? Theme.sized(36) : (text === "" || bare) && !menu ? Theme.sized(14) : Theme.sized(20))
    implicitHeight: large ? Theme.controlLarge : Theme.control
    radius: Theme.corner
    readonly property real frame: enabled ? 1 : 0.4
    activeFocusOnTab: true
    Keys.onSpacePressed: btn.clicked()
    Keys.onReturnPressed: btn.clicked()
    Keys.onEnterPressed: btn.clicked()
    border.width: primary ? 0 : 1
    border.color: toned ? Qt.rgba(tone.r, tone.g, tone.b, frame) : Qt.rgba(Theme.fieldLine.r, Theme.fieldLine.g, Theme.fieldLine.b, frame)
    FocusRing {}
    Accessible.role: Accessible.Button
    Accessible.ignored: !visible
    Accessible.name: spoken !== "" ? spoken : text !== "" ? text : tip !== "" ? tip : icon
    Accessible.description: tip
    Accessible.onPressAction: if (enabled) btn.clicked()
    readonly property color ground: primary ? (btnHover.hovered ? Theme.accentHi : Theme.accent)
        : toned ? Qt.rgba(tone.r, tone.g, tone.b, btnHover.hovered ? 0.3 : 0.18)
        : (btnHover.hovered ? Theme.ctlHi : Theme.ctl)
    color: Qt.rgba(ground.r, ground.g, ground.b, ground.a * frame)
    scale: btnPress.pressed && !Theme.stored.reduceMotion ? 0.97 : 1
    Behavior on color { ColorAnimation { duration: Theme.quick } }
    Behavior on scale { NumberAnimation { duration: Theme.quick / 2 } }
    Row {
        id: btnContent
        anchors.centerIn: parent
        spacing: Theme.gapSmall
        opacity: btn.enabled ? 1 : 0.5
        Icon { visible: btn.icon !== ""; name: btn.icon; color: btnLabel.color; anchors.verticalCenter: parent.verticalCenter }
        Text {
            id: btnLabel
            visible: text !== "" && !btn.bare
            color: btn.primary ? Theme.onSelected : btn.danger && btn.enabled ? Theme.err : btn.toned ? btn.tone : Theme.ink
            font.pixelSize: btn.large ? Theme.body : Theme.small
            font.weight: btn.large ? Font.DemiBold : btn.primary || btn.toned ? Font.Medium : Font.Normal
        }
        Icon { visible: btn.menu; name: "chevron-down"; size: Theme.sized(12); color: btnLabel.color; opacity: 0.8; anchors.verticalCenter: parent.verticalCenter }
    }
    HoverHandler { id: btnHover }
    Tip { text: btn.bare && btn.tip === "" ? btn.text : btn.tip; over: btnHover.hovered || btn.activeFocus }
    TapHandler {
        id: btnPress
        gesturePolicy: TapHandler.ReleaseWithinBounds
        grabPermissions: PointerHandler.CanTakeOverFromAnything | PointerHandler.ApprovesTakeOverByItems
        onTapped: btn.clicked()
    }
}
