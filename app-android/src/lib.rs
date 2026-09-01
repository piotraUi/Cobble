//! Android entry point.
//!
//! Not implemented yet — this is roadmap step 6. It will use
//! `android-activity` to drive the same `renderer`/`client-core` game
//! loop as `app-desktop`, with a virtual joystick + touch-look + touch
//! buttons feeding `client_core::InputState` instead of keyboard/mouse,
//! and will be built to a `.apk` with `cargo-ndk`/`cargo-apk`.
