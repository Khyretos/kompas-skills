---
name: worker/android
description: The Kompanion Android app (plain Java WebView app with UnifiedPush) and its build container.
roles: [worker, reviewer]
tags: [android, java, gradle, unifiedpush, apk]
paths: ["android/**"]
---
# Worker: Android app

1. (2026-10-05) Plain Java, no AndroidX: `android.app.Activity`, `android.app.Notification.Builder`,
   `android.app.PendingIntent` (never `android.os.PendingIntent`, never `NotificationCompat`).
2. (2026-10-05) `Context.getString(int)` takes a resource id. Stored values come from
   `getSharedPreferences(name, MODE_PRIVATE).getString(key, null)`.
3. (2026-10-05) Kotlin lambdas from Java (UnifiedPush callbacks) return `kotlin.Unit.INSTANCE`.
   Kotlin functions with default parameters and no `@JvmOverloads` need every argument from Java:
   `UnifiedPush.register(ctx, "default", name, null)`.
4. (2026-10-05) UnifiedPush connector 3.x: extend `PushService` (`onNewEndpoint`, `onMessage`,
   `onRegistrationFailed`, `onUnregistered`), declare it `exported="false"` with the action
   `org.unifiedpush.android.connector.PUSH_EVENT`. Unencrypted pushes arrive with `decrypted == false`.
6. (2026-10-05) Groovy: `versionCode((x) as Integer)`; without the outer parentheses the cast
   applies to the call's result.
7. (2026-10-05) Check every APK with `apksigner verify --print-certs` and `aapt2 dump badging`
   (package, target SDK, permissions) before it goes to a phone.
8. (2026-10-05) Never log push endpoints or cookies; they work like passwords.
9. (2026-10-05) ntfy from F-Droid (1.25.2) does not answer UnifiedPush's default-distributor link,
   so `tryUseCurrentOrDefaultDistributor` calls back `false` although ntfy is installed. Fall back
   to `getDistributors(ctx)` (minus your own package), `saveDistributor`, then `register`.
   Test push on a real phone: the server log shows the phone's API calls; `push_endpoints` shows
   whether it registered.
10. (2026-10-05) Xiaomi HyperOS (MIUI) blocks one app from waking another: logcat shows
    "Unable to launch app io.heckel.ntfy ... not permitted to auto start ... Security_WakePath".
    Both ntfy and the app need Autostart (Settings > Apps > Permissions > Autostart on HyperOS 3;
    adb: `am start -n com.miui.securitycenter/com.miui.permcenter.autostart.AutoStartManagementActivity`;
    check with `appops get <pkg> 10008`). An activity resumed from the background does not run
    `onCreate`, so force-stop and relaunch to retry the registration.
11. (2026-10-05) ntfy creates UnifiedPush topics on its *Default server* (ntfy.sh unless changed).
    The server refuses endpoints on other servers (400 "not on an allowed push server"), which is
    right; tell the user to set ntfy's Default server to their own and delete the old topic.
