# Verification

## Automated Rust checks

The workspace has 27 passing tests and one deliberately ignored stalled-child fixture. The supervisor test invokes and terminates that fixture; it is not an untested feature.

- Nine private-workspace tests cover persistence, duplicate paths, unsupported/missing files, original preservation, future schemas, malformed settings, legacy-data reuse, concurrent initialization, and migration from private schema 1 to 2 with retained documents/preferences and cached-text deletion.
- Four parser tests check all seven synthetic files against independent expected results, original PDF page and Word table/header references, Unicode/entities/strict Word namespaces, malformed XML/DTD rejection, and file/expanded-XML limits.
- Three executable integration tests run the shipped GUI-subsystem executable headlessly and invoke its actual child worker: all seven cases, original preservation, no public-catalog initialization, persistent caches, same-size content changes, deletion, and overwrite refusal.
- Two worker-supervisor tests cover cancellation before process launch and termination of a stalled child.
- Seven public-catalog tests and two location-discovery tests retain the source-library, provenance/integrity, migration, export, isolation, concurrency, and rename checks from 0.2.0.

Formatting and workspace Clippy run with warnings denied. Release builds use the lockfile. The test fixtures contain no real people, employers, identifiers, or private documents. See [Samples](../examples/README.md) and [expected results](../examples/expected-results.json).

## Offline reading observation

On Linux, each of the seven files was read through the actual executable and child worker under `strace -f -e trace=network`. All seven runs returned their expected states and produced **zero network system calls**. The Word sample contains an external `example.invalid` hyperlink relationship; the label is extracted and the URL is not fetched.

This observation covers those diagnostic reading runs, not every possible desktop/operating-system action. Source/dependency inspection also confirms no HTTP client/AI integration in the reader and no core/extractor dependency in the public updater. Resource limits are not a full OS security sandbox. Future source networking still requires enforced restrictions and new boundary tests.

## Native interface

The real ComplyPH window is checked on the shared Linux desktop using only synthetic documents and public sources. The README screenshot comes from the actual native reader. Checks include sample selection, automatic extraction, PDF page navigation/search, Word Unicode/table references, scanned-file status, repeated opening, and retained local data.

The existing seven-domain source filters, source search, offline source reading, and exact original export were also checked for 0.2.0. The exported NPC circular matched the original byte for byte. No private user document was used.

## Windows workflow

GitHub Actions uses the pinned Rust/MSVC toolchain and static C runtime. It repeats formatting, tests, and Clippy, builds the desktop/updater, packages an NSIS installer/app ZIP, and runs an isolated install check.

The check refuses an existing runner installation/data root and uses synthetic data only. It verifies:

1. Upgrade from the hash-pinned LaborLens PH 0.1.0 installer, registered directory/shortcut replacement, executable startup, and default reuse of the legacy data root.
2. Import of 14 official snapshots across seven domains without changing private bytes.
3. All seven installed reader fixtures, including the same child-process resource limits used by the GUI, expected text/states/locations, and original preservation.
4. Private schema 1 → 2 migration without losing prior document references/preferences, and cache creation.
5. Reinstall location retention, fresh installation, upgrade from hash-pinned ComplyPH 0.2.0, and data/cache/original preservation through uninstall.

Exact-commit results and machine-readable installation evidence accompany releases. The publisher accepts artifacts only from successful `main` push builds and verifies source SHA, sample bytes, manifests, checksums, upgrade evidence, and uploaded asset sizes/digests before publishing.

Interactive Windows display scaling, assistive technology, and every device’s drag/drop behavior remain target-PC checks. The build is unsigned. No claim is made that extraction faithfully reproduces every font, layout, embedded object, revision, or image.

## Remaining scope

Source verification means official origin, identity, and preserved-content integrity. It does not certify legal currency, complete amendments, interpretation, or compliance; [known collection gaps](../library/README.md) remain unchanged.

OCR, legal checking rules, reports, watched folders, manual source import, and daily source downloads are not implemented. No runtime legal-source update has run.
