# 0012 Touchpad flings follow Chrome on Linux

Status: accepted, 2026-11-17.

The user asked for a full Chrome-like inertial scroll on the touchpad: the throw when the
fingers lift, fingers landing to stop it, and Chrome's own behaviour as the spec. omarky threw
the page on the older Chromium curve, `dv/dt = -3.7 (v + 172)` from `ui/events/gestures/fling_curve.cc`
(RD-27). Before this change a swipe on Linux stopped dead when the fingers lifted. Rune
coasted only on momentum the platform sends, which macOS does and Linux doesn't
(`decisions.tsv`, phase 10 A).

Read from Chromium's sources on 2026-11-17, Chrome on Linux does four things:

- **The throw.** `ui/ozone/platform/wayland/host/wayland_event_source.cc` keeps the last three
  touchpad scroll frames. At `axis_stop` it fits their velocity by least squares, with time
  counted back from the lift. It throws nothing when the lift comes more than 200 ms after the
  last frame.
- **The curve.** `PhysicsBasedFlingCurve` (`ui/events/gestures/physics_based_fling_curve.cc`),
  on by default on Linux through `features::kExperimentalFlingAnimation`. The page travels
  `v² / 2a` with `a` = 2.7559e-5 inches per ms² at 96 points per inch (2,645.7 points per s²).
  The travel is capped at three window heights. The fling lasts `|v| / 2300` seconds, at most
  2 s, on an ease-out cubic Bézier whose start is fitted to the release speed.
- **The stop.** Fingers landing on the touchpad stop a running fling. The compositor reports
  this as the `zwp_pointer_gesture_hold_v1` hold. A new swipe also stops it.
- **The boost** (`ui/events/blink/fling_booster.cc`). A swipe that begins within 50 ms of a
  stop, keeps heading the same way and moves fast enough adds to the throw. The stopped
  fling's start speed is added to its own, and the travel cap grows by the same factor.

Decision: Quire does all four, in Rune, so every Rune scroll view behaves this way. Rune's
winit carries each Wayland axis event's own time, so frames that arrive together after a slow
paint still throw by their real spacing. It also reports the hold gesture.

Two consequences differ from omarky's spec:

- The page travels farther and slows more gently than on omarky's curve. A 2,400 point a
  second swipe carries 1,088.6 points.
- A fling still runs under reduced motion, as it does in Chrome. A fling continues the
  fingers' own movement, which the user started. It is not an animation the app adds.

The code is in Rune: `runtime/input/throw.rs` handles the release, the stop and the boost, and
`runtime/scroll/fling.rs` is the curve. `vendor/winit` adds the event times and the hold.
Quire's `desktop/src/throw_tests.rs` checks the reader against the numbers above.
