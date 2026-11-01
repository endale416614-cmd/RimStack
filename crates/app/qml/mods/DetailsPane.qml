pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

Rectangle {
    color: Theme.panel

    Scrolled {
        anchors.fill: parent
        padding: Theme.gapCard

        CompareSummary {}
        SortMoves {}
        ProblemsPanel { id: problems }
        SelectionSummary { visible: Mods.selectedCount > 1 && Mods.comparing === null && !problems.visible }
        Column {
            width: parent.width
            visible: Mods.selectedCount === 1 && Mods.comparing === null && !problems.visible && !Hub.mods.previewing
            ModHeader {}
            AboutTab {}
            LoadOrderTab {}
            FilesTab {}
        }
        ListSummary { visible: Mods.selectedId < 0 && Mods.comparing === null && !problems.visible && !Hub.mods.previewing }
    }
}
