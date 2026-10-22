// Opened by address only when asked for, so RimStack itself never loads QtTest.
pragma ComponentBehavior: Bound
import QtQuick
import QtTest
import RimStack.Ui

Item {
    id: checks
    property ModsScreen screen
    property ListView activeList
    property ListView inactiveList
    property Item details
    property int failed: 0

    function check(what, ok) {
        console.log("check " + (ok ? "ok   " : "FAIL ") + what)
        if (!ok)
            failed += 1
    }

    function divider() {
        const wide = details.width, edge = details.mapToItem(screen, details.width, 200)
        events.mouseMove(screen, edge.x + 1, edge.y, -1, Qt.NoButton, Qt.NoModifier)
        check("the pointer over the divider moves nothing", details.width === wide)
        events.mousePress(screen, edge.x + 1, edge.y, Qt.LeftButton, Qt.NoModifier, -1)
        for (let i = 1; i <= 6; i++)
            events.mouseMove(screen, edge.x + 1 + 10 * i, edge.y, 10, Qt.LeftButton, Qt.NoModifier)
        events.mouseRelease(screen, edge.x + 61, edge.y, Qt.LeftButton, Qt.NoModifier, 10)
        check("dragging the divider sixty pixels widens the details by as much", Math.abs(details.width - wide - 60) <= 2)
        Mods.prefs.details = wide
    }

    function dueHint(item) {
        for (const child of item.data) {
            // Shown only: a hint whose button is folded away (a narrow load order) is skipped.
            if (child.due === true && child.step > 0 && child.visible)
                return child
            const found = child.data ? checks.dueHint(child) : null
            if (found)
                return found
        }
        return null
    }

    function hints() {
        const selected = Mods.selectedId
        let shown = 0, below = true
        Mods.hintStep = 1
        for (let hint = checks.dueHint(screen); hint && shown < 3; hint = checks.dueHint(screen)) {
            const top = hint.background.mapToItem(null, 0, 0), under = hint.parent.mapToItem(null, 0, hint.parent.height)
            below = below && Math.abs(top.y - under.y - 9) < 1 && hint.opened
            const row = hint.contentItem.children[1], next = row.children[row.children.length - 1]
            events.mouseClick(next, next.width / 2, next.height / 2, Qt.LeftButton, Qt.NoModifier, -1)
            if (checks.dueHint(screen) === hint)
                break
            shown += 1
        }
        check("Next and Done on the first-run hints go through each and close the last", shown > 0 && Mods.hintStep === 0)
        check("and reach nothing under the hints", Mods.selectedId === selected)
        check("each hint shows just below its button", below)
        Mods.endHints()
    }

    function suggestions() {
        const away = () => events.mouseClick(details, details.width / 2, details.height - 8, Qt.LeftButton, Qt.NoModifier, -1)
        Mods.filterFocusRequested()
        const filter = screen.Window.activeFocusItem.parent
        const opened = filter.suggesting
        away()
        check("a click away from the filter closes its suggestions", opened && !filter.suggesting && filter.editing)
        events.keyClick(Qt.Key_I, Qt.NoModifier, -1)
        check("and typing opens them again", filter.suggesting)
        const selected = Mods.selectedId
        events.mouseClick(filter, 40, filter.height + 4 + 6 + 13, Qt.LeftButton, Qt.NoModifier, -1)
        check("a click on a suggestion takes it and reaches nothing under it", Mods.filtered.startsWith("is:") && Mods.selectedId === selected)
        events.keyClick(Qt.Key_Escape, Qt.NoModifier, -1)
        check("Esc empties the filter and closes its suggestions", !filter.suggesting && filter.editing && Mods.filtered === "")
        events.keyClick(Qt.Key_I, Qt.NoModifier, -1)
        away()
        events.keyClick(Qt.Key_Escape, Qt.NoModifier, -1)
        check("and empties it with them closed, ahead of the screen's own Esc", Mods.filtered === "")
    }

    function nameAt(row) {
        return Mods.active.at(row).name
    }

    function drag(from, fromY, to, toY) {
        from.parent.ensurePolished()
        to.parent.ensurePolished()
        from.forceLayout()
        to.forceLayout()
        const a = from.mapToItem(screen, 120, fromY), b = to.mapToItem(screen, 120, toY)
        events.mousePress(from, 120, fromY, Qt.LeftButton, Qt.NoModifier, -1)
        for (let i = 1; i <= 12; i++)
            events.mouseMove(screen, a.x + (b.x - a.x) * i / 12, a.y + (b.y - a.y) * i / 12, 20, Qt.LeftButton, Qt.NoModifier)
        events.mouseRelease(screen, b.x, b.y, Qt.LeftButton, Qt.NoModifier, 20)
    }

    Choice {
        id: probe
        property int at: 0
        visible: false
        options: ["One", "Two", "Three"]
        current: at
        onPicked: index => at = index
    }
    Item {
        width: 200
        height: 90
        RowKeys {
            id: rows
            property int at: 0
            property int presses: 0
            visible: false
            count: 3
            current: at
            onChosen: index => at = index
            onEntered: presses += 1
        }
    }
    function keys() {
        if (Mods.compareView)
            events.keyClick(Qt.Key_Escape, Qt.NoModifier, -1)
        if (screen.menuOpen)
            events.keyClick(Qt.Key_Escape, Qt.NoModifier, -1)
        probe.visible = true
        probe.forceActiveFocus()
        events.keyClick(Qt.Key_Right, Qt.NoModifier, -1)
        events.keyClick(Qt.Key_Right, Qt.NoModifier, -1)
        events.keyClick(Qt.Key_Right, Qt.NoModifier, -1)
        events.keyClick(Qt.Key_Left, Qt.NoModifier, -1)
        check("the arrow keys move a choice and stop at its ends", probe.at === 1)
        probe.visible = false
        rows.visible = true
        rows.forceActiveFocus()
        for (const key of [Qt.Key_Down, Qt.Key_Down, Qt.Key_Down, Qt.Key_Up])
            events.keyClick(key, Qt.NoModifier, -1)
        const stepped = rows.at === 1
        events.keyClick(Qt.Key_End, Qt.NoModifier, -1)
        const last = rows.at === 2
        events.keyClick(Qt.Key_Home, Qt.NoModifier, -1)
        events.keyClick(Qt.Key_Return, Qt.NoModifier, -1)
        check("a list of rows takes Up, Down, Home, End and Enter", stepped && last && rows.at === 0 && rows.presses === 1)
        rows.visible = false
        events.keyClick(Qt.Key_2, Qt.ControlModifier, -1)
        const away = !screen.visible
        events.keyClick(Qt.Key_1, Qt.ControlModifier, -1)
        check("Ctrl with a figure goes to that section and back", away && screen.visible)
    }

    TestEvent { id: events }
    Timer {
        interval: 300
        repeat: true
        running: true
        onTriggered: {
            if (Mods.active.count === 0 || !Hub.mods.filesRead)
                return
            stop()
            checks.hints()
            checks.divider()
            checks.suggestions()
            // `row` below assumes compact rows, 26 high.
            Mods.prefs.headings = 0
            Theme.stored.comfortable = false
            const row = y => 4 + 13 + 26 * y
            const moved = checks.nameAt(8), count = Mods.active.count

            checks.drag(checks.activeList, row(8), checks.activeList, row(12))
            checks.check("a mod dragged down the load order lands where it was dropped", checks.nameAt(12) === moved && checks.nameAt(8) !== moved)
            checks.check("and the list has unsaved changes", Hub.mods.dirty && Mods.active.count === count)

            const taken = Mods.inactive.at(2).name
            checks.drag(checks.inactiveList, 13 + 26 * 2, checks.activeList, row(5))
            checks.check("a mod dragged from the inactive list is activated where it was dropped", Mods.active.count === count + 1 && checks.nameAt(5) === taken)

            events.mouseClick(checks.activeList, 120, row(3), Qt.RightButton, Qt.NoModifier, -1)
            checks.check("the right mouse button selects the mod and opens its menu", Mods.info.name === checks.nameAt(3) && checks.screen.menuOpen)

            const before = checks.inactiveList.contentY
            events.mouseWheel(checks.inactiveList, 120, 200, Qt.NoButton, Qt.NoModifier, 0, -120, -1)
            wheelDone.before = before
            wheelDone.start()
        }
    }
    Timer {
        id: wheelDone
        property real before
        interval: 400
        onTriggered: {
            const moved = checks.inactiveList.contentY - before
            checks.check("one notch of the wheel scrolls the system's lines, not more", Math.abs(moved - Application.styleHints.wheelScrollLines * 40) < 2)
            comparing.start()
        }
    }
    Timer {
        id: comparing
        property int tries: 0
        interval: 500
        repeat: true
        onTriggered: {
            tries += 1
            if (Hub.mods.conflicts === 0 && tries < 20)
                return
            stop()
            seeking.start()
        }
    }
    Timer {
        id: seeking
        property int at: 0
        interval: 1
        repeat: true
        onTriggered: {
            if (at < Mods.active.count && Mods.comparing === null) {
                Mods.select(Mods.nthMod(Mods.active, at++), true)
                const other = (Mods.info.losesTo || []).concat(Mods.info.winsOver || [])[0]
                if (other)
                    Mods.compareWith(other.modId)
                return
            }
            stop()
            if (Mods.comparing === null) {
                checks.keys()
                Qt.exit(checks.failed > 0 ? 1 : 0)
            } else
                zooming.start()
        }
    }
    Timer {
        id: zooming
        interval: 600
        onTriggered: {
            const view = Mods.compareView, choice = view.zoomChoice
            const saved = !Hub.mods.dirty
            events.mouseClick(choice, choice.width - 16, choice.height / 2, Qt.LeftButton, Qt.NoModifier, -1)
            checks.check("a click on the last zoom makes the picture eight times its fitted size", view.zoom === 8)
            checks.check("and reaches nothing under the comparison", !Hub.mods.dirty === saved && !Hub.mods.previewing)
            const before = view.chosen
            events.keyClick(Qt.Key_Down, Qt.NoModifier, -1)
            checks.check("the down key shows the next shared texture", view.textures.length < 2 || view.chosen === before + 1)
            checks.keys()
            Qt.exit(checks.failed > 0 ? 1 : 0)
        }
    }
}
