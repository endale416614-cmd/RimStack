// Driven by tools/check-app.sh, which points RimStack at a copy of the game's settings.
// Exits with 1 if any check failed.
pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

Item {
    id: checks
    required property ListView view

    Connections {
        target: Hub.mods
        function onSaved(ok) {
            if (!ok) {
                console.log("check FAIL saving: " + Mods.saveProblem)
                Qt.exit(1)
            }
        }
    }

    Connections {
        target: Mods
        function onFirstFrameShown() {
            if (Theme.selfTest)
                selfTestTimer.start()
        }
    }

        Timer {
        interval: 300
        repeat: true
        running: Mods.checkMode !== ""
        onTriggered: {
            if (Mods.active.count === 0 || Hub.mods.rescanning)
                return
            if (Mods.checkMode === "close") {
                stop()
                checks.closeWhileBusy()
                return
            }
            if (!Hub.mods.filesRead)
                return
            stop()
            if (Mods.checkMode === "screens" || Mods.checkMode === "layout") {
                const made = Qt.createComponent("qrc:/qt/qml/RimStack/Ui/qml/checks/" + (Mods.checkMode === "layout" ? "LayoutChecks.qml" : "ScreensChecks.qml"))
                if (made.status !== Component.Ready) {
                    console.log("check FAIL the screen checks did not load: " + made.errorString())
                    Qt.exit(1)
                }
                made.createObject(checks, { window: checks.Window.window })
                return
            }
            Mods.checkMode === "textures" ? checks.checkTextures() : Mods.checkMode === "live" ? checks.checkLive() : checks.runChecks()
        }
    }

    function closeWhileBusy() {
        console.log("check state " + Mods.active.count + " active, " + Mods.errors + " errors, files read: " + Hub.mods.filesRead)
        Hub.mods.rescan()
        Hub.textures.audit()
        checks.Window.window.close()
    }

    property int liveStep: 0
    property int liveWaited: 0
    property int liveTotal: 0
    function checkLive() {
        const folder = Hub.argument("--live-folder")
        liveTotal = Mods.active.count + Mods.inactive.count
        Hub.settings.keep("workshop", folder)
        liveWatch.start()
    }
    Timer {
        id: liveWatch
        interval: 500
        repeat: true
        onTriggered: {
            const total = Mods.active.count + Mods.inactive.count
            const offer = Notices.shown.find(notice => !!notice.run)
            checks.liveWaited += 1
            const late = checks.liveWaited > 50
            if (checks.liveStep === 0 && (late || (!Hub.mods.rescanning && total === checks.liveTotal + 1))) {
                checks.textureCheck("a Workshop folder changed in Settings is read at once", total === checks.liveTotal + 1 && Hub.settings.report.workshop.endsWith("workshop-b"))
                checks.liveStep = 1
                const third = Hub.mods.packageMod(false, "check.third")
                Mods.copyToLocal(third)
            } else if (checks.liveStep === 1 && (late || (!Hub.mods.rescanning && total === checks.liveTotal + 2))) {
                checks.textureCheck("a Workshop mod copied to the game's own Mods folder shows once copied", total === checks.liveTotal + 2 && Mods.upkeepProblem === "")
                checks.liveStep = 2
                console.log("check live-ready")
            } else if (checks.liveStep === 2 && (late || total === checks.liveTotal + 3)) {
                checks.textureCheck("a mod folder added outside shows by itself", total === checks.liveTotal + 3)
                checks.liveStep = 3
            } else if (checks.liveStep === 3 && (late || offer)) {
                checks.textureCheck("a ModsConfig.xml written outside is offered, not taken", !!offer && Mods.active.count === 2)
                if (offer)
                    offer.run()
                checks.liveStep = 4
            } else if (checks.liveStep === 4 && (late || !Hub.mods.saving)) {
                checks.textureCheck("and used when asked for", Mods.active.count === 1 && !Hub.mods.dirty)
                Qt.exit(checks.textureFailures > 0 ? 1 : 0)
            }
        }
    }

    // `--check=textures` renames a file: only ever on the script's made-up mods.
    property int textureFailures: 0
    function textureCheck(what, ok) {
        checks.timed(what)
        console.log("check " + (ok ? "ok   " : "FAIL ") + what)
        if (!ok)
            textureFailures += 1
    }
    function checkTextures() {
        checks.stepBegan = Date.now()
        const a = Mods.nthMod(Mods.active, 0), b = Mods.nthMod(Mods.active, 1)
        Mods.select(a, true)
        textureCheck("two mods that share a texture are in conflict", ((Mods.info.losesTo || [])[0] || {}).modId === b)
        const tools = Hub.textures
        tools.compare(a, b)
        let shared = tools.shared
        textureCheck("the comparison lists the shared texture, kept by load order", shared.length === 1 && !shared[0].keepsEarlier && !shared[0].yourChoice && shared[0].used === "second")
        textureCheck("and names the two mods, the earlier first", tools.comparison.earlier.name === "first" && tools.comparison.later.name === "second")
        const differing = tools.difference(shared[0].at)
        textureCheck("the difference of the two versions is a picture of every pixel", differing.total === 64 && differing.differing === 64 && differing.file.endsWith(".png"))
        textureCheck("keeping the earlier mod's version works", tools.keep(shared[0].at, true) === "")
        shared = tools.shared
        const now = shared[0]
        textureCheck("the comparison then shows your choice", shared.length === 1 && now.keepsEarlier && now.yourChoice && now.used === Theme.fill("{}, chosen", "first") && now.laterShown.endsWith(".rimstack-hidden"))
        const picture = Qt.createQmlObject("import QtQuick; Image { asynchronous: false; cache: false }", checks)
        picture.source = Theme.fileUrl(now.laterShown)
        textureCheck("the hidden picture can still be shown", picture.status === Image.Ready && picture.sourceSize.width > 0)
        picture.destroy()
        textureCheck("the later mod has one hidden texture", tools.hiddenIn(b) === 1 && tools.hiddenIn(a) === 0)
        afterRecheck.start()
    }
    Timer {
        id: afterRecheck
        interval: 1200
        onTriggered: {
            checks.stepBegan = Date.now()
            checks.textureCheck("the selected mod's textures are counted", Mods.textures.count === 1)
            const a = Mods.nthMod(Mods.active, 0), b = Mods.nthMod(Mods.active, 1)
            Mods.select(a, true)
            checks.textureCheck("the conflict is gone once the later file is hidden", (Mods.info.losesTo || []).length === 0)
            const tools = Hub.textures
            checks.textureCheck("unhide all puts it back", tools.unhideMod(b) === "" && tools.hiddenIn(b) === 0)
            tools.compare(a, b)
            const shared = tools.shared
            checks.textureCheck("and the load order decides again", shared.length === 1 && !shared[0].keepsEarlier && !shared[0].yourChoice && shared[0].used === "second")
            Qt.exit(checks.textureFailures > 0 ? 1 : 0)
        }
    }
    // A script runs in one go, so the stall watch cannot tell its steps
    // apart: each is timed from the check before it in the same run.
    property double stepBegan: 0
    function timed(what) {
        const now = Date.now()
        if (checks.stepBegan > 0 && now - checks.stepBegan >= 50)
            console.log("step_ms=" + (now - checks.stepBegan) + " " + what)
        checks.stepBegan = now
        Qt.callLater(checks.runEnded)
    }
    function runEnded() {
        checks.stepBegan = 0
    }

    function runChecks() {
        let failed = 0
        checks.stepBegan = Date.now()
        const check = (what, ok) => {
            checks.timed(what)
            console.log("check " + (ok ? "ok   " : "FAIL ") + what)
            if (!ok)
                failed += 1
        }
        const order = () => { const ids = []; for (let i = 0; i < Mods.active.count; i++) ids.push(Mods.nthMod(Mods.active, i)); return ids.join(",") }
        if (Mods.checkMode === "restore") {
            check("the order chosen before the restart is shown", Mods.inactive.rows() === Mods.inactive.count && Mods.active.rows() === Mods.active.count)
            Hub.mods.moveMod(true, 3, 0)
            Mods.save(() => { check("moved back and saved", !Hub.mods.dirty); Qt.exit(failed > 0 ? 1 : 0) })
            return
        }
        const start = order()
        const counts = [Mods.active.count, Mods.inactive.count]
        const shownNames = () => { const names = []; for (let r = 0; r < Mods.inactive.rows() && names.length < 40; r++) if (Mods.inactive.modIdAt(r) >= 0) names.push(Mods.inactive.at(r).name.toLowerCase()); return names }
        const byName = () => { const names = shownNames(); return names.every((n, i) => i === 0 || names[i - 1].localeCompare(n) <= 0 || names[i - 1] <= n) }
        const wasByName = Mods.prefs.inactiveByName, hadHeadings = Mods.prefs.headings
        Mods.prefs.inactiveByName = true
        check("the inactive mods are in name order", byName() && Mods.inactive.rows() === Mods.inactive.count)
        check("lists are filled and clean", Mods.active.count > 4 && Mods.inactive.count > 0 && !Hub.mods.dirty && !Hub.mods.canUndo)

        // Sort waits for the mods' files to be read; they are not again for a moment
        // after a mod comes or goes.
        Mods.previewSort()
        const moving = Hub.mods.previewing
        check("sort gives a preview or says there is nothing to do", Hub.mods.sortNote !== "")
        if (moving) {
            Mods.endSort(false)
            check("discard restores the order", order() === start && !Hub.mods.previewing && !Hub.mods.dirty)
            Mods.previewSort()
            Mods.endSort(true)
            check("apply keeps the sorted order", order() !== start && Hub.mods.dirty && !Hub.mods.previewing)
            Mods.previewSort()
            check("sorting a sorted list moves nothing", !Hub.mods.previewing)
            check("and says so in the language chosen", Hub.mods.sortNote.startsWith(Theme.say("Already in order.")))
            Hub.mods.sortNote = ""
            Hub.mods.undo()
            check("undo takes the sort back", order() === start)
        }
        const outOfOrder = (Mods.overview.troubles || []).find(t => t.fix.action === "sort")
        if (outOfOrder) {
            Mods.applyFix(outOfOrder.modId, outOfOrder.fix)
            check("Sort on a mod out of its own order gives the preview", Hub.mods.previewing)
            Mods.endSort(false)
            check("and discarding it leaves the order as it was", order() === start && !Hub.mods.dirty)
        }

        Hub.mods.moveMod(true, 0, 3)
        check("a move makes the list dirty", Hub.mods.dirty && Hub.mods.canUndo && order() !== start)
        check("what is unsaved is said in the language chosen", Hub.mods.unsaved === Theme.fill("{} moved", 1))
        Hub.mods.undo()
        check("undo puts it back", !Hub.mods.dirty && order() === start && Hub.mods.canRedo)
        check("and the inactive mods are still in name order", byName() && Mods.inactive.rows() === Mods.inactive.count)
        Mods.prefs.headings = 0
        check("one numbered list has no headings", Mods.active.rows() === Mods.active.count)
        Hub.mods.moveMod(true, 0, 3)
        Hub.mods.undo()
        check("and stays one list after undo", Mods.active.rows() === Mods.active.count)
        Mods.prefs.headings = 1
        check("always under headings has headings", Mods.active.rows() > Mods.active.count)
        Mods.prefs.headings = hadHeadings
        Mods.prefs.inactiveByName = false
        check("by category, the inactive mods sit under headings", Mods.inactive.rows() > Mods.inactive.count)
        Mods.prefs.inactiveByName = true
        Hub.mods.redo()
        check("redo moves it again", Hub.mods.dirty && order() !== start)
        Hub.mods.undo()

        const second = Mods.nthMod(Mods.active, 1)
        Hub.mods.take(true, 1, -1)
        check("deactivating moves a mod across", Mods.active.count === counts[0] - 1 && Mods.inactive.count === counts[1] + 1)
        Hub.mods.take(false, Mods.inactive.indexOf(second), -1)
        check("activating brings it back", Mods.active.count === counts[0] && Mods.active.indexOf(second) >= 0)
        while (Hub.mods.canUndo)
            Hub.mods.undo()
        check("undo all the way is the saved list", !Hub.mods.dirty && order() === start)

        Mods.select(Mods.nthMod(Mods.active, 0), true)
        Mods.selectThrough(Mods.nthMod(Mods.active, 2), true)
        check("shift selects a run", Mods.selectedCount === 3)
        Mods.selectAlso(Mods.nthMod(Mods.active, 1), true)
        check("ctrl takes one out", Mods.selectedCount === 2)
        Mods.moveSelected()
        check("the selected mods move together", Mods.active.count === counts[0] - 2 && Mods.selectedCount === 0)
        Hub.mods.undo()
        check("and come back with one undo", Mods.active.count === counts[0] && order() === start && !Hub.mods.canUndo)

        const warned = [...new Set(Mods.troubles.filter(t => t.fix.action === "trust").map(t => t.modId))].slice(0, 3)
        if (warned.length === 3) {
            Mods.select(warned[0], true)
            Mods.selectAlso(warned[1], true)
            Mods.selectAlso(warned[2], true)
            const offered = Mods.versionWarned.length
            Mods.trustSelected(true)
            const left = Mods.troubles.filter(t => t.fix.action === "trust" && warned.indexOf(t.modId) >= 0).length
            check("ignoring the game version covers every selected mod", offered === 3 && left === 0 && Mods.selectedCount === 3)
            Mods.trustSelected(false)
            check("and warning again brings all three back", Mods.versionWarned.length === 3)
            Mods.select(-1, false)
        }

        const a = Mods.nthMod(Mods.active, 0), b = Mods.nthMod(Mods.active, 1)
        Mods.select(b, true)
        Mods.picking = "before"
        Mods.pick(a)
        check("a rule of your own is added", (Mods.info.yourRules || []).some(rule => rule.first === b && rule.then === a) && Mods.picking === "")
        Hub.mods.removeRule(b, a)
        check("and removed", (Mods.info.yourRules || []).length === 0)
        const worked = Mods.about(b).category
        const byHand = worked === "Textures" ? "Patches" : "Textures"
        Mods.fileUnder(b, byHand)
        check("a mod filed under a category by hand keeps that choice", Mods.about(b).categoryChosen === byHand)
        Mods.fileUnder(b, "")
        check("and goes back to the one worked out for it", Mods.about(b).categoryChosen === "" && Mods.about(b).category === worked)
        Mods.select(-1, false)

        Mods.step(Mods.active, 1)
        check("the down key selects the first mod", Mods.selectedId === Mods.nthMod(Mods.active, 0))
        Mods.step(Mods.active, 1)
        check("and again the second", Mods.selectedId === Mods.nthMod(Mods.active, 1))
        Mods.step(Mods.active, -1)
        Mods.step(Mods.active, -1)
        check("the up key stops at the first", Mods.selectedId === Mods.nthMod(Mods.active, 0))
        const below = Mods.nthMod(Mods.active, 1)
        Mods.activateSelected(Mods.active)
        check("Enter moves the mod to the other list and selects the one below it", Mods.active.count === counts[0] - 1 && Mods.selectedId === below && Mods.selectedActive)
        Hub.mods.undo()
        check("and one undo brings it back", Mods.active.count === counts[0] && order() === start)
        Mods.select(Mods.nthMod(Mods.active, 0), true)
        Mods.nudge(Mods.active, -1)
        check("Alt+Up on the first mod moves nothing", order() === start)
        Mods.nudge(Mods.active, 1)
        check("Alt+Down moves the mod one place down", Mods.active.indexOf(Mods.selectedId) === 1 && order() !== start)
        Hub.mods.undo()
        check("and one undo puts it back", order() === start)
        Mods.select(Mods.nthMod(Mods.active, 0), true)

        const nameOn = r => Mods.active.at(r).name.toLowerCase()
        const orderBefore = order()
        Mods.columns.showBy("name")
        check("a click on Name shows the load order by name", Mods.active.rows() === Mods.active.count && nameOn(0) <= nameOn(1) && nameOn(1) <= nameOn(2) && order() === orderBefore)
        Mods.columns.showBy("name")
        check("a second click turns it round", nameOn(0) >= nameOn(1))
        Mods.columns.showBy("name")
        check("and a third shows the load order again", Mods.columns.shownBy === "" && Mods.active.rows() >= Mods.active.count)
        Mods.columns.toggle("author")
        check("a column can be hidden", Mods.columns.shown.indexOf("author") === -1)
        Mods.columns.toggle("author")
        Mods.columns.move("author", 0)
        check("and put back in its place", Mods.columns.shown[0] === "author")
        const sizesBefore = Mods.prefs.columnSizes
        Mods.columns.size("author", 200 * Theme.grow)
        check("a column keeps the width it was dragged to", Mods.columns.widths.author === Theme.sized(200))
        Mods.columns.size("author", 9999)
        check("within what a column may take", Mods.columns.chosenWidths.author === 420)
        Mods.prefs.columnSizes = sizesBefore
        const listed = Mods.columns.entries
        check("the column menu lists every column, the chosen ones marked", listed.length === 3 && listed[0].text.startsWith("✓"))
        check("and Load says what its times need while none are known", Mods.columns.loadTimes || listed[2].note.length > 0)

        const subRows = () => { let n = 0; for (let r = 0; r < Mods.active.rows(); r++) if (Mods.active.at(r).isSub === true) n += 1; return n }
        const before = subRows()
        Mods.putInSub(Mods.selectedId, "check-sub")
        check("a mod grouped under a new tag sits under its row", subRows() === before + 1 && Mods.info.sub === "check-sub")
        Mods.putInSub(Mods.selectedId, "")
        check("and taken out again", subRows() === before && !Mods.info.sub)
        Mods.putInSub(Mods.selectedId, "check-sub")
        const subRow = () => { for (let r = 0; r < Mods.active.rows(); r++) if (Mods.active.at(r).isSub === true && Mods.active.at(r).name === "check-sub") return r; return -1 }
        const rowsOpen = Mods.active.rows()
        Hub.mods.toggleSub(true, subRow())
        check("a group folds its mod away and keeps its row", Mods.active.rows() === rowsOpen - 1 && subRow() >= 0)
        Hub.mods.toggleSub(true, subRow())
        check("and shows it again", Mods.active.rows() === rowsOpen)
        Mods.removeSub(Mods.active, subRow())
        check("a group taken off leaves its mod in the list", subRows() === before && Mods.active.rows() === rowsOpen - 1 && !Mods.info.sub)
        const tagged = Mods.selectedId
        Hub.mods.keepNote(tagged, "check-tag, other", "")
        check("a mod's first tag is the group it sits under", subRows() === before + 1 && Mods.info.sub === "check-tag")
        Mods.putInSub(tagged, "other")
        check("grouping under another of its tags moves that tag first", Mods.info.sub === "other" && Mods.info.tags === "other, check-tag")
        Hub.mods.keepNote(tagged, "", "")
        check("and without tags it is in no group", subRows() === before && !Mods.info.sub)

        Mods.keepNote("check-tag, second", "a note")
        check("tags and a note are kept", Mods.info.tags === "check-tag, second" && Mods.info.note === "a note" && Mods.active.at(Mods.active.rowOf(0)).tags === "check-tag, second")
        Mods.filtered = "tag:check"
        const shown = () => { let n = 0; for (let r = 0; r < Mods.active.rows(); r++) if (Mods.active.modIdAt(r) >= 0) n += 1; return n }
        check("tag: narrows the list to the tagged mod", shown() === 1)
        Mods.filtered = "is:inactive"
        check("is:inactive leaves the load order out and every inactive mod in", Mods.active.matching === 0 && Mods.inactive.matching === Mods.inactive.count)
        Mods.filtered = "author:zzzz-nobody"
        check("author: with no match shows nothing", Mods.active.rows() === 0)
        Mods.filtered = ""
        Mods.keepNote("", "")
        check("and forgotten", !Mods.info.tags && !Mods.info.note && shown() === counts[0])
        Mods.select(-1, false)

        Mods.prefs.inactiveByName = wasByName
        Hub.mods.moveMod(true, 0, 3)
        const asItWas = order(), every = Mods.active.rows()
        Hub.mods.moveMod(true, 2, 6)
        const changedOrder = order()
        check("what changed since the save is put in words", Hub.mods.unsaved !== "" && Hub.mods.dirty)
        if (Mods.problemCount > 0) {
            Mods.showProblem(true)
            check("a step to a problem says its place among them", Mods.problemPlace >= 1 && Mods.problemPlace <= Mods.problemCount && Mods.selectedId === Mods.problemMod)
        }
        Mods.filtered = "is:changed"
        check("is:changed shows only what changed", Mods.active.rows() >= 1 && Mods.active.rows() < every)
        Mods.filtered = ""
        Hub.mods.discardChanges()
        check("discarding goes back to the saved list, and Undo brings the change back", order() !== changedOrder && Hub.mods.canUndo)
        Hub.mods.undo()
        check("undo after a discard restores the change", order() === changedOrder)
        Hub.mods.undo()
        check("and once more, the list before it", order() === asItWas)
        let undone = false
        Notices.offer("Done", "Undo", () => undone = true)
        Notices.offer("Done again", "Undo", () => undone = true)
        Notices.say("Said")
        check("one offer to undo stands at a time, beside what was said", Notices.shown.length === 2 && Notices.shown[0].text === "Done again")
        Notices.shown[0].run()
        Notices.dismiss(Notices.shown[0].key)
        check("an offer runs and goes", undone && Notices.shown.length === 1 && Notices.earlier.length === 3)
        check("every problem has its words and its fix", Mods.troubles.every(t => t.words !== "" && t.group !== "") && Mods.troubles.length >= Hub.mods.errors)
        Mods.previewSort()
        const sortLines = Hub.mods.sortMoves()
        check("each move of a sort has a reason", !Hub.mods.previewing || sortLines.every(group => group.name !== "" && group.moves.every(m => m.why !== "")))
        Mods.endSort(false)
        Mods.leftOffRestored = true
        const one = Mods.nthMod(Mods.active, 3), other = Mods.nthMod(Mods.active, 7)
        Mods.select(one, true)
        Mods.select(other, true)
        check("the mod remembered for the next start is the one selected last", Mods.prefs.lastMod !== "" && Mods.prefs.lastMod === String(Mods.about(other).packageId).toLowerCase())
        Mods.select(-1, false)
        check("and none once nothing is selected", Mods.prefs.lastMod === "")
        Mods.leftOffRestored = false
        const columnsBefore = Mods.prefs.columns
        check("the chosen columns are read from what is kept", Mods.columns.chosen.join(",") === columnsBefore)
        const search = Hub.search
        search.start()
        check("the search for the broken mod starts with some suspects off", search.active && search.list.length > 0 && search.list.length < search.everything && search.rounds > 0)
        let answers = 0
        while (search.asking && answers < 40)
            search.answer(answers++ % 2 === 0)
        check("answers narrow it to one mod", search.found !== "" || search.stuck !== "")
        const began = search.everything
        const back = search.end(false)
        check("stopped, nothing is left of it and the list it began with is given back", !search.active && back.length === began)
        Mods.select(Mods.nthMod(Mods.active, 20), true)
        Mods.selectAlso(Mods.nthMod(Mods.active, 22), true)
        const pair = Mods.selectedIds.slice(), unmoved = order()
        Mods.selectedToEdge(true)
        const toEdge = order() !== unmoved
        check("selected mods go to the top of their category in one step", toEdge || Mods.active.indexOf(pair[0]) === 20)
        if (toEdge)
            Hub.mods.undo()
        check("which one Undo takes back", order() === unmoved)
        Mods.select(-1, false)
        Mods.prefs.inactiveByName = true
        Mods.prefs.headings = 0
        check("a save is written off the window's thread", Mods.save(() => { check("the changed list is saved", !Hub.mods.dirty && Mods.saveProblem === ""); Qt.exit(failed > 0 ? 1 : 0) }) && Hub.mods.saving)
    }

    property int frames: 0
    property int slowFrames: 0
    property real worstMs: 0

    FrameAnimation {
        id: meter
        running: false
        onTriggered: {
            const ms = frameTime * 1000
            checks.frames += 1
            if (ms > 20)
                checks.slowFrames += 1
            if (ms > checks.worstMs)
                checks.worstMs = ms
        }
    }

    NumberAnimation {
        id: scripted
        target: view
        property: "contentY"
        from: 0
        to: Math.max(0, view.contentHeight - view.height)
        duration: 10000
        onStarted: {
            checks.frames = 0
            checks.slowFrames = 0
            checks.worstMs = 0
            meter.running = true
        }
        onFinished: {
            meter.running = false
            console.log("scroll frames=" + checks.frames + " slow=" + checks.slowFrames + " worst_ms=" + checks.worstMs.toFixed(1))
            if (Theme.selfTest)
                Qt.quit()
        }
    }

    Shortcut { sequence: "F5"; onActivated: scripted.start() }

    Timer {
        id: selfTestTimer
        interval: 300
        onTriggered: {
            const rows = () => "active=" + Mods.active.rows() + " inactive=" + Mods.inactive.rows()
            console.log("selftest start " + rows())
            Hub.mods.toggleCategory(true, 3)
            console.log("selftest collapsed-content " + rows())
            Hub.mods.toggleCategory(true, 3)
            console.log("selftest expanded-content " + rows())
            Mods.filterBy("hygiene")
            console.log("selftest filtered " + rows())
            Mods.filterBy("")
            console.log("selftest cleared " + rows())
            Hub.mods.moveMod(true, 5, 900)
            console.log("selftest moved " + rows())
            Hub.mods.take(false, 0, -1)
            console.log("selftest took-one " + rows())
            Hub.mods.take(false, 0, 1000)
            console.log("selftest took-at " + rows())
            Hub.mods.take(true, 1000, -1)
            console.log("selftest gave-back " + rows())
            scripted.start()
        }
    }
}
