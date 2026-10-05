export type TaskLike = { id: string; title: string; state: string };

/** Per device: off unless the user switched it on in Settings (localStorage "kk-desk-notify" = "1"). */
export function enabled(): boolean {
  try {
    const val = localStorage.getItem("kk-desk-notify");
    return val === "1";
  } catch {
    return false;
  }
}

/** Remembers the choice on this device; storage can be blocked (private window). */
function store(v: "0" | "1"): void {
  try { localStorage.setItem("kk-desk-notify", v); } catch { /* not remembered */ }
}

export async function enable(on: boolean): Promise<"on" | "off" | "denied"> {
  if (!on) {
    store("0");
    return "off";
  }

  // Tauri path
  if ((window as any).__TAURI__?.notification) {
    const granted = await (window as any).__TAURI__.notification.isPermissionGranted();
    if (!granted) {
      await (window as any).__TAURI__.notification.requestPermission();
      const nowGranted = await (window as any).__TAURI__.notification.isPermissionGranted();
      if (nowGranted) {
        store("1");
        return "on";
      }
      return "denied";
    }
    store("1");
    return "on";
  }

  // Browser path
  if (!("Notification" in window)) {
    return "denied";
  }

  const p = await Notification.requestPermission();
  if (p === "granted") {
    store("1");
    return "on";
  }
  store("0");
  return "denied";
}

/** What to say for a state change, or null when it is not worth a notification. */
export function messageFor(prev: TaskLike | undefined, next: TaskLike): string | null {
  if (prev === undefined || prev.state === next.state) {
    return null;
  }
  switch (next.state) {
    case "needs_input":
      return "Needs you";
    case "failed":
      return "Failed";
    case "done":
      return "Done";
    default:
      return null;
  }
}

/** Called for every task update from the server. */
export function onTaskUpdate(
  prev: TaskLike | undefined,
  next: TaskLike,
  open: (taskId: string) => void
): void {
  try {
    if (!enabled()) {
      return;
    }

    const msg = messageFor(prev, next);
    if (msg === null) {
      return;
    }

    if (document.hasFocus()) {
      return;
    }

    const title = "Kompanion: " + next.title.slice(0, 80);
    const body = msg;

    // Tauri path
    if ((window as any).__TAURI__?.notification) {
      (window as any).__TAURI__.notification.sendNotification({ title, body });
      return;
    }

    // Browser path
    if (Notification.permission !== "granted") {
      return;
    }

    const n = new Notification(title, {
      body,
      tag: "kk-task-" + next.id,
      icon: "/api/mail-logo.png", // public PNG of the logo
    });

    n.onclick = () => {
      window.focus();
      open(next.id);
      n.close();
    };
  } catch {
    // Notifications must never break the app
  }
}
