/** One shared way to close overlays (worker/web lesson 15): a click on the backdrop,
 *  the × button (call requestClose from its action), or Escape (call requestClose from the
 *  key handler). Asks first when a form inside has unsaved edits, and gives focus back to
 *  the element that opened it. */
export interface Modal { open(opener?: HTMLElement | null): void; requestClose(): void; readonly dirty: boolean }

export function modal(backdrop: HTMLElement, close: () => void): Modal {
  const openerRef = { current: null as HTMLElement | null }
  let isDirty = false

  // Focus restoration helper
  const restoreFocus = (): void => {
    const el = openerRef.current
    if (el && el.isConnected) {
      el.focus()
      openerRef.current = null
    }
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
  const handleBackdropPointerDown = (ev: PointerEvent): void => {
    const downOnBackdrop = ev.target === backdrop
    if (!downOnBackdrop) {
      return
    }
  }

  const handleBackdropPointerUp = (ev: PointerEvent): void => {
    if (ev.target === backdrop) {
      requestClose()
    }
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
