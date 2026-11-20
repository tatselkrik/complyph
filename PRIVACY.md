# Privacy

## ComplyPH 0.7.0

ComplyPH runs locally. Selecting a PDF/DOCX registers its filename, canonical path, size, and modification time, then reads supported text in a local child process. Extracted text, paragraph/page/table locations, the original’s SHA-256 hash, reading state, and read date are saved in the private SQLite workspace. Preferences use the same workspace. The private workspace also stores human-confirmed scope and review reports containing passages, outcomes, source/rule identifiers and hashes. The five numeric checks are local and do not certify compliance.

Document bytes pass from the app to its own worker through local stdin/stdout pipes. Worker arguments contain only the document format; inherited credentials and proxy settings are cleared. The readers do not fetch document hyperlinks or external XML entities, execute fields, macros, PDF JavaScript, or embedded programs. Parser diagnostics are not logged. Resource limits and cancellation protect responsiveness; this is not a full operating-system sandbox.

The desktop, document reader and rule engine contain no HTTP client, telemetry, advertising, AI integration or document upload. The separate updater uses a Windows AppContainer child for approved source downloads. Status queries and source import make no online request.

## Public reference library

The separate public catalog stores 19 preserved references: 14 agency downloads, 3 identity-checked uploads and 2 provisional third-party copies. The catalog stores their original bytes, derived reading text, domains, citations, dates, integrity hashes, and provenance. Provenance is classified individually; mirrors and stitched social-media images are not agency-original verification. No private documents or document-derived queries were used for acquisition.

Legal library → Add legal source explicitly copies a selected public PDF/DOCX into the local catalog, with derived reading text, user-provided official citation, domains, dates and verification notes. This action does not register a private document or write to the private workspace. Importing does not fetch the source URL. A separate explicit URL approval enables source update checks. Local paths are not stored in source metadata. Pending imports have no verification timestamp; user identity checks remain separate from agency-download verification. Same-URL revisions preserve earlier snapshots; identical files do not rewrite metadata.

The app renders reading text only. Archived source HTML is not executed or rendered as a webpage. Saving an original exports that public source locally without overwriting an existing file. Other software used to open exported originals operates independently.

Source verification does not establish complete amendments, current applicability, OCR accuracy, or compliance. Known gaps and source-specific limits are visible in the library and inventory.

## Storage and deletion

Windows uses `%ProgramData%\ComplyPH`, normally `C:\ProgramData\ComplyPH`. The owning account, SYSTEM and Administrators receive access; the protected ACL excludes general public/users access. It is a machine path for a single owning account, not an all-users document store. The app refuses to take another account's ownership. Linux retains prior local-root discovery.

The desktop copies a single detected legacy workspace with SQLite backup, including committed WAL data, checks integrity and activates a staged copy. Old databases and original documents remain in place. A folder picker handles custom/ambiguous sources. Explicit import into an unused destination retains its empty databases as backup; populated destinations cannot be replaced or merged. The migration marker records the old local path privately. The public updater does not migrate private data.

Original private documents remain at their selected locations. Removing a document entry deletes its reference, cached text and saved reports together. SQLite deletion is not a promise of secure erasure; database journals, backups, and storage media can retain recoverable bytes. Moving an original can make its saved reference unavailable. The cache is validated by content hash whenever a document is opened; use Read again after editing a document already on screen. Uninstall preserves the workspace, extracted text, source catalog, and originals. A developer diagnostic mode can explicitly export a private reading result to a new local JSON file; GUI report copying places private JSON on the clipboard only when explicitly requested. The check diagnostic exports a saved review to a new local JSON file.

SQLite files are not encrypted by the app. Access depends on operating-system permissions and device protection. Unix private directories/files receive restrictive permissions. Windows workspace directories use explicit owner-only access alongside SYSTEM and Administrators. Existing application data is not encrypted or securely erased by migration.

## Source update boundary

No URL is approved by default. The user approves an official HTTPS publication URL inside the local source reader. The updater checks those URLs daily while the desktop is open, with an immediate check button; a closed app does not schedule requests. Other-platform builds keep update networking disabled.

The public-catalog coordinator has no private-workspace or extraction dependency. Its network child receives only an approved source URL and optional ETag/Last-Modified values previously returned by that public server. Requests use GET without a body. Agencies see the approved URL, normal IP/TLS connection metadata, the app user-agent and their own cache validators. Document contents, paths, hashes, findings, employer/employee data and source-review notes are never request inputs. The app does not send search queries derived from documents.

The Windows child runs in an AppContainer with Internet-client capability and a read/execute-only temporary copy of its executable. It receives an explicit list of pipe handles, without inherited catalog/document handles. The owning process's private database, catalog and standard original-document paths are denied by the AppContainer; Windows tests probe synthetic canaries. No private-network capability is granted. The HTTP layer also requires supported agency domains, HTTPS on port 443, public DNS addresses and same-origin redirects. Proxy credentials, cookies and HTTP authentication are not used. TLS certificate validation stays enabled. Environment/service credentials are not inherited. Job objects bound memory, CPU, child count and lifetime; closing the desktop terminates its updater processes.

The worker cannot open a catalog or choose a document. The coordinator stores downloaded public bytes, source hashes, response validators, attempts/failures and review notes in public schema 4. The private schema stays at 3. New downloads remain reviewable copies rather than automatic legal verification or rule updates. Both source imports and update exports preserve original files and refuse to overwrite a chosen existing destination.

The AppContainer profile is an account-local Windows security profile named ComplyPH.SourceUpdates. A temporary executable directory contains application code only and is removed after a batch. Backups, external programs, operating-system services and user changes to file-sharing permissions operate independently of ComplyPH. All test documents and privacy canaries are synthetic. See [source update limits and validation](docs/SOURCE_UPDATES.md).
