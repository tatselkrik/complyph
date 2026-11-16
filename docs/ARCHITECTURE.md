# Architecture

## Current implementation

LaborLens PH 0.1.0 is a native Rust desktop foundation using egui/eframe. It registers references to local PDF/DOCX files and stores preferences. It does not yet extract text, apply legal rules, create reports, watch folders, or download sources.

| Component | Responsibility | Dependencies on private data |
| --- | --- | --- |
| `laborlens-desktop` | Native UI, local file browser, document list, and preferences | Uses private workspace and public catalog |
| `laborlens-core` | Private file references, settings, paths, and SQLite migrations | Owns the private workspace |
| `laborlens-library` | Public legal-source catalog and source-version schema | No private-workspace dependency |
| `laborlens-updater` | Independent public-catalog status stub | Public catalog only |

The desktop and updater stub contain no HTTP client. The public library and private workspace have different databases. The current updater reports zero sources and no successful update when initialized; it does not fabricate freshness or verification status.

## Local storage

On Windows, the default data root is `%LOCALAPPDATA%\LaborLens PH`:

```text
private/workspace.sqlite3   Document references and preferences
library/catalog.sqlite3    Public sources and source-version metadata
```

Document references use canonical paths. Re-registering the same path refreshes file metadata without creating a second entry. Removing an entry leaves the original untouched. SQLite schema versions newer than the application supports are rejected, and malformed preferences are reported instead of silently overwritten.

## Interface

The interface and application logic are Rust. The file browser is also rendered in egui, so selection does not depend on a platform file-picker service. Its pinned local patch corrects light-theme editor backgrounds. Fonts and icons are bundled.

SQLite and operating-system components include native code written in other languages. Rust application code does not imply that every dependency is implemented in Rust.

## Planned compliance domains

Labor compliance is the first review area. The shared review engine should also support occupational safety and health (DOLE OSHS), data privacy (the Data Privacy Act and NPC issuances), tax requirements (BIR), and SSS, PhilHealth, and Pag-IBIG requirements as coverage is developed.

Keep document extraction, private evidence storage, applicability evaluation, finding presentation, and report snapshots shared. Each domain supplies its source mappings, required facts, versioned rules, evaluators, and validation cases. Several domains can apply to one document or review.

The current catalog already stores generic source titles, issuers, URLs, and source versions; it has no Labor Code-specific schema. Domain membership, a rule registry, structured evidence, and rule applicability are not implemented yet. Their future migrations should use stable identifiers, allow one source to support several domains, and keep regulators distinct from domains. DOLE, for example, can issue sources relevant to both labor and occupational safety and health.

The [compliance-domain design](COMPLIANCE_DOMAINS.md) defines these boundaries and the extension points. Domain expansion retains local document processing and the independent public-source updater.

## Planned review pipeline

1. Extract local PDF/DOCX text with document-location references; use offline OCR for scans where supported.
2. Select the review domains and collect the context and local evidence each requires, such as region, employment category, industry, and the period under review.
3. Resolve applicable source and rule versions for that period and evaluate explicit, versioned rules against the supplied facts.
4. Produce findings with the document passage, source provision, explanation, and any missing context.
5. Preserve the source and rule versions used by each report.

Rules are validated data and Rust logic, not executable instructions downloaded from a website. Money calculations should use exact decimal arithmetic. Unsupported wording and missing facts must produce a review state rather than an unsupported legal conclusion.

Reports must identify the selected domains, evaluated rules, missing evidence, and unsupported coverage. A document review does not establish that an external payment, filing, or workplace activity occurred; checks that depend on those facts need suitable local evidence.

## Planned automatic source updates

A separate updater will check approved publication listings and registered legal documents daily, download valid changes into staging, preserve earlier versions, and refresh the local source index. Checks missed while a PC is off or offline should run when it becomes available. The UI must show last successful checks and failures per source.

The updater must receive no private document contents, file paths, hashes, employee details, findings, or document-derived search queries. Its requests are determined by the public source catalog. Redirects, source domains, file types, and acquisition metadata need validation.

Before networking is enabled, enforce the updater's file and network restrictions at the operating-system level and verify them at runtime. A crate boundary alone does not provide that enforcement.

Source text, source verification, applicability, and rule coverage are separate states. A valid acquisition can update the local source library automatically. Updating a checking rule automatically requires a supported, validated transformation. New or ambiguous legal requirements should flag affected checks as needing maintenance.

Manual source addition should accept an official URL or local legal document, preserve provenance and dates, and identify unresolved verification. Adding a source does not silently create a new legal interpretation.
