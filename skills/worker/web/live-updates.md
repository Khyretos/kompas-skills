---
name: worker/web/live-updates
description: Web views that re-render on live updates: keeping focus, scroll, open state and user edits; redraw keys.
roles: [worker, reviewer]
tags: [render, rerender, live, focus, redraw, pane, search, refresh, state, store]
paths: ["web/src/views/**", "web/src/state.ts", "web/src/main.ts"]
---
# Worker: web app (vanilla TypeScript): live updates

3. (2026-10-03) The settings sheet and the task editor must not re-render on unrelated store changes (live machine stats arrive every second). Add new state to the right pane's key list in `render()`.
24. (2026-10-04) Never keep UI state ("this step is open") from the native `toggle` event: it fires
    as a later task, so a re-render that lands between the click and the event replaces the
    `<details>` and the choice is lost (pcagent flake, 1 in ~30 runs). Toggle through an
    `onAction` handler on the `<summary>` that records the state first and then sets `d.open`.
    Prove a flake fix with `npx playwright test <file> --repeat-each=30`, never with retries or skips.
26. (2026-10-04) In `main.ts`, `wire(shell)` gets the outer root, not `.shell`. An event for code that
    listens on `.shell` (resize.ts) must be dispatched on `$(".shell")`: events bubble up, never down.
    (Opening a task didn't re-open a collapsed panel; the Playwright test caught it.)
27. A field inside a pane that re-renders while you type (a search box) needs its value in the store
    on every keystroke, and `remount` keeps the caret (it saves selectionStart/End). Debounce only
    the request, and drop answers for an older query.
30. (2026-10-05) Remember UI state by the identity of the thing itself, never by its current
    container: step groups were keyed by message id, and the steps moved to the reply message when
    it arrived, so the user's open/closed choice was lost. Key by the first step's id.
31. (2026-10-05) Search-as-you-type: number every query and drop answers that are not the newest;
    clear the old list only when the new answer arrives (clearing on each keystroke flickers); keep
    the shown results in an array and open `shown[index]` (Qwen read a `data-result` it never set).
    Text from the user or the server goes in with textContent, never innerHTML, also in "Nothing
    found for ..." lines. Static markup too: the app enforces Trusted Types, so `el.innerHTML = ...`
    throws and the overlay never opened (all five Playwright tests failed in CI). Build overlays
    with createElement/append, or the `html` template and `mount`.
32. (2026-10-05) A live re-render between mousedown and mouseup swallows the click: the press
    landed on an element that is gone, so the browser sends no click. On a busy CI machine this
    made "close the step group" and the voice button fail now and then; real users lose clicks the
    same way while steps run. `onAction` now runs the action on pointerup when the pressed element
    was replaced by its twin (same action, id, data-id). Measure "held down" from pointerdown to
    pointerup, never to when the click handler runs (jank adds to it), and let Stop wait for a
    microphone that is still opening, or the recording comes back empty.
33. (2026-10-05) A pane that re-renders on live updates must keep what the user typed: the Run
    form's Folder field was emptied by a task progress tick, so Start did nothing. `remount` keeps
    fields marked `data-edited` (set on input/change, cleared on submit) and refocuses a field by
    form + name when it has no id. When the pointerup fallback ran an action, skip the click
    Chrome may still send to the twin, or a toggle runs twice and cancels out.
35. (2026-10-05) A list that other views depend on must stay current where it is used: the Run
    form read the computer list, which was only refreshed on the Machines tab, so a list loaded
    during a server restart said "offline" until a reload. Poll it lightly everywhere and
    re-render only when the part that matters (who is online) changes.
40. (2026-10-05) Step groups in the chat fold themselves shut when all steps are done, so
    `waitFor()` (visible) times out on finished steps. Wait with `{ state: "attached" }`, then set
    `details.open = true` in `page.evaluate` before the screenshot.
48. (2026-10-05) New state is drawn only when its key is in the redraw list of the pane that shows it
    (`rightKeys` in main.ts: tasks, access, activity, machines). A value shown on Activity but listed
    under Tasks appears only when it happens to load before the first render.
