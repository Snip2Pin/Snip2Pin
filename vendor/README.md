# Vendored dependency patches

`egui-winit` is copied from the published 0.36.2 crate, upstream commit
[`49682f8`](https://github.com/emilk/egui/commit/49682f8baa058bf49e011035cfbd6e825f88a5ef).
Its MIT and Apache 2.0 license texts are retained in its directory. The manifest
only adjusts license paths for this layout and disables publishing.

The sole runtime change skips `Window::is_maximized()` when initializing a
macOS undecorated viewport, leaving that information unknown (`None`). Winit
0.30.13 temporarily adds a title bar during this query. On an ordered window
at the work-area edge, AppKit moves the content down and restoring the original
style does not restore its position. See
[winit #4071](https://github.com/rust-windowing/winit/issues/4071).
Minimized state, decorated windows and other platforms retain their original
behavior. The patch neither changes geometry nor substitutes window classes.

`cargo test --test macos_lifecycle` covers the ordered-window initialization
with fully transparent, mouse-pass-through windows and activation prohibited.
It must fail with the unpatched dependency. Remove this copy and the Cargo
patch when an upstream release fixes the query, retaining the regression test.
