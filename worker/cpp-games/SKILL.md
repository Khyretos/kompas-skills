---
name: worker/cpp-games
description: kk-engine games in C++: cameras, controllers, assets, tests.
roles: [worker, reviewer]
tags: [cpp, game, engine]
paths: ["**/*.cpp", "**/*.hpp", "**/*.h", "**/CMakeLists.txt"]
---
# Worker: kk-engine games (C++)

Source: the racing round (kk-engine PR #3, 2026-10-04): seven cameras, any controller, deeper dents.
Car space in `games/racing`: +Z forward, +X left, yaw + turns right.

1. (2026-10-04) Look inside an asset before designing round it: `strings FILE.fbx | grep -o "SK_Veh_[A-Za-z0-9_]*" | sort -u`
   lists its part names (`kke_model_info` only gives counts). It showed every Synty Street Racer car has a full
   interior (`SteeringW`, `Seats`, `Speedometer_Needle`), so the cockpit camera sits in the real seat instead of a
   drawn dashboard.
10. (2026-10-04) Screenshot every mode, not just the new ones, and compare each with what it must show. The bonnet
    camera had been inside the windscreen frame for weeks, and the TV camera looked like the chase camera; only the
    screenshots showed it.

## Testing

- xdotool at about 10 fps misses quick presses: hold keys about 0.4 s. A HUD note lasts 1.2 s: screenshot within it.
