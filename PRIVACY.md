# Privacy

## ComplyPH 0.2.0

ComplyPH runs locally. Registering a private PDF/DOCX stores its filename, canonical path, size, and modification time in the private SQLite workspace. Preferences use the same workspace. This release does not extract private document text, evaluate compliance, or generate reports.

The application and catalog-status tool contain no HTTP client, telemetry, advertising, AI integration, document upload, or runtime source downloader.

## Public reference library

The separate public catalog stores 14 official publication snapshots, their original bytes, derived reading text, domains, citations, dates, integrity hashes, and provenance. The bundle was acquired from public official publication channels during development. No private documents or document-derived queries were used for acquisition.

The app renders reading text only. Archived source HTML is not executed or rendered as a webpage. Saving an original exports that public source locally without overwriting an existing file. Other software used to open exported originals operates independently.

Source verification does not establish complete amendments, current applicability, OCR accuracy, or compliance. Known gaps and source-specific limits are visible in the library and inventory.

## Storage and deletion

Fresh installations use `%LOCALAPPDATA%\ComplyPH`. An existing `%LOCALAPPDATA%\LaborLens PH` data root is reused in place. If both roots exist, the app preserves both and asks for explicit selection. It does not move live databases.

Original private documents remain at their selected locations. Removing a document entry deletes only the reference. Moving an original can make its saved reference unavailable. Uninstall preserves the workspace, source catalog, and originals.

SQLite files are not encrypted by the app. Access depends on operating-system permissions and device protection. Unix private directories/files receive restrictive permissions. Windows uses the user's application-data directory.

## Future updater boundary

The future updater will operate on the public source catalog without a dependency on the private workspace crate. Shared location discovery reads directory existence only.

Crate separation is not an operating-system sandbox. Enforced file/network restrictions, approved-source validation, and runtime verification are required before enabling networking. Private document content, identifiers, paths, hashes, employee/employer/payroll data, findings, and derived search terms must never be updater inputs.

Backups, cloud-sync folders, endpoint tools, browsers, and the operating system act independently of ComplyPH. All bundled sample HR documents are synthetic.
