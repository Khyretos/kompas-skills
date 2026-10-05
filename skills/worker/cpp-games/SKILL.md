# Worker: kk-engine games (C++)

Source: the racing round (kk-engine PR #3, 2026-10-04): seven cameras, any controller, deeper dents.
Car space in `games/racing`: +Z forward, +X left, yaw + turns right.

1. (2026-10-04) Look inside an asset before designing round it: `strings FILE.fbx | grep -o "SK_Veh_[A-Za-z0-9_]*" | sort -u`
   lists its part names (`kke_model_info` only gives counts). It showed every Synty Street Racer car has a full
   interior (`SteeringW`, `Seats`, `Speedometer_Needle`), so the cockpit camera sits in the real seat instead of a
   drawn dashboard.
2. (2026-10-04) Glass is drawn dark and solid, and there is no per-part hide or transparency for models. To see out
   from inside, collapse the glass parts' vertices to one point with `ModelModule::setDeformedVertices` (degenerate
   triangles draw nothing), and only when the camera mode changes: it is a vertex upload.
3. (2026-10-04) A part's axis from its vertices: the steering column is the direction the wheel's vertices spread
   least (power iteration on the inverse covariance, `Cars.cpp`), flipped to point at the driver.
4. (2026-10-04) `InputModule::setPlayers(1)` deletes maps 1 to 3; new players copy map 0's bindings later. Bind on
   map 0 before `commitDefaults()`; binding the other maps in a loop is wasted.
5. (2026-10-04) Pedals rest at one end of their axis (+1 or -1, not 0). A plain `Binding` has no offset and would
   read a resting pedal as a full brake. Read them in the game: `amount = (v - rest) / (full - rest)`.
6. (2026-10-04) `JoyButton` / `JoyAxis` / `JoyHat` bindings also match gamepads (`InputDevices::deviceMatches`):
   flight-stick defaults fire on a pad's raw buttons too. Check what each raw index is on a pad before binding it.
7. (2026-10-04) GameShellModule opens the pause menu on gamepad and keyboard events only; other devices need the
   `shell.open` action. Menus are driven through the `ui.*` actions: bind a stick's hat and buttons to those.
8. (2026-10-04) A button that does two things needs its own action per meaning. A flight stick's trigger was both
   handbrake and "race again", so pulling it mid-race restarted the race; `race.again.over` only counts once the
   race is over.
9. (2026-10-04) Shared state behind a per-player feature: the TV camera kept one spot for the whole game, so the
   code forced players off it (`KKE_RACE_CAMERA=tv` gave the chase view). Keep such state on the car or player.
10. (2026-10-04) Screenshot every mode, not just the new ones, and compare each with what it must show. The bonnet
    camera had been inside the windscreen frame for weeks, and the TV camera looked like the chase camera; only the
    screenshots showed it.

## Testing (soucouyant)

- `tools/runner/kkrun build` builds without FEMFX. Dents need the FEMFX build: `cmake --preset everything-release
  -DKKE_WARNINGS_AS_ERRORS=ON`, `cmake --build build-release -j4 --target racing`, then
  `tools/check_game racing --bin build-release/bin`. Without it there is no `deepest dent` line in the log.
- Set `KKE_ASSETS_DIR` before every run, or you get block cars and no dashboard.
- `KKE_RACE_CAMERA=<name>` picks the camera, `KKE_RACE_AUTOPILOT=1` drives, `KKE_RACE_BENCH=pileup` crashes cars.
- Real controllers are plugged in on soucouyant (two T.16000M sticks, a GameCube adapter). `KKE_LOBBY_JOIN=1`
  seats the first gamepad, which is the GameCube adapter, not the virtual pad. Hide it:
  `SDL_JOYSTICK_HIDAPI_GAMECUBE=0` (or `SDL_GAMECONTROLLER_IGNORE_DEVICES=0x057e/0x0337`).
- `KKE_VIRTUAL_PAD_SCRIPT` times are game time (`ctx.totalTime`), which runs slower than the wall clock at low fps.
- Two players: `KKE_VIRTUAL_INPUT=pad KKE_LOBBY_JOIN=1`, then player 1 goes down to the Start row in the lobby and
  presses Return. `KKE_RACE_LOBBY=0` skips the lobby and seats only player 1.
- xdotool at about 10 fps misses quick presses: hold keys about 0.4 s. A HUD note lasts 1.2 s: screenshot within it.

## Could a 9B local model do this alone?

Partly. It could write the camera maths for chase, bonnet and TV and the README, with `Cameras.cpp` as the example.
It needs lessons 2, 4, 5, 6 and 7 word for word (facts about this engine that aren't in any public doc), and one
step at a time: cameras, then controllers, then the set-up flow. Judging the screenshots needs a person or a vision
model asked one yes/no question at a time ("Is the steering wheel at the bottom middle?").
