pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

Item {
    id: slide
    property real from: 0
    property real to: 1
    property real step: 0.05
    property real value: 0
    property string title
    property string reading
    signal moved(real value)
    readonly property real shown: value
    readonly property real part: (shown - from) / (to - from)
    function at(x) {
        const raw = from + Math.max(0, Math.min(1, x / track.width)) * (to - from)
        return Math.round(raw / step) * step
    }
    function put(x) {
        const next = at(x)
        if (Math.abs(next - value) > 1e-6)
            slide.moved(next)
    }
    implicitWidth: Theme.sized(180)
    implicitHeight: Theme.control
    activeFocusOnTab: true
    Accessible.role: Accessible.Slider
    Accessible.name: title
    Accessible.description: reading
    Keys.onLeftPressed: slide.moved(Math.max(from, value - step))
    Keys.onRightPressed: slide.moved(Math.min(to, value + step))
    Rectangle {
        id: track
        x: knob.width / 2
        width: parent.width - knob.width
        height: Theme.gapTiny
        radius: Math.min(width, height) / 2
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.ctl
        Rectangle { width: parent.width * slide.part; height: parent.height; radius: Math.min(width, height) / 2; color: Theme.accent }
    }
    Rectangle {
        id: knob
        x: track.x + track.width * slide.part - width / 2
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.sized(16)
        height: width
        radius: Math.min(width, height) / 2
        color: Theme.ink
        FocusRing { of: slide; corner: knob.radius }
    }
    HoverHandler { cursorShape: Qt.PointingHandCursor }
    TapHandler { onTapped: point => { slide.forceActiveFocus(); slide.put(point.position.x - track.x) } }
    DragHandler {
        id: drag
        property real latest: 0
        property real stamp: 0
        target: null
        onCentroidChanged: {
            if (!active)
                return
            latest = centroid.position.x - track.x
            if (Date.now() - stamp >= 16) {
                stamp = Date.now()
                slide.put(latest)
            }
        }
        onActiveChanged: if (!active) slide.put(latest)
    }
}
