// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 juanre7

//! Fuzzes the `config.json` reader.
//!
//! `AppConfig::from_json` runs on a hand-rolled JSON parser (`json_field`): it finds the
//! key by name, steps over the `:`, and cuts the value at a quote or at the first `,`,
//! `\n` or `}`. All of that is `find`, `get` and `strip_prefix` over byte indices of a
//! `&str`, which is exactly where an odd document can slice through the middle of a UTF-8
//! character and panic. The file is written by the application, but it lives on the user's
//! disk: anyone can edit it by hand, and a power cut can leave it half written.
//!
//! Two things are checked:
//!
//! 1. No document panics. A corrupt `config.json` has to fall back to the defaults, not
//!    stop Ruuutu from starting.
//! 2. What is read can be written back and read again unchanged. `to_json` claims to be
//!    symmetric with `from_json`; this is what keeps it honest. That symmetry broke once
//!    for real — the writer emitted `Png` while the reader looked for `PNG` — and it cost
//!    PNG and JPEG silently reverting to WebP on every restart.

#![no_main]

use libfuzzer_sys::fuzz_target;
use ruuutu::config::AppConfig;

fuzz_target!(|data: &[u8]| {
    // The file is read with `read_to_string`, so whatever reaches `from_json` is always
    // valid UTF-8. `from_utf8_lossy` reproduces that contract without throwing away half
    // of the generated cases, and lets the fuzzer keep exploring arbitrary bytes.
    let document = String::from_utf8_lossy(data);

    let parsed = AppConfig::from_json(&document);

    // The fixed point: serializing what was read and reading it back must give the same
    // thing. Otherwise a setting is lost silently on the next restart.
    let reparsed = AppConfig::from_json(&parsed.to_json());
    assert_eq!(
        parsed, reparsed,
        "to_json and from_json are no longer symmetric for: {document:?}"
    );

    // The settings turn into encoder parameters on every capture; that step honouring its
    // ranges is part of the configuration's contract.
    let opts = parsed.save_options();
    assert!((1..=100).contains(&opts.quality), "quality out of range: {opts:?}");
    assert!((1..=9).contains(&opts.png_level), "png_level out of range: {opts:?}");
    assert!(
        (1..=100).contains(&opts.scale_percent),
        "scale_percent out of range: {opts:?}"
    );
});
