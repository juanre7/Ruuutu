// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 juanre7

//! The Ruuutu core: the two modules that never touch Win32.
//!
//! Ruuutu is a Windows application, but persisting the settings and encoding the image is
//! plain logic. Keeping it in a library gives it two things the binary cannot have:
//!
//! - It builds and runs its tests on any platform, with no desktop session.
//! - It can be fuzzed with `cargo-fuzz`, which needs LLVM's sanitizers and therefore does
//!   not exist on Windows (see `fuzz/`).
//!
//! This is not a second copy: `src/main.rs` imports these modules instead of declaring
//! them again, so the binary and the tests compile the very same code.
//!
//! The one Windows-specific corner inside `config` — autostart, which lives in the
//! registry rather than in the file — sits behind `#[cfg(windows)]` and reads as "off, and
//! cannot be turned on" everywhere else.

pub mod config;
pub mod storage;
