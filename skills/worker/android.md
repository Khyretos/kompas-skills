---
extends: worker/android
---
5. (2026-10-05) Build only with `android/build.sh` (throwaway image, Google's SDK tools never on the
   host). AGP 9.4.1 expects build-tools 36.0.0. The Gradle cache is a folder in the user's home.
