/** One shared way to close overlays (worker/web lesson 15): a click on the backdrop,
 *  the × button (call requestClose from its action), or Escape (call requestClose from the
 *  key handler). Asks first when a form inside has unsaved edits, and gives focus back to
 *  the element that opened it. */
export interface Modal { open(opener?: HTMLElement | null): void; requestClose(): void; readonly dirty: boolean }

export function modal(backdrop: HTMLElement, close: () => void): Modal {
  const openerRef = { current: null as HTMLElement | null }
  let isDirty = false

  // Focus restoration helper
  // The opener may have been re-rendered while the overlay was open: then focus
  // the element that replaced it (same data-action).
  const restoreFocus = (): void => {
    const el = openerRef.current
    openerRef.current = null
    if (!el) return
    const action = el.dataset.action
    const target = el.isConnected ? el : action ? document.querySelector<HTMLElement>(`[data-action="${CSS.escape(action)}"]`) : null
    target?.focus()
  }

  // Dirty state management
  const setDirty = (val: boolean): void => {
    isDirty = val
  }

  // Input tracking: inputs inside forms make it dirty, others don't
  const handleInput = (ev: Event): void => {
    const target = ev.target as Element
    if (target.closest("form")) {
      setDirty(true)
    }
  }

  // Submit tracking: resets dirty flag
  const handleSubmit = (): void => {
    setDirty(false)
  }

  // Close logic
  const requestClose = (): void => {
    if (isDirty && !confirm("Close without saving your changes?")) {
      return
    }
    setDirty(false)
    close()
    restoreFocus()
  }

  // Backdrop click handling (pointerdown/pointerup sequence)
  // Close only when the press AND the release both land on the backdrop, so
  // dragging a text selection out of the window doesn't close it.
  let downOnBackdrop = false
  const handleBackdropPointerDown = (ev: PointerEvent): void => {
    downOnBackdrop = ev.target === backdrop
  }

  const handleBackdropPointerUp = (ev: PointerEvent): void => {
    const close = downOnBackdrop && ev.target === backdrop
    downOnBackdrop = false
    if (close) requestClose()
  }

  // Form event listeners using delegation on the backdrop
  backdrop.addEventListener("input", handleInput)
  backdrop.addEventListener("submit", handleSubmit)

  // Global event listeners for backdrop pointer events
  backdrop.addEventListener("pointerdown", handleBackdropPointerDown)
  backdrop.addEventListener("pointerup", handleBackdropPointerUp)

  // Open logic
  const open = (opener?: HTMLElement | null): void => {
    openerRef.current = opener || (document.activeElement instanceof HTMLElement ? document.activeElement : null)
    setDirty(false)
  }

  return {
    open,
    requestClose,
    get dirty() {
      return isDirty
    },
  }
}
