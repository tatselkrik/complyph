# Roadmap

## 0.3.0 — Local document reader

Implemented: offline Rust PDF/DOCX text reading, page/paragraph/table/part references, selectable text and search, private content-hash-validated caches, private schema 1 → 2 migration, cancellation and process resource limits, explicit scan/protected/damaged-file states, and seven synthetic cases with expected results. Originals remain unchanged. This milestone provides reading, not legal assessment.

## 0.2.0 — ComplyPH and the starter library

Implemented: ComplyPH branding, preserving legacy-data discovery, schema 2 public catalog migration, 14 original official-publication snapshots across seven domains, domain filters, source search, offline reading, search within a source, source-copy export, and integrity checks. Known acquisition gaps remain explicit; source verification is not legal currency or compliance.

## 0.1.0 — LaborLens PH desktop foundation

Implemented: the native Rust interface, local PDF/DOCX file registration, searchable document history, metadata, saved preferences, private SQLite storage, an independent public legal-source catalog, and Windows packaging.

The application does not yet perform legal analysis.

## Shared engine and compliance domains

Build the review pipeline around shared document extraction, local evidence, applicability, citations, and reports. Define separate domain metadata and rules for labor compliance, DOLE OSHS, the Data Privacy Act/NPC, BIR, SSS, PhilHealth, and Pag-IBIG. Domain definitions use stable identifiers and keep regulators, source documents, and rules separate.

Begin with one labor review area and validate the full workflow. Add other domains as their source collections, evidence requirements, and rule coverage are validated. Support selecting several domains for a review and report each domain's coverage independently. See the [compliance-domain design](COMPLIANCE_DOMAINS.md).

## Verified sources and first checking rules

Extend the initial official-source collection, close acquisition gaps, and establish one well-defined labor review area. Record domain membership, issuer, source URL, publication and effective dates, applicability, verification state, and version history. Allow a source to belong to more than one domain. Pair supported requirements with explicit checking rules and representative validation cases.

Maintain source documents and rule coverage separately. A new source does not automatically establish a correct new checking rule.

## Local text extraction

Supported PDF/DOCX text reading and location references shipped in 0.3.0. Future work: broaden layout/font/embedded-content coverage, provide extraction-quality review, and add offline OCR for scans with clear limits on recognition quality.

## First complete review

Collect the required context and evidence, evaluate supported rules, and present potential issues beside the relevant document passage. Include the domain, exact source provision, effective date, explanation, missing facts, and review status. Resolve applicability using the period under review and preserve its source and rule versions.

Validate with synthetic or appropriately anonymized documents and qualified legal review. Evaluate both missed issues and incorrect flags. A result with no issue found must state its supported coverage.

## Library management and daily updates

Add manual source entry by URL or local file. Check approved official publication pages daily and automatically preserve valid new source versions. Record failures and missed checks visibly. Apply rule updates automatically only through validated transformations; flag unsupported legal changes for maintenance.

Enforce and test the updater's operating-system network and file restrictions before enabling it. Private document data must not become an input to the updater.

## Folder intake and reports

Monitor a designated local folder, wait for files to finish writing, and avoid repeated reviews of unchanged content. Keep originals unchanged. Export reports that preserve the source and rule versions used.
