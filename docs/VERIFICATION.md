# Verification — ComplyPH 0.4.1

## Automated Rust checks

The workspace has **48 passing tests and one deliberately ignored stalled-child fixture**. The supervisor test invokes and terminates that fixture; it is not an untested feature. Formatting and workspace Clippy pass with warnings denied. Builds use the lockfile and pinned Rust toolchain.

- Five rule-validation tests cover independently transcribed Article 83/87/86 boundaries (including 0.01 either side), favourable terms, unknown/excluded scope, incomplete reading, unsupported/all-in/conditional/quoted/conflicting wording, pinned source quotes and tampering, and actual PDF/DOCX checking fixtures.
- Thirteen private-store tests cover retained references/preferences/caches, schema 1/2 → 3, report persistence/deletion, original preservation, malformed/future databases, concurrency and migration. Migration tests include committed WAL data, unchanged old source bytes, v0.3 cached text, ambiguous/corrupt/future sources, populated destinations and rollback of explicit import into an unused destination.
- Four executable integration tests invoke the actual desktop binary and child reader: the seven reader fixtures, content-hash cache refresh/reuse, overwrite refusal, three checking fixtures and saved private report history.
- Five parser tests cover independent reader outcomes, physical PDF pages, Word paragraph/table/header locations, XML/Unicode/entities/revisions, malformed documents and resource limits.
- Eight desktop unit tests cover passage ranges, changed/missing evidence, Unicode/chunks, complete article boundaries, related-rule rendering and cancellation/termination. Ten public-catalog tests cover all 19 sources, 14/3/2 provenance counts, tampering including provisional sources, export, idempotency, legacy schemas, concurrent import, historical source lookup and metadata-only bundle upgrades. Three path tests cover legacy discovery, ambiguity and empty directories.

These are technical tests, not qualified legal certification. See [rule scope, independent cases and limitations](VALIDATED_CHECKS.md). Synthetic files contain no real employee/employer records.

## Offline observation

The v0.4.0 desktop executable was run headlessly under `strace -f -e trace=network` to read the synthetic issue DOCX, validate the source pin, evaluate the rules and save a report. It produced the three expected potential issues with **zero network system calls**. The prior v0.3.0 seven-format reader observation likewise produced zero network calls.

This observation covers those runs, not every possible operating-system action. The desktop/checker/status-tool dependency boundary contains no HTTP client or AI integration. Worker limits are resource guards, not full OS confinement. Future approved-source networking still requires enforced file/network restrictions and new tests; no runtime online source check has been performed by this release.

## Native interface

The native Linux interface is checked using synthetic data: document selection, extraction, disabled/enabled check action, three applicability controls, accuracy confirmation, local execution, saved findings with original paragraph locations, Reports history and the library's 19 saved / 14 agency / 3 uploaded / 2 provisional display. The connected review is checked for independent pane scrolling, passage jumps/highlighting, complete offline provisions, related-rule tabs and saved-report reopening. No private user employment document was used.

## Windows release gate

GitHub Actions repeats formatting, all Rust tests and Clippy on Windows/MSVC, builds both executables with a static C runtime, packages NSIS/app ZIP and runs an isolated install test. Windows-specific ACL/process-enumeration code also passes a Linux-to-Windows GNU target type check; actual runtime permissions are checked on Windows.

The isolated Windows test refuses pre-existing installations/data roots. It requires:

1. Upgrade from hash-pinned earlier 0.1.0 desktop, registered installation/shortcut replacement and executable startup.
2. Desktop migration of the legacy SQLite workspace to `%ProgramData%\ComplyPH`, retained source database bytes, migration marker, protected owner/SYSTEM/Administrators ACL, retained document references/preferences and private schema 3.
3. Public catalog status of 19 sources in seven domains, with 14 agency downloads, 3 identity-checked uploads and 2 provisional copies; no successful runtime update is claimed.
4. All seven installed reader cases through the real child worker, expected text/states/locations and preserved originals.
5. All three installed checking cases, source pin/context audit data, expected findings and persisted private reports/caches.
6. Reinstall location retention, fresh install, upgrade from hash-pinned ComplyPH 0.4.0, and private/public/original preservation through uninstall.

The publisher accepts packages only from a successful `main` push run. It checks the exact source commit, manifests, provenance collection size, independent reader/check outcomes, ProgramData/ACL evidence, sample bytes, x64 GUI executable structure, checksums and uploaded asset digests before publishing. Each release includes machine-readable `windows-install-check.json` and `release-manifest.json`; those artifacts record the exact commit actually tested.

Interactive Windows display scaling, assistive technology, account-specific permissions and every device's drag/drop behavior remain target-PC checks. The build is unsigned. No promise is made that extraction reproduces every font/layout/object/revision or that three matching numeric terms establish complete legal compliance.

## Remaining scope

Daily source downloads, manual legal-source import, watched folders, in-app OCR, other-domain rules and qualified legal review remain outstanding. Uploaded/provisional additions preserve their verification limits; [coverage gaps](../library/README.md) remain visible.
