---
name: worker/web/controls
description: Web controls and markup: overlays that close three ways, buttons in cards and list rows, media with Range.
roles: [worker, reviewer]
tags: [modal, dialog, overlay, sheet, button, buttons, media, audio, video, list, row, card]
---
# Worker: web app (vanilla TypeScript): controls

15. Every overlay (modal, sheet, dialog) closes three ways: a click on the backdrop outside the
    window (only when both pointerdown and pointerup land on the backdrop, so dragging a text
    selection out doesn't close it), a visible × button top-right, and Escape. If a form inside
    has unsaved edits, ask before closing. Return focus to the element that opened it. Use the
    shared helper `core/modal.ts`; never write a one-off.
22. A button inside a card button is invalid HTML (and breaks clicks). Put the second button
    (play) next to the card button in the same `li`, positioned over it.
23. Media served to `<audio>` needs HTTP Range support (Safari will not play without it):
    tower-http `ServeFile` gives it; check with a `Range: bytes=0-99` request expecting 206.
28. A list row that should do something is a `<button>` inside the `<li>` with a `data-action`, never
    a bare `<li>` (the sidebar task lines were dead for that reason). Test the click in Playwright.
