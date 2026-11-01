pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import RimStack.Ui

Flickable {
    id: scrolled
    default property alias body: inner.data
    property alias spacing: inner.spacing
    property int padding: 0
    readonly property alias column: inner
    contentHeight: inner.implicitHeight + 2 * padding
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    interactive: contentHeight > height
    Wheel {}
    ScrollBar.vertical: Scroller { id: bar }
    Column {
        id: inner
        x: scrolled.padding
        y: scrolled.padding
        width: scrolled.width - 2 * scrolled.padding - bar.width
    }
}
