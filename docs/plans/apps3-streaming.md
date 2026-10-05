# Apps 3: game streaming and remote control, soucouyant → Kees's Android phone

Research for Kompanion task "Apps 3 (B)" (2026-10-05). Nothing is installed until Kees
picks; installing on soucouyant goes through the soucouyant thread, the phone is his hands.

## soucouyant in short
CachyOS, **Hyprland** (wlroots-family Wayland compositor), AMD RX 9070 XT (RDNA4: VAAPI
HEVC/AV1 encode), gaming and development desktop. Kees's rule: FOSS only.

## Hosts (on soucouyant)

| Host | Licence | Linux status | Virtual display on Linux | Fits Hyprland? | Notes |
|---|---|---|---|---|---|
| **Sunshine** (LizardByte) | GPL-3.0 | Mature, packaged for Arch/CachyOS | No built-in one; Hyprland can make one itself (`hyprctl output create headless`) | Yes: wlr-screencopy / KMS capture, VAAPI | The baseline, most users, best docs |
| Apollo (ClassicOldSong) | GPL-3.0 | Works, but its headline feature is Windows-only | **Not on Linux yet** (planned) | Partly | Pairs best with Artemis (auto resolution, per-client permissions) |
| Apollo-Linux (community fork) | GPL-3.0 | Small fork | EVDI kernel module | Unclear | Few users, kernel module to maintain |
| Vibepollo (Nonary) | not stated in README | **Beta**, targets Arch/CachyOS with **KDE Plasma 6** | Own kernel driver | Not mentioned (Plasma only) | Says ~99 % AI-generated; licence unclear: not for now |

## Clients (on the phone)

| Client | Licence | Notes |
|---|---|---|
| **Artemis** (Moonlight fork for Android) | GPL-3.0 | Custom resolutions and bitrates, several mouse modes (touchpad, multi-touch), virtual gamepad skins, external-monitor mode, lower MediaTek decode latency merged in; works with Sunshine, extra tricks with Apollo |
| Moonlight Android | GPL-3.0 | Upstream, stable, fewer features |

Android adds 1-3 frames of display latency on any client; the decoder fix in Artemis
matters most on MediaTek phones.

## Recommendation
**Sunshine on soucouyant + Artemis on the phone**, with a Hyprland headless output as the
virtual display (phone resolution, monitors untouched, removed when the stream ends).
Revisit Apollo when its Linux virtual display ships, and Vibepollo when it leaves beta,
supports wlroots/Hyprland and states a licence.

## Setup plan (after Kees picks)
1. soucouyant (soucouyant thread): install `sunshine` from the CachyOS/Arch repo; capture
   method wlr (Hyprland) with KMS as fallback; encoder VAAPI (HEVC, AV1 if the phone decodes
   it); firewall: Sunshine ports only from the LAN and WireGuard; web UI behind localhost.
2. Virtual display: a Sunshine "prep" command per app that runs
   `hyprctl output create headless KK-STREAM` and sets it to the phone's resolution and
   refresh, moves the game there, and removes it on stream end.
3. Apps in Sunshine: "Desktop" (remote control of the normal desktop) and Steam Big Picture.
4. Phone (Kees): Artemis APK from its GitHub releases, pair with the PIN, HEVC, bitrate
   20-40 Mbit/s on Wi-Fi, gamepad mapping.
5. Measure: on-screen timer photo for latency, dropped frames and bitrate at 1080p60 and
   at the phone's native resolution, over Wi-Fi and WireGuard; write the numbers here.

Sources: github.com/ClassicOldSong/Apollo (Linux virtual display discussion #1516),
github.com/Nonary/Vibepollo (README), tech-insider.org "Apollo vs Sunshine 2026",
github.com/alonsojr1980/moonlight-android-turbo (decoder latency, merged into Artemis),
github.com/ImStillBlue/sunshine-virtual-display (headless display for Sunshine on CachyOS).
