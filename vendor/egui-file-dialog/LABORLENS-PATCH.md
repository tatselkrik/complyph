# Local patch

Upstream: egui-file-dialog 0.16.0 from crates.io, by Jannis Teipel, MIT license.

The upstream search and path editors use egui's `Frame::dark_canvas`, which is a nearly black background even with a light theme. LaborLens PH replaces those uses with `Frame::canvas` filled from the current theme's text-edit background. This preserves readable navy text on a cream/ivory surface.

Source files and the upstream license are retained. The upstream manifest is preserved as `Cargo.toml.upstream`; the build manifest omits upstream examples, development dependencies, and lint configuration. No networking or document-processing behavior is added.
