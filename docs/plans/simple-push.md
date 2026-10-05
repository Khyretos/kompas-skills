**Goal:** A regular user installs the Kompanion app, signs in, and gets task notifications without push apps or extra server setup; UnifiedPush via ntfy remains an optional path for technical users.

**Machine / role:** kireserver; Android app (`android/`), server (`server/src/push.rs`, `server/src/api.rs` events stream), install docs.

**Depends on:** Apps 2 part 1 (the app and UnifiedPush push, live on Kees's phone since 2026-10-05).

**Steps**
1. Implement `UnifiedPush.getDistributors()` check in the Android app: if a distributor exists, use today's path; otherwise, establish an own connection.
2. Create a foreground service with `foregroundServiceType="dataSync"` that displays one quiet permanent notification "Kompanion is connected", keeps the server's existing live stream `/api/events` open using the WebView session cookie, reconnects with backoff after network changes, and converts task events "needs you", "failed" and "done" into notifications identical to the push path.
3. Add a toggle in the app's settings to switch the foreground service off.
4. On first start, detect the device maker via `Build.MANUFACTURER`; if it is Xiaomi, Huawei or similar, show a screen with a button that opens the correct settings page (for Xiaomi HyperOS: `com.miui.securitycenter/com.miui.permcenter.autostart.AutoStartManagementActivity`) and instructs the user to set "battery: no restrictions". This screen appears once and can be reopened from settings.
5. Verify the server sends only the signed-in user's events via the existing events stream; no new server code is required.
6. In Kompanion's server compose file (`docker-compose.yml` at the repo root), add ntfy as an optional service under `profiles: [push]`, off by default; turning the profile on also writes `[push] servers` in `kompanion.toml`, so the user edits nothing by hand.
7. Update the README to explain both paths in plain words, presenting the simple path (no ntfy) first.
8. Confirm Sign-in through Keycloak continues to work inside the app as it already does.
9. Write a server test verifying the events stream delivers a task state change, and add a manual check list to `android/README.md` instead of an instrumented Android test.

**Done when:** on a phone without ntfy, a task that needs Kees shows a notification within a minute with the app closed; on Kees's Xiaomi the setup screen opens the Autostart page; with ntfy installed the app still uses UnifiedPush.

**How to test:** uninstall ntfy on a test phone, run the W2 demo task to "needs you"; then reinstall ntfy and repeat.

**Principles:** FOSS only; local models draft, a stronger model reviews and writes lessons; Kreative Kompas palette with bright text (7:1 headers and buttons); every action updates without a reload; English and Spanish at least; one SSO account per person; security first (no open endpoints, grants for every action on a PC); simple for regular users, extras optional for technical users.

_Kees, 2026-10-05. Drafting by the local AI (Coder on kireserver), reviewed by Claude._
