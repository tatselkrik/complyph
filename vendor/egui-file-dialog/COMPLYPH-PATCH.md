# Local patch

Upstream: egui-file-dialog 0.16.0 from crates.io, by Jannis Teipel, MIT license.

ComplyPH uses the current theme’s text-edit canvas for readable navy text on cream/ivory. Dialog geometry is constrained to the viewport so the picker fits compact displays. Modal pickers register egui’s keyboard modal layer; Escape cancels the picker after keyboard navigation, while inner folder/path edits dismiss first. Path, search, filename and folder-navigation controls expose descriptive accessibility names.

Source files and the upstream license are retained. The upstream manifest is preserved as `Cargo.toml.upstream`; the build manifest omits upstream examples, development dependencies, and lint configuration. No networking or document-processing behavior is added.
