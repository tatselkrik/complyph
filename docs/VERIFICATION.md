# Verification

## Automated source checks

The foundation has eight tests covering:

- Document references and preferences surviving a database reopen.
- Duplicate canonical paths refreshing metadata without duplicate entries.
- Unsupported files, missing files, and folders not creating document records.
- Entry removal preserving original file bytes.
- Newer database schemas being rejected without overwrite.
- Malformed stored preferences being reported without replacement.
- Empty catalogs never claiming verified sources or a successful update.
- Public catalog initialization preserving an unrelated private-workspace sentinel.

The source is checked with formatting and workspace Clippy with warnings denied. The GitHub workflow repeats these checks on Windows, builds the release, packages it, and runs the isolated installer smoke check. Its exact-commit result is available from the repository's Actions page.

## Native UI verification

The foundation was run as a native egui/eframe Linux application. Observed checks include all five navigation destinations, file-browser navigation and sample registration, Ctrl+O and Escape, filename search and clearing, document details, saved preferences, and recovery of the last section and document list after restart.

A filename-row activation issue caused by selectable text was fixed and retested. The file browser's hard-coded black editor backgrounds were corrected for the light theme. The original synthetic PDF remained unchanged.

The README screenshot is from the actual native window using a synthetic sample. No real employee or employer records are included in the repository or screenshots.

## Windows packaging checks

The Windows workflow uses the pinned Rust/MSVC toolchain with a statically linked C runtime. It builds the application and updater, packages an NSIS installer and app ZIP, and checks installation, desktop and Start menu shortcuts, CLI startup through `--help`, catalog status, and preservation of synthetic data and originals through uninstallation.

The installer smoke test runs only on an isolated GitHub Actions runner and refuses to run over an existing installation. It does not install software on a contributor's personal computer.

The release is unsigned. Checksums accompany release assets. Hosted-runner installation checks do not establish Windows UI behavior on every graphics driver or device. Windows interactive display scaling, assistive technology, and native drag and drop remain target-device acceptance checks.

## Current scope

The app records file references and metadata only. There is no document-text extraction, verified legal corpus, checking engine, report generation, watched folder, or daily downloader yet. The current updater is a local status stub and makes no network requests.

The crate boundary between public and private storage is verified by source structure and tests. A future downloader still requires operating-system sandboxing, approved-source restrictions, and runtime network verification before networking is enabled.
