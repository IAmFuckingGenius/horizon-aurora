//! Gameplay and startup, above the reusable client library.
// No `#![recursion_limit]` or other inner attribute horizon-mixins doesn't know here: the
// launcher's horizon-mixins then rejects every native mod targeting this crate (#8169).
// Android's host (android_host.rs) calls into the activity through JNI and the C library.
#![cfg_attr(not(target_os = "android"), forbid(unsafe_code))]
extern crate self as horizon_client;
pub use horizon_client_lib::*;
// Horizon on Android: the launcher and the platform hooks the game calls as `crate::<item>`.
#[cfg(target_os = "android")]
pub mod android_host;
#[cfg(target_os = "android")]
pub(crate) use android_host::*;
pub mod entry;
pub mod game;
pub mod mod_api;

#[cfg(test)]
mod brewing_speed_load_tests;
