pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

Column {
    id: empty
    property string icon
    property string title
    property string hint
    spacing: Theme.gapSmall
    Icon { anchors.horizontalCenter: parent.horizontalCenter; name: empty.icon; size: Theme.sized(28); color: Theme.ink3 }
    Text { width: parent.width; horizontalAlignment: Text.AlignHCenter; color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: empty.title }
    Text { width: parent.width; horizontalAlignment: Text.AlignHCenter; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.body; lineHeight: 1.3; text: empty.hint }
}
