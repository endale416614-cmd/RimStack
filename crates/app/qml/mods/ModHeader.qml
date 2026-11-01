pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

Column {
    id: side
    width: parent.width

    property bool large: true
    Rectangle {
        visible: side.large && !!Mods.info.preview
        width: parent.width
        height: Math.min(width * 9 / 16, Theme.sized(220))
        radius: Theme.cornerLarge
        color: Theme.well
        clip: true
        Image { anchors.fill: parent; asynchronous: true; fillMode: Image.PreserveAspectFit; sourceSize.width: Theme.sized(720); source: side.large && Mods.info.preview ? Theme.fileUrl(Mods.info.preview) : "" }
        HoverHandler { cursorShape: Qt.PointingHandCursor }
        TapHandler { onTapped: side.large = false }
    }
    Item { visible: side.large && !!Mods.info.preview; width: 1; height: Theme.gapMedium }
    RowLayout {
        width: parent.width
        spacing: Theme.gapLarge
        Rectangle {
            visible: !(side.large && !!Mods.info.preview)
            Layout.alignment: Qt.AlignTop
            implicitWidth: Theme.sized(64)
            implicitHeight: Theme.sized(64)
            radius: Theme.cornerLarge
            color: Theme.well
            clip: true
            Image {
                anchors.fill: parent
                visible: status === Image.Ready
                asynchronous: true
                fillMode: Image.PreserveAspectCrop
                sourceSize.width: Theme.sized(192)
                source: Mods.info.preview ? Theme.fileUrl(Mods.info.preview) : ""
            }
            Text { visible: !Mods.info.preview; anchors.centerIn: parent; color: Theme.brand; font.pixelSize: Theme.display; font.weight: Font.DemiBold; text: (Mods.info.name || "?").charAt(0).toUpperCase() }
            HoverHandler { enabled: !!Mods.info.preview; cursorShape: Qt.PointingHandCursor }
            TapHandler { enabled: !!Mods.info.preview; onTapped: side.large = !side.large }
            Tip { text: Mods.info.preview ? Theme.say("Click for the picture at full size") : "" }
        }
        Column {
            Layout.fillWidth: true
            spacing: Theme.gapThin
            CopyText { width: parent.width; font.pixelSize: Theme.heading; font.weight: Font.DemiBold; text: Mods.info.name || "" }
            Text { width: parent.width; elide: Text.ElideRight; color: Theme.ink2; font.pixelSize: Theme.body; text: Mods.info.author || "" }
            Flow {
                width: parent.width
                topPadding: Theme.gapThin
                spacing: Theme.gapTiny
                Chip { visible: !!Mods.info.source; maxWidth: parent.width; icon: Mods.info.workshop ? "cloud" : "hard-drive"; text: Theme.say(Mods.info.source || "") }
                Chip { visible: !!Mods.info.repository && !Mods.info.workshop; icon: "git-branch"; text: Theme.say("Git") }
                Chip { visible: !!Mods.info.version; maxWidth: parent.width; text: Mods.info.version || "" }
                Chip { visible: !!Mods.info.gameVersions; maxWidth: parent.width; tone: Mods.info.madeFor && !Mods.info.versionOk && !Mods.info.official ? Theme.warn : Theme.ink2; text: Theme.fill("RimWorld {}", (Mods.info.gameVersions || "")); tip: Mods.info.versionOk ? Theme.say("Marked as working on this game version") : "" }
                Chip { visible: !!Mods.info.megabytes; text: Mods.info.megabytes ? Theme.fill("{} MB", (Mods.info.megabytes < 10 ? Mods.info.megabytes.toFixed(1) : Math.round(Mods.info.megabytes))) : "" }
                Chip { visible: !!Mods.info.updated; maxWidth: parent.width; text: Mods.info.updated ? Theme.fill("updated {}", Theme.ago(Mods.info.updated)) : "" }
            }
        }
    }
    Flow {
        id: acts
        readonly property bool narrow: width < toggle.fullWidth + openFolder.fullWidth + workshopPage.fullWidth + 2 * spacing
        width: parent.width
        topPadding: Theme.gapLarge
        spacing: Theme.gapSmall
        Btn { id: toggle; visible: !Hub.mods.previewing; primary: !Mods.selectedActive; icon: Mods.selectedActive ? "minus" : "plus"; text: Mods.selectedActive ? Theme.say("Deactivate") : Theme.say("Activate"); onClicked: Mods.toggleSelected() }
        Btn { id: openFolder; compact: acts.narrow; icon: "folder-open"; text: Theme.say("Open folder"); onClicked: Qt.openUrlExternally(Theme.fileUrl(Mods.info.path)) }
        Btn {
            id: workshopPage
            visible: !!Mods.info.workshop
            compact: acts.narrow
            icon: "external-link"
            text: Theme.say("Workshop page")
            onClicked: Theme.openWorkshop(Mods.info.folder)
        }
        Btn {
            visible: !Mods.info.workshop && !!Mods.info.repository
            icon: "git-branch"
            text: Theme.say("Repository page")
            tip: Mods.info.repository || ""
            onClicked: Qt.openUrlExternally(Mods.info.repository)
        }
    }

    Item { width: 1; height: Theme.gapLarge }
    Choice {
        width: parent.width
        stretch: true
        title: Theme.say("Details")
        options: ["About", "Load order", "Files"]
        current: Mods.tab
        onPicked: index => Mods.tab = index
    }
}
