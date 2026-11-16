# Architecture

## Current implementation

ComplyPH 0.2.0 is a native Rust desktop application using egui/eframe. It combines the private PDF/DOCX reference workspace with 14 bundled official-publication snapshots across seven compliance domains. Private-document extraction, legal checking, reports, folder monitoring, manual source import, and runtime source downloads remain future work.

| Component | Responsibility | Private-data dependency |
| --- | --- | --- |
| `complyph-desktop` | Native workspace, file selection, source filters, reader, and original export | Uses both stores |
| `complyph-core` | Private document references, settings, paths, and schema protection | Owns private workspace |
| `complyph-library` | Public catalog, domains, snapshots, provenance, integrity, and migration | No private-workspace dependency |
| `complyph-paths` | Shared current/legacy data-root discovery | Directory existence only; no document or database reads |
| `complyph-updater` | Independent public-catalog status CLI and bundled import | Public catalog only |

There is no HTTP client, telemetry, AI integration, or runtime downloader. Source acquisition for the bundled collection is a maintainer activity involving public publications only.

## Local storage and the rename

Fresh Windows installations use `%LOCALAPPDATA%\ComplyPH`. If only `%LOCALAPPDATA%\LaborLens PH` exists, both the desktop and catalog status tool reuse it in place. Live SQLite files are not copied or moved. If both roots exist, discovery fails visibly and preserves both; the developer `--data-dir` override can select a workspace explicitly.

```text
private/workspace.sqlite3   Private document references and preferences; schema 1
library/catalog.sqlite3    Public sources, versions, domains, originals, and reading text; schema 2
```

The installer detects a registered LaborLens PH installation and upgrades its existing installation directory. It replaces known legacy executable/shortcut branding and the uninstall registration. It does not remove original documents or application-data directories.

Document references use canonical paths. Duplicate registration refreshes metadata without adding another entry. Removing an entry preserves the original file. Newer database schemas and malformed saved preferences are rejected without destructive replacement.

## Public source catalog

[The starter manifest](../library/starter/manifest.json) defines stable source keys, seven domain IDs, original and resolved publication URLs, acquisition/verification dates, hashes, extraction methods, applicability notes, and known gaps. [The library notes](../library/README.md) define verification scope and inventory.

On catalog open, migrations preserve schema 1 entries and versions. A local transaction imports original bytes and derived reading text into source snapshots, relates sources to domains, and records the immutable bundle identity. Sources can belong to several domains. Repeated and concurrent opens must not duplicate the bundle.

Original bytes and reading text are SHA-256 checked against their metadata before import, after reopen, and before export. A damaged snapshot produces an integrity error. Export uses a new file and refuses to overwrite an existing path.

Verification establishes publication provenance, document identity, and content integrity. It does not establish complete amendments, current applicability, OCR accuracy, or a legal conclusion. A bundled verification timestamp is separate from a successful runtime-update check; the latter remains unset.

Signed/issued dates are recorded only where established. Unknown effective dates remain unknown. Conditional effectivity clauses and applicability notes are preserved. Source retrieval is not treated as legal effectivity.

## Interface

The interface and application logic are Rust; operating-system components and bundled SQLite also contain native code in other languages. The egui file browser, fonts, and icons are local.

The Legal library page filters sources by domain, searches titles/instruments/issuers, displays reading text in bounded sections, searches within a source, and exports an exact original. Reading sections are display chunks, not source page citations. Active source HTML is never rendered.

## Planned checking pipeline

1. Extract private PDF/DOCX text locally with original document-location references.
2. Select review domains and collect required local context and evidence, including the period under review.
3. Resolve applicable source and rule versions for that period.
4. Evaluate explicit, versioned rules and produce findings with passages, source provisions, missing facts, and review status.
5. Preserve evaluated domains, coverage, evidence, source versions, and rule versions in reports.

The source/domain layer is implemented. Rule definitions, rule-to-provision mappings, structured review evidence, and the checking engine are not implemented. Rules must be reviewed data and Rust logic. Money calculations need exact decimal arithmetic. Missing evidence must remain a missing-evidence state.

## Planned source updates

A separate updater will check approved public publication listings and registered documents daily, preserve valid new versions, and show last successful checks and failures per source. Missed checks should run when the PC becomes available.

The updater must receive no private document contents, filenames, paths, hashes, employee/employer details, payroll data, findings, or document-derived queries. Requests must be determined by the public catalog. Validate source domains, redirects, file types, and acquisition metadata.

Before networking is enabled, enforce operating-system file/network restrictions and verify the boundary at runtime. A crate boundary alone is insufficient. Source acquisition, verification, applicability, and rule coverage remain distinct. A new source version does not silently activate a new legal interpretation.

Manual source import remains planned and must preserve provenance, dates, unresolved verification, and domain assignments.
