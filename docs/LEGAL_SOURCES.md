# Add legal source

ComplyPH can preserve local PDF and DOCX legal references with citation and verification metadata. Source intake runs offline and keeps the private document workspace separate.

## Import

Open **Legal library → Add legal source**. Choose a local file, inspect the extracted-text preview and enter:

| Field | Meaning |
| --- | --- |
| Title | Publication title displayed in the library |
| Instrument / number | Act, order, circular, advisory or other reference identifier |
| Issuing agency | Organization responsible for the publication |
| Official URL | HTTPS citation supplied by the user; recorded without making a request |
| Compliance domains | One or more of Labor, DOLE OSHS, Privacy/NPC, BIR, SSS, PhilHealth and Pag-IBIG |
| Issue date | Date signed/published, when established; `YYYY-MM-DD` or blank |
| Effective from | Established effective date, which may differ from the issue date; blank if uncertain |
| Effectivity / scope notes | Publication requirements, exclusions, amendments and unresolved applicability |
| Verification status / note | Pending or user-recorded identity comparison, with its limits |

Select **Save legal source**. Use the library's search/domain filters to find it, **Read local snapshot** to inspect its text, and **Save source copy** to export the preserved original to a new file. The source details show dates, citation, provenance, verification notes, extraction limits and the original SHA-256 hash.

The original selected file is unchanged. The import creates a public-reference copy in `library/catalog.sqlite3`, normally under `C:\ProgramData\ComplyPH`. It does not register that reference in Documents or write its text into the private review database. “Public” describes the catalog's reference-data role; it is a local database, not an upload or a publicly shared folder. Select public legal material for this workflow.

## Verification

**Awaiting verification** is the default. The official citation is a user-provided claim; recording a government URL does not establish that the selected file came from that site. Pending sources have no verification timestamp.

**Identity checked by me** requires confirming a comparison with the official publication and recording what was compared. The local record saves the user assertion and its time. It is labelled **Identity checked · user reviewed**, separately from the bundled **Verified agency download** sources. It does not certify current law, amendment completeness, applicability or OCR accuracy.

Raw-file and derived-text hashes protect snapshot integrity in both states. Integrity checks do not independently prove legal authority. Manually imported references never automatically add or change an automated checking rule.

## Versions and limits

The canonical citation URL identifies a manually imported source. A different file hash under the same URL adds an immutable snapshot and preserves earlier bytes, reading text and metadata. Importing identical bytes under that URL keeps the earlier snapshot and its original metadata/status; metadata editing is not implemented. A manual import cannot replace a bundled source or alter the source pinned by the five labour checks. Historical report citations still open their exact recorded hashes.

PDF/DOCX intake uses the existing bounded reader: 32 MiB input, 300 PDF pages, 4 MiB extracted text, 12,000 text units and a 20-second worker deadline. Parser memory/CPU guards apply. Scans without selectable text, encrypted files and unreadable files are rejected without a partial source entry; use external OCR first. Mixed selectable/scanned pages can produce incomplete reading text with warnings. Compare the preview with the original, especially for tables, figures and omitted content. In-app OCR is not available.

Daily checks of explicitly approved official URLs, retained downloads, reviewed comparison/adoption, metadata/identity history and approved-page publication discovery are implemented. Saving a citation alone does not approve networking or establish legal currency. See [library maintenance](LIBRARY_MAINTENANCE.md) and [source updates](SOURCE_UPDATES.md).

## Developer verification

The GUI and diagnostic use the same bounded parser and catalog write path. A diagnostic import requires an explicit public library directory and refuses to overwrite its result file:

```sh
complyph --import-legal-source reference.pdf --metadata source.json --output new-result.json --library-dir local-data/library
```

The [fictional metadata fixture](../examples/synthetic-source.json) documents the JSON fields. Use it only with synthetic test material. Executable tests cover PDF/DOCX import, duplicate intake, same-URL version history, verification separation, preserved originals/private data, failed scans/protected/damaged files and output overwrite refusal. Catalog tests additionally cover invalid URLs/dates/domains, required comparison notes, bundled-source protection and hash tampering after reopen.
