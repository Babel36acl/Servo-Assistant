# glib 0.18.5 security backport

Source: crates.io glib 0.18.5, preserved with its original version and MIT license.
Only `src/variant_iter.rs` is modified: make the out-argument `p` mutable and pass
`&mut p` to `g_variant_get_child`, exactly as upstream PR #1343 does.

- Advisory: https://github.com/advisories/GHSA-wrw7-89jp-8q8g
- Upstream fix: https://github.com/gtk-rs/gtk-rs-core/pull/1343
- Upstream merge: 05dff0ee696f9bcd8617cd48c4b812d046d440cb
- Compatible release request: https://github.com/gtk-rs/gtk-rs-core/issues/2010

Tauri's Linux GTK3 dependencies require glib ^0.18. Adding a separate 0.20
dependency would not replace their vulnerable copy. Cargo's workspace patch
replaces all 0.18 users with this local backport. Do not suppress the advisory
or claim this is an upstream fixed release. Version-based scanners can still
report 0.18.5; consult the resolved source and this patch.

Remove this override when the GTK stack accepts an upstream patched release.
The application's `glib_security` integration test runs with release
optimizations in Linux release CI to exercise the affected iterator methods.
