pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

Item {
    id: checks
    required property var window
    property int failed: 0

    readonly property var sizes: [[1440, 860], []]
    readonly property var scales: [1.6, 1.3, 1, 0.85]
    readonly property var popups: ["keys", "search", "lists", "history", "tray", "play", "view", "columns", "ask"]

    readonly property var scenes: {
        const all = []
        const add = (size, scale, what, where) => all.push(Object.assign({ size, scale, section: 0, tab: 0, popup: "", label: where + " at " + scale + " in " + (size.length > 0 ? size.join("x") : "the smallest window") }, what))
        for (const size of sizes)
            for (const scale of scales) {
                for (const section of [0, 1, 2, 3, 4, 5])
                    add(size, scale, { section }, "section " + section)
                for (const tab of [1, 2])
                    add(size, scale, { tab }, "section 0 tab " + tab)
                for (const popup of popups)
                    add(size, scale, { popup }, "the " + popup + " sheet")
                for (const first of [0, 1, 2, 3, 4])
                    add(size, scale, { first }, "first run step " + first)
            }
        const only = Hub.argument("--only")
        return all.filter(scene => scene.label.indexOf(only) >= 0)
    }
    property int at: 0
    property int settled: 0
    property bool grabbing: false

    function busy() {
        return Hub.mods.rescanning || Hub.saves.busy || Hub.logs.busy || Hub.textures.busy
    }

    function shownSize() {
        return checks.window.width + "x" + checks.window.height
    }

    function painted(text) {
        const w = text.contentWidth
        const h = text.contentHeight
        const across = text.effectiveHorizontalAlignment
        const down = text.verticalAlignment
        const dx = across === Text.AlignRight ? text.width - w : across === Text.AlignHCenter ? (text.width - w) / 2 : 0
        const dy = down === Text.AlignBottom ? text.height - h : down === Text.AlignVCenter ? (text.height - h) / 2 : 0
        const whole = text.mapToItem(null, dx, dy, w, h)
        let seen = Qt.rect(whole.x, whole.y, whole.width, whole.height)
        let holder = null
        for (let up = text.parent; up; up = up.parent)
            if (up.clip) {
                seen = cut(seen, up.mapToItem(null, 0, 0, up.width, up.height))
                holder = holder || up
            }
        return { text, whole, seen, holder }
    }

    // A scrolling view may hide what is scrolled away, along the way it scrolls.
    function spills(one, bounds) {
        const h = one.holder
        const room = h ? h.mapToItem(null, 0, 0, h.width, h.height) : bounds
        const down = !!h && h.contentHeight !== undefined
        const across = !!h && h.contentWidth !== undefined && h.contentWidth > h.width + slack
        const x = across ? one.whole.x : room.x - slack
        const y = down ? one.whole.y : room.y - slack
        const inside = cut(one.whole, Qt.rect(x, y, across ? one.whole.width : room.width + 2 * slack, down ? one.whole.height : room.height + 2 * slack))
        return inside.width + slack < one.whole.width || inside.height + slack < one.whole.height
    }
    readonly property real slack: 1

    function cut(a, b) {
        const x = Math.max(a.x, b.x)
        const y = Math.max(a.y, b.y)
        return Qt.rect(x, y, Math.max(0, Math.min(a.x + a.width, b.x + b.width) - x), Math.max(0, Math.min(a.y + a.height, b.y + b.height) - y))
    }

    function texts(root, into) {
        for (const child of root.children || []) {
            if (!child.visible || child.opacity < 0.05)
                continue
            if (child.elide !== undefined && child.truncated !== undefined && child.text !== "")
                into.push(child)
            texts(child, into)
        }
        return into
    }

    function look(root, bounds) {
        const problems = []
        const found = texts(root, []).map(text => checks.painted(text)).filter(one => one.seen.width > 1 && one.seen.height > 1)
        for (const one of found) {
            const t = one.text
            if (t.elide === Text.ElideNone && t.contentWidth > t.width + slack)
                problems.push("\"" + t.text + "\" is " + Math.round(t.contentWidth) + " wide in a box of " + Math.round(t.width))
            if (spills(one, bounds))
                problems.push("\"" + t.text + "\" runs past the edge")
        }
        found.sort((a, b) => a.seen.y - b.seen.y)
        for (let i = 0; i < found.length; i++)
            for (let j = i + 1; j < found.length && found[j].seen.y < found[i].seen.y + found[i].seen.height; j++) {
                const both = cut(found[i].seen, found[j].seen)
                if (both.width > 2 && both.height > 2)
                    problems.push("\"" + found[i].text.text + "\" over \"" + found[j].text.text + "\"")
            }
        return problems
    }

    function scan(scene) {
        const popup = scene.popup !== "" ? checks.window.openable[scene.popup] : null
        const root = scene.first !== undefined ? checks.welcome(checks.window.contentItem) : popup ? popup.contentItem.parent : checks.window.contentItem
        const problems = look(root, root.mapToItem(null, 0, 0, root.width, root.height))
        const wanted = Hub.argument("--picture")
        if (wanted !== "" && scene.label.startsWith(wanted))
            checks.grabbing = (root === checks.window.contentItem ? root.children[0] : root).grabToImage(result => {
                result.saveToFile(Hub.argument("--picture-to"))
                checks.grabbing = false
            })
        console.log("check " + (problems.length === 0 ? "ok   " : "FAIL ") + "nothing overlaps or spills in " + scene.label + ", shown at " + shownSize())
        for (const problem of problems.slice(0, 8))
            console.log("check   " + problem)
        if (problems.length > 0)
            failed += 1
    }

    function arrange(scene) {
        Theme.stored.textScale = scene.scale
        checks.window.width = scene.size[0] || checks.window.smallest.width
        checks.window.height = scene.size[1] || checks.window.smallest.height
        checks.window.section = scene.section
        Mods.tab = scene.tab
        checks.window.firstRun = scene.first !== undefined
        const popup = scene.popup !== "" ? checks.window.openable[scene.popup] : null
        if (scene.popup === "ask")
            Notices.ask({ title: "Set the mods' settings aside", lines: ["Every Mod_*.xml is moved out of", { path: "/home/example/.config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios/Config" }, "Mods start from their defaults. Moving the files back restores them."], label: "Set aside", run: () => {} })
        else if (scene.popup === "columns")
            popup.openAt(checks.window.contentItem, 0, 0, Mods.columns.entries)
        else if (popup)
            popup.open()
    }

    function welcome(item) {
        for (const child of item.children) {
            if (child.steps !== undefined)
                return child
            const found = welcome(child)
            if (found)
                return found
        }
        return null
    }

    function leave(scene) {
        const popup = scene.popup !== "" ? checks.window.openable[scene.popup] : null
        if (popup)
            popup.close()
    }

    Component.onCompleted: {
        Theme.stored.reduceMotion = true
        if (Mods.selectedId === "")
            Mods.select(Mods.nthMod(Mods.active, 0), false)
        arrange(scenes[0])
    }

    function finish() {
        Theme.stored.textScale = 1
        Theme.stored.reduceMotion = false
        Qt.exit(checks.failed > 0 ? 1 : 0)
    }

    Timer {
        interval: 150
        repeat: true
        running: true
        onTriggered: {
            checks.settled += 1
            if (checks.at === checks.scenes.length) {
                if (!checks.grabbing) {
                    stop()
                    checks.finish()
                }
                return
            }
            if (checks.settled < 3 || (checks.busy() && checks.settled < 60))
                return
            const scene = checks.scenes[checks.at]
            const first = scene.first !== undefined ? checks.welcome(checks.window.contentItem) : null
            if (first && first.step !== scene.first) {
                first.step = scene.first
                checks.settled = 0
                return
            }
            checks.scan(scene)
            checks.leave(scene)
            checks.at += 1
            checks.settled = 0
            if (checks.at < checks.scenes.length)
                checks.arrange(checks.scenes[checks.at])
        }
    }
}
