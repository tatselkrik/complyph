# Architecture

## Current implementation

ComplyPH 0.10.0 is a native Rust desktop application using egui/eframe. It combines the private PDF/DOCX text reader and workspace with 19 preserved references with explicit provenance across seven compliance domains. Six deterministic labour comparisons, saved reports, separate reviewer tracking and offline PDF exports are implemented. Local PDF/DOCX legal-source import is implemented. Approved URL checks and immutable downloaded-change history are implemented for Windows. Bounded local PDF OCR and optional local folder intake are implemented.

| Component | Responsibility | Private-data dependency |
| --- | --- | --- |
| `complyph-desktop` | Native workspace, file selection, source filters, reader, and original export | Uses both stores |
| `complyph-core` | Private references, content snapshots, text cache, settings, and migrations | Owns private workspace |
| `complyph-extract` | In-memory PDF/DOCX parsing and structured text locations | Receives bytes only; no file/network I/O |
| `complyph-reports` | Pure local PDF rendering from a captured review/history, bundled fonts and atomic new-file export | Receives private snapshot; no HTTP/original-file access |
| `complyph-ocr` | Memory PDF rendering and OCR through packaged native C APIs; no HTTP | Receives private bytes in bounded worker |
| `complyph-checks` | Pure, source-pinned numeric rules with context gates; no I/O | Receives extraction/context values |
| `complyph-library` | Public catalog, domains, snapshots, provenance, integrity, and migration | No private-workspace dependency |
| `complyph-paths` | Shared current/legacy data-root discovery | File/directory existence and process-name checks; no document/database content reads |
| `complyph-updater` | Public-catalog coordinator, scheduler inputs and AppContainer-only HTTP worker | Public catalog only |

The desktop and private-processing crates have no HTTP client or telemetry, AI integration, or runtime downloader. Source acquisition for the bundled collection is a maintainer activity involving public publications only.

## Local storage and upgrades

Windows uses `%ProgramData%\ComplyPH`, normally `C:\ProgramData\ComplyPH`, with a protected current-owner/SYSTEM/Administrators ACL. The desktop, before opening a window/store, detects a single legacy LocalAppData or known Public workspace and copies databases through SQLite backup into a protected staging directory. It checks schema bounds, integrity and source stability before same-volume rename activation. It never copies live WAL files directly, merges populated destinations or deletes originals. Other running review-app processes block migration. A folder picker resolves ambiguity/custom locations. Explicit import closes active connections, refuses a populated target and retains an unused target as backup with rollback on failure. A migration marker records the old path. Linux retains prior local discovery. The updater refuses pending private migration and only handles its public catalog.

```text
private/workspace.sqlite3   Private document references, text caches, preferences, review runs and reviewer actions and folder intake; schema 5
library/catalog.sqlite3    Public sources, versions, domains, originals, and reading text; schema 4
```

The installer detects a registered earlier installation and upgrades its existing installation directory. It replaces known legacy executable/shortcut branding and the uninstall registration. It does not remove original documents or application-data directories.

Document references use canonical paths. Duplicate registration refreshes metadata without adding another entry. Removing an entry cascades to its text cache and saved reports and preserves the original file. Private migration 1 → 2 adds the cache table; 2 → 3 adds review history without replacing existing document records or preferences. Migration 3 → 4 adds separate reviewer actions and preserves original report JSON. Newer database schemas and malformed saved preferences are rejected without destructive replacement.

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

1. Inspect the locally extracted PDF/DOCX text and original document-location references (implemented).
2. Select review domains and collect required local context and evidence, including the period under review.
3. Resolve applicable source and rule versions for that period.
4. Evaluate explicit, versioned rules and produce findings with passages, source provisions, missing facts, and review status.
5. Preserve evaluated domains, coverage, evidence, source versions, and rule versions in reports.

The source/domain layer and the first six labour rules are implemented. Rules use a pinned agency source; the desktop validates raw/text hashes, current document hash, human-confirmed scope and extraction quality before saving a local review. See [rule scope/validation](VALIDATED_CHECKS.md). Additional domain packs and legal-period reconciliation remain planned. Rules must be reviewed data and Rust logic. Money calculations need exact decimal arithmetic. Missing evidence must remain a missing-evidence state.

## Private document reading

The desktop registers documents and reads them on a background thread. Before parsing, the core reads a bounded local snapshot and computes its SHA-256. A matching cache for the current reader engine can be reused only after this content check. A missing/unreadable original does not silently fall back to old cached text. Changed content is extracted again; an additional hash check before saving rejects a source that changed during parsing. This is a point-in-time read, not a watched folder: an original edited after display requires **Read again** or reopening it.

Untrusted parsing runs in a child process of the same executable, entered before UI/catalog initialization. Arguments carry the format only. Bytes go to stdin and a bounded JSON result returns through stdout; stderr is discarded. The child receives no inherited credentials/proxy configuration. No shell, Python, office installation, downloaded model, HTTP client, or API is used. The public library and updater do not depend on `complyph-core` or `complyph-extract`.

The supervisor cancels obsolete jobs, ignores stale results, terminates a worker after 20 seconds, and bounds output at 12 MiB. Linux workers limit address space to 1 GiB, CPU time to 12 seconds, and core dumps to zero. Windows workers use a Job Object with 512 MiB process memory, 12 seconds of user CPU time, and one active process. Failure to apply limits produces a reader error. Parser resource guards limit resource use; the source downloader uses separate AppContainer confinement for file and network access.

Input is limited to 32 MiB. PDFs use pdf-extract/lopdf, keep physical page numbers, reject password protection, and limit pages to 300. Empty text pages are identified; an entirely textless PDF is **Needs OCR**. Columns, font mappings, table layout, images, and reading order can be imperfect. No partial PDF parsing error is reported as fully read text.

DOCX uses ZIP and namespace-aware streaming XML, with no archive extraction to disk. It reads `word/document.xml`, header/footer parts, and foot/endnotes. Locations identify the original part, 1-based paragraph, and table/row/cell where available. Headers/notes are supplementary parts, not inferred page content. Text runs, Unicode, tabs, line breaks, and visible hyperlink labels are preserved. Deleted/moved-from text, text boxes, comments, images, and embedded content are omitted. Field instructions are ignored. Macros, external relationships, and document links are never executed/fetched; DTDs and non-predefined XML entities are rejected.

DOCX limits are 2,048 archive entries, 64 MiB declared expanded bytes, 16 MiB per selected XML part, 128 nesting levels, and 12,000 paragraphs. Both formats stop at 4 MiB of extracted text. Known failures have explicit statuses; a crash or exhausted worker is shown as a local reading failure with the original preserved.

Cache data lives only in `private/workspace.sqlite3`, under a foreign key that deletes it with the document entry. The GUI uses text-only selectable labels, location navigation, search, and bounded display chunks. Legal checks separately require supported scope, reading integrity and pinned source integrity. No compliance result can be inferred from a successful text read.

The developer-only `--read-file PATH --output NEW-JSON-FILE [--data-dir PATH]` route exercises the same worker/cache pipeline without opening a window. It refuses to overwrite an existing file. Installer smoke tests use it only with bundled synthetic data; its JSON output contains private text if a developer deliberately selects a private document.

## Manual source import

Manual source import uses the same bounded child parser as private reading, without registering a private document. Source metadata validation and transactional writes live in `complyph-library`, which has no private-store, parser or HTTP dependency. Public schema 3 adds explicit manual-pending/user-checked provenance. The selected public bytes and location-labelled reading text are saved with their hashes, domains, dates and verification notes; absolute local source paths are omitted. Manual URLs cannot replace bundled entries. New same-URL file hashes append immutable versions; duplicates retain their recorded metadata. User checks never become agency-download verification or activate a rule. See [legal sources](LEGAL_SOURCES.md).

## Connected review navigation

The Review workspace presents the local reader and findings in separate scrolling panes. Evidence actions resolve the exact structured location and whitespace-normalized text against the saved document hash. Matching ranges retain original UTF-8 byte offsets, select the appropriate display chunk and drive highlight/scroll positioning. A different document hash, missing location or unmatched passage disables navigation rather than guessing. Repeated matches are explicit.

Supporting provisions are read by source key, raw hash and reading-text hash from preserved catalog snapshots. Rule mappings select complete article boundaries and related implementing rules; quotation checks prevent opening the wrong provision. The offline viewer also exposes the full saved source. It does not follow an external agency link or replace a historical source with the latest version. Reports can reopen this connected view while retaining their recorded scope and evidence.

## Source update coordinator

Public schema 4 adds explicit URL approvals, per-source attempts/validators/leases, immutable downloaded bytes and review records. No watch is enabled during bundle import. A source's saved reference and its checking-rule pins remain unchanged by downloading or reviewing a candidate. SHA-256 is checked before exporting a downloaded copy or reusing a conditional-comparison body.

The desktop starts the packaged coordinator with the public-library directory only. It polls due watches every minute and checks at most 20 per batch. A lease prevents duplicate concurrent requests; expired interrupted leases can retry. Completed attempts use a 24-hour daily interval; manual checks can retry failures sooner. The desktop owns a kill-on-close job for the coordinator and all descendants.

For each URL, the coordinator starts a resource-limited AppContainer process through STARTUPINFOEX security capabilities and an explicit inherited-handle list. The network child confirms its AppContainer token before reading a request or constructing an HTTP client. It has InternetClient capability, no private-network/library capabilities, an executable directory with read/execute access only and no catalog path. Its framed pipe protocol carries a small public request, a bounded response header and at most 32 MiB of body bytes. HTTPS uses Rustls/WebPKI; credentials, proxies, cookies and request bodies are excluded. The resolver rejects private/reserved addresses and passes the checked addresses directly to the connector. Redirects are same-origin HTTPS and limited to three. The parent verifies URL origin, payload type, size and hash before committing a change.

Other-platform builds fail closed rather than creating an unconfined downloader. Source checks inspect selected URL bytes; they do not crawl other publications or establish legal currency. See [source updates](SOURCE_UPDATES.md).

## Reviewer tracking and report export

Private schema 4 stores `review_actions` separately from original `review_runs.value` JSON. Each action records a self-reported reviewer name, required note, time, optional rule ID and before/after status. The default human status is Unresolved, independent of the original automated outcome. Notes apply to one saved review; new reviews never inherit resolved statuses. Corrections and reopened issues append new entries. Deleting a document cascades through its reviews and tracking, preserving the original file.

A fingerprint hashes the stored original JSON bytes and saved row identifiers/timestamp, so deserialization defaults do not invalidate historical reports and reused row IDs cannot accept stale actions. An immediate transaction checks that fingerprint and the latest revision for the targeted document-note/finding stream before appending. A stale editor must load and review the latest state; another finding’s update does not block an independent action. SQLite triggers reject in-place action changes and updates to a tracked review. This history is not an authenticated signature or tamper-proof log against a machine owner who can alter the database.

PDF export captures the original review, document name and all actions in one read transaction. The renderer uses `lopdf` and embedded Lato TrueType fonts with explicit Unicode/GID mappings. A4 layout wraps text and paginates; limits are 200 pages and 8 MiB of input text. Unsupported glyphs fail explicitly, with complete JSON available as the alternative. An export worker receives only the captured DTO and destination path; it never opens the original or a legal catalog. A temporary file in the destination directory is completed and synced before new-file persistence; existing paths are refused. PDF strings are encoded as text, without actions, links, attachments, scripts or external resources. PDF reports identify the saved scope, evidence, legal quote, source/rule hashes and history cutoff; they do not establish current legal currency.

The diagnostic routes `--review-state`, `--review-action` and `--export-review-pdf` exercise the same store/renderer as the native controls. JSON metadata is bounded and rejects unknown fields; a new output is required before an action is written. Normal use needs no command prompt. See [reports](REPORTS.md).

## OCR and watched intake

Private schema 5 adds a single opt-in folder configuration and path/signature/hash intake ledger. Polling is nonrecursive and performed off the UI thread. Stable-file observations use monotonic time, a ten-second quiet period, completion markers, writer-excluding Windows opens and bounded/fair batches. Transactions recheck the configuration generation before registration; a paused/replaced/forgotten watch cannot commit queued work under an obsolete generation. Content duplicates are checked against live local files. Removal retains an unchanged-path tombstone; forgetting intake clears it. No review/OCR is started by intake.

OCR uses the existing pipe/supervisor model with 90 CPU/120 elapsed seconds, 20 PDF pages and 2,800 pixels per side. PDFium renders from memory without V8/XFA. Tesseract initializes from hash-checked English model bytes and consumes RGB pixels through fixed C APIs; no document filename/URL/configuration is passed to it. The Windows build excludes curl/archive features and uses a static CRT. The Rust application orchestrates native C/C++ libraries with their bundled licences. Recognition metadata and byte-aligned uncertainty ranges are validated before caching. Rule engine 4 requires explicit OCR confirmation, complete page text and supported provenance; the five earlier legal thresholds are unchanged. Historical review JSON is not rewritten. See [implementation and limits](OCR_AND_FOLDERS.md).

## Service incentive leave

Rule engine 4 adds `PH-LAB-095-01`, comparing only explicit annual paid SIL days with five, under reviewer-established employee coverage, qualifying service and exception facts plus a nonempty bounded basis. The three facts default independently to unknown; an established exclusion prevents a threshold conclusion. They are assertions rather than automated eligibility findings. A separate conservative sentence matcher retains semicolons and wrapped lines so attached conditions cannot become unconditional grants. Written day values zero through ten and exact hundredths are supported. Vacation pools, unpaid grants, other periods and conflicting/unsupported clauses remain unresolved.

The native form, saved review and PDF show the three facts and recorded basis. The offline viewer maps the unchanged pinned source to Article 95, Article 82 and the complete Book III Rule V. No source pin, public bundle or database schema changes. Missing SIL context in historical JSON remains unknown; loading never adds new findings or rewrites original report bytes. See [SIL source assessment](SERVICE_INCENTIVE_LEAVE.md).
