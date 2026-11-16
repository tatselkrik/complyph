# Verification

## Automated Rust checks

Sixteen tests cover:

- Seven private-workspace cases: persistence, duplicate paths, unsupported/missing files, original preservation, future schemas, malformed settings, and reopening legacy documents/preferences after the rename.
- Seven public-catalog cases: real snapshots in seven domains without a fabricated runtime update, idempotent imports and private-file isolation, schema 1 migration, corruption rejection, exact-byte export and overwrite refusal, future-schema protection, and concurrent first opens.
- Two location-discovery cases: reuse of legacy data without moving it, and explicit handling of two existing branded roots.

Formatting and workspace Clippy run with warnings denied. Bundled originals and reading text are hash checked; raw HTML is marked binary in Git so checkout cannot change its line endings.

## Native interface

The actual ComplyPH application was run on the Cloud Linux desktop with public legal sources and a synthetic document workspace. Observed checks include seven domain filters, source-title search, an offline source reader, search within a source, original-file export, and reopening the retained document list and catalog.

The exported NPC circular matched the original PDF SHA-256 byte for byte. The reader was adjusted to collapse excessive blank display lines and provide a larger text area; original stored reading text and publication bytes remain unchanged.

The README preview is from this actual native window. It contains no real employee/employer documents. Earlier document intake and preference interactions were also checked in the v0.1.0 foundation.

## Windows workflow

The Windows workflow uses the pinned Rust/MSVC toolchain and a statically linked C runtime. It repeats source checks, builds the desktop and catalog tool, packages an NSIS installer/app ZIP, and runs an isolated installation check.

The check installs the published v0.1.0 fixture after verifying its hash, creates synthetic schema 1 private/public databases, upgrades to ComplyPH using the registered old installation directory, and verifies replacement branding, shortcuts, CLI startup, and discovery of the existing data root. Catalog import must produce 14 verified publication snapshots across seven domains without changing private database bytes.

It also checks a fresh ComplyPH installation and verifies that uninstall preserves the private workspace, reference catalog, and original synthetic document. The check refuses an existing runner installation or workspace and does not run on a contributor's personal PC.

Exact-commit results and machine-readable installation evidence accompany releases. These checks do not establish interactive Windows scaling, assistive-technology behavior, or drag and drop on every device. The build is unsigned.

## Source verification scope

See [Starter library](../library/README.md) for official publication links, source-specific notes, extraction limitations, and blocked acquisitions. Verification concerns provenance, document identity, and hashes; it does not certify legal currency, complete amendment coverage, interpretation, or compliance.

There is no private-document extraction/checking engine, report generator, watched folder, manual source importer, or daily downloader. No runtime update check has run. Future networking still requires operating-system restrictions and boundary verification.
