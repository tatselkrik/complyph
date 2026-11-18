# Verification — ComplyPH 0.6.0

## Automated Rust checks

The workspace passes 62 tests on Windows and 61 on Linux, covering the reader, rules, storage, source updates and isolation, with one deliberately ignored stalled-child fixture. The supervisor test invokes and terminates that fixture; it is not an untested feature. Formatting and workspace Clippy pass with warnings denied. Builds use the lockfile and pinned Rust toolchain.

- Five rule-validation tests cover independently transcribed Article 83/87/86 boundaries (including 0.01 either side), favourable terms, unknown/excluded scope, incomplete reading, unsupported/all-in/conditional/quoted/conflicting wording, pinned source quotes and tampering, and actual PDF/DOCX checking fixtures.
- Thirteen private-store tests cover retained references/preferences/caches, schema 1/2 → 3, report persistence/deletion, original preservation, malformed/future databases, concurrency and migration. Migration tests include committed WAL data, unchanged old source bytes, v0.3 cached text, ambiguous/corrupt/future sources, populated destinations and rollback of explicit import into an unused destination.
- Six executable integration tests invoke the actual desktop binary and child reader: the seven reader fixtures, content-hash cache refresh/reuse, overwrite refusal, three checking fixtures and saved private report history; public-reference PDF/DOCX import, duplicate/version handling, status separation, failed-file rejection, private/original preservation and overwrite refusal.
- Five parser tests cover independent reader outcomes, physical PDF pages, Word paragraph/table/header locations, XML/Unicode/entities/revisions, malformed documents and resource limits.
- Eight desktop unit tests cover passage ranges, changed/missing evidence, Unicode/chunks, complete article boundaries, related-rule rendering and cancellation/termination. Seventeen public-catalog tests cover all 19 sources, 14/3/2 provenance counts, tampering including provisional sources, export, idempotency, legacy schemas, concurrent import, historical source lookup and metadata-only bundle upgrades, manual URL/date/domain validation, comparison-note requirements, immutable manual history and bundled-source protection. Three path tests cover legacy discovery, ambiguity and empty directories.

These are technical tests, not qualified legal certification. See [rule scope, independent cases and limitations](VALIDATED_CHECKS.md). Synthetic files contain no real employee/employer records.

## Offline observation

The v0.5.0 source-import diagnostic was run under `strace -f -e trace=network` using the fictional PDF/metadata fixture. The complete bounded parse and public-catalog write succeeded with **zero network system calls**, pending verification and no private directory created.

The v0.4.0 desktop executable was run headlessly under `strace -f -e trace=network` to read the synthetic issue DOCX, validate the source pin, evaluate the rules and save a report. It produced the three expected potential issues with **zero network system calls**. The prior v0.3.0 seven-format reader observation likewise produced zero network calls.

This observation covers those runs, not every possible operating-system action. Those earlier runs covered the offline desktop/checker path. In v0.6.0 the status command still makes no request, and source networking requires the separate AppContainer child. Parser resource guards are distinct from downloader OS confinement.

## Native interface

The native Linux interface is checked using synthetic data: document selection, extraction, disabled/enabled check action, three applicability controls, accuracy confirmation, local execution, saved findings with original paragraph locations, Reports history and the library's 19 saved / 14 agency / 3 uploaded / 2 provisional display. The connected review is checked for independent pane scrolling, passage jumps/highlighting, complete offline provisions, related-rule tabs and saved-report reopening. The source-intake flow was also exercised end to end: themed file selection, bounded extraction, invalid-date rejection, required comparison confirmation, pending import, library count/status refresh, search and offline reading with saved citation metadata. No private user employment document was used.

The v0.6.0 source-update interface was exercised with an explicitly synthetic downloaded-copy fixture: pending-change filtering, source history, required review notes, immutable review records, local export with a matching SHA-256 hash, and the centered export dialog. The original reference and private document store were preserved. This fixture did not contact an agency or represent a legal amendment.

## Windows release gate

GitHub Actions repeats formatting, all Rust tests and Clippy on Windows/MSVC, builds both executables with a static C runtime, packages NSIS/app ZIP and runs an isolated install test. Windows-specific ACL/process-enumeration code also passes a Linux-to-Windows GNU target type check; actual runtime permissions are checked on Windows.

The isolated Windows test refuses pre-existing installations/data roots. It requires:

1. Upgrade from hash-pinned earlier 0.1.0 desktop, registered installation/shortcut replacement and executable startup.
2. Desktop migration of the legacy SQLite workspace to `%ProgramData%\ComplyPH`, retained source database bytes, migration marker, protected owner/SYSTEM/Administrators ACL, retained document references/preferences and private schema 3.
3. Public catalog status of 19 sources in seven domains, with 14 agency downloads, 3 identity-checked uploads and 2 provisional copies; no successful runtime update is claimed.
4. All seven installed reader cases through the real child worker, expected text/states/locations and preserved originals.
5. All three installed checking cases, source pin/context audit data, expected findings and persisted private reports/caches.
6. Installed public-source intake and duplicate handling, public schema 4, pending verification, unchanged private data and preserved source originals.
7. Reinstall location retention, fresh install, upgrade from hash-pinned ComplyPH 0.5.0, and private/public/original preservation through uninstall.
8. Installed AppContainer denial of synthetic private-store, catalog and ordinary original-file canaries, an empty no-approval batch and certificate-validated HTTPS transport in a separate public-only probe catalog.

The publisher accepts packages only from a successful `main` push run. It checks the exact source commit, manifests, provenance collection size, independent reader/check outcomes, ProgramData/ACL evidence, sample bytes, x64 GUI executable structure, checksums and uploaded asset digests before publishing. Each release includes machine-readable `windows-install-check.json` and `release-manifest.json`; those artifacts record the exact commit actually tested.

Interactive Windows display scaling, assistive technology, account-specific permissions and every device's drag/drop behavior remain target-PC checks. The build is unsigned. No promise is made that extraction reproduces every font/layout/object/revision or that three matching numeric terms establish complete legal compliance.

## Remaining scope

Reviewed promotion of downloaded references, source-page discovery, metadata editing, watched folders, in-app OCR, other-domain rules and qualified legal review remain outstanding. Uploaded/provisional additions preserve their verification limits; [coverage gaps](../library/README.md) remain visible.

## Source update validation

Catalog tests exercise explicit agency URL approval, invalid/local/credential URLs, no-approval behavior, changed/unchanged/304/error states, byte reversions, duplicate bodies, daily scheduling, interrupted leases, stale/revoked approvals, immutable review notes, original/private preservation, export overwrite refusal, hash tampering and schema 3 → 4 retention. Synthetic responses are fixtures and do not claim a legal amendment.

The updater's actual executable is tested for direct-worker isolation refusal and no-approval status/empty batches with a private sentinel. Windows additionally launches its AppContainer and tests denial of the private database, public catalog and an ordinary synthetic original. The installed package repeats this denial test and the no-approval batch check. Release publication requires those recorded results. HTTP policy tests reject private/reserved/mapped addresses; the worker uses a checked-address resolver, certificate-validated HTTPS and same-origin redirects.

The v0.5.0 offline source-import observations above describe those earlier runs. In v0.6.0 the desktop/reader/rules remain without an HTTP client; approved update checks deliberately make public-source requests in a separate confined Windows process. Other-platform builds fail closed. Crate boundaries alone are not relied on for network-worker file access.

The installed Windows check also observes a certificate-validated response from an explicitly selected public agency URL using an isolated public-only probe catalog. An HTTP rejection establishes transport only; it is recorded as a failed source check, not source verification. Source availability and legal effect are not inferred from this smoke check.
