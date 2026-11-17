# Privacy

## ComplyPH 0.5.0

ComplyPH runs locally. Selecting a PDF/DOCX registers its filename, canonical path, size, and modification time, then reads supported text in a local child process. Extracted text, paragraph/page/table locations, the original’s SHA-256 hash, reading state, and read date are saved in the private SQLite workspace. Preferences use the same workspace. The private workspace also stores human-confirmed scope and review reports containing passages, outcomes, source/rule identifiers and hashes. The three numeric checks are local and do not certify compliance.

Document bytes pass from the app to its own worker through local stdin/stdout pipes. Worker arguments contain only the document format; inherited credentials and proxy settings are cleared. The readers do not fetch document hyperlinks or external XML entities, execute fields, macros, PDF JavaScript, or embedded programs. Parser diagnostics are not logged. Resource limits and cancellation protect responsiveness; this is not a full operating-system sandbox.

The application and catalog-status tool contain no HTTP client, telemetry, advertising, AI integration, document upload, or runtime source downloader.

## Public reference library

The separate public catalog stores 19 preserved references: 14 agency downloads, 3 identity-checked uploads and 2 provisional third-party copies. The catalog stores their original bytes, derived reading text, domains, citations, dates, integrity hashes, and provenance. Provenance is classified individually; mirrors and stitched social-media images are not agency-original verification. No private documents or document-derived queries were used for acquisition.

Legal library → Add legal source explicitly copies a selected public PDF/DOCX into the local catalog, with derived reading text, user-provided official citation, domains, dates and verification notes. This action does not register a private document or write to the private workspace. The source URL is not fetched. Local paths are not stored in source metadata. Pending imports have no verification timestamp; user identity checks remain separate from agency-download verification. Same-URL revisions preserve earlier snapshots; identical files do not rewrite metadata.

The app renders reading text only. Archived source HTML is not executed or rendered as a webpage. Saving an original exports that public source locally without overwriting an existing file. Other software used to open exported originals operates independently.

Source verification does not establish complete amendments, current applicability, OCR accuracy, or compliance. Known gaps and source-specific limits are visible in the library and inventory.

## Storage and deletion

Windows uses `%ProgramData%\ComplyPH`, normally `C:\ProgramData\ComplyPH`. The owning account, SYSTEM and Administrators receive access; the protected ACL excludes general public/users access. It is a machine path for a single owning account, not an all-users document store. The app refuses to take another account's ownership. Linux retains prior local-root discovery.

The desktop copies a single detected legacy workspace with SQLite backup, including committed WAL data, checks integrity and activates a staged copy. Old databases and original documents remain in place. A folder picker handles custom/ambiguous sources. Explicit import into an unused destination retains its empty databases as backup; populated destinations cannot be replaced or merged. The migration marker records the old local path privately. The public updater does not migrate private data.

Original private documents remain at their selected locations. Removing a document entry deletes its reference, cached text and saved reports together. SQLite deletion is not a promise of secure erasure; database journals, backups, and storage media can retain recoverable bytes. Moving an original can make its saved reference unavailable. The cache is validated by content hash whenever a document is opened; use Read again after editing a document already on screen. Uninstall preserves the workspace, extracted text, source catalog, and originals. A developer diagnostic mode can explicitly export a private reading result to a new local JSON file; GUI report copying places private JSON on the clipboard only when explicitly requested. The check diagnostic exports a saved review to a new local JSON file.

SQLite files are not encrypted by the app. Access depends on operating-system permissions and device protection. Unix private directories/files receive restrictive permissions. Windows workspace directories use explicit owner-only access alongside SYSTEM and Administrators. Existing application data is not encrypted or securely erased by migration.

## Future updater boundary

The future updater will operate on the public source catalog without a dependency on the private workspace crate. Shared location discovery reads directory/file existence and Windows process names only; no document/database contents.

Crate separation is not an operating-system sandbox. Enforced file/network restrictions, approved-source validation, and runtime verification are required before enabling networking. Private document content, identifiers, paths, hashes, employee/employer/payroll data, findings, and derived search terms must never be updater inputs.

Backups, cloud-sync folders, endpoint tools, browsers, and the operating system act independently of ComplyPH. All bundled sample HR documents are synthetic.
