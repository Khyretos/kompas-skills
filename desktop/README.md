# Kreative Kompanion desktop app (Apps 1)

Tauri 2 (MIT/Apache-2.0). The window loads the Kompanion server's web UI
(`https://kompanion.kreative-kompas.com`), so the app has no second UI to maintain.

- **Tray icon:** Open / Quit; closing the window hides it to the tray.
- **Notifications:** Settings > On this device > "Desktop notifications" in the web app;
  inside this app they go through Tauri's notification plugin, which only the server's
  own pages may use (`src-tauri/capabilities/default.json`).
- **Sign-in:** the same as in the browser (password or Keycloak), kept in the app's own
  cookie store.
- **Build:** CI (`.forgejo/workflows/desktop.yml`) builds an AppImage and a .deb on the
  kireserver runner and attaches them to the run. Locally:
  `cd desktop && npx @tauri-apps/cli@2 build` (needs libwebkit2gtk-4.1-dev and Rust).
- **Install on soucouyant (CachyOS):** make the AppImage executable and start it, or
  convert the .deb with `debtap`; autostart: copy the .desktop file to
  `~/.config/autostart`. Hyprland: `windowrulev2 = float, class:^(kreative-kompanion)$,
  title:^(Open|Save)` for file dialogs if needed.
- **Another server:** change `app.windows[0].url` in `src-tauri/tauri.conf.json` and the
  matching `remote.urls` in the capability, then rebuild.
