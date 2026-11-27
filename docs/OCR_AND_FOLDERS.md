# Local OCR and watched folders

ComplyPH supports recognition for scanned PDFs and optional folder intake. Both features process documents locally. The Windows installer/ZIP includes the OCR runtime and English recognition data. No OCR service, account, API key, subscription, separate installation command or runtime model download is used.

## Read a scanned PDF

1. Open a PDF in **Review**. The ordinary reader still uses selectable text first. Expand **OCR options** to choose page rotation, layout and scan enhancement; settings apply to every page.
2. Select **Run local OCR** to recognize every page, including pages in mixed text/image documents.
3. Inspect the recognized text at its original physical page number. Low-confidence words receive amber highlights and a page count. Selecting a finding highlights its evidence instead.
4. Compare every page with the original, including all numbers, table relationships and operative terms. High recognition confidence is not proof of accuracy; a wrong number may be unflagged.
5. In the checking scope, confirm both the ordinary accuracy/operative-term statement and the separate **I checked every OCR page…** statement. Establish each rule’s applicability before running a review.

See [supported layouts and OCR options](READING_QUALITY.md) for the initial reading-quality profile, metadata/history behavior and difficult-document limits. The rule engine independently blocks numeric conclusions when OCR confirmation is missing, provenance is unsupported, a page has no recognized text or other reading warnings remain. Confirmation records a human assertion; the app cannot verify that assertion. If text is wrong or incomplete, use a better original/corrected local document or manual review rather than confirming inaccurate text.

Recognized text is cached privately against the original file hash. Reopening an unchanged file reuses its OCR reading. **Read again** repeats the cached reading method; **Run local OCR again** explicitly reruns recognition. A changed file needs a new accurate reading and a new review. Previous findings, notes and citations remain historical snapshots.

OCR limits are **20 PDF pages**, **32 MiB input**, **4 MiB recognized text**, a **2,800-pixel maximum page side**, **90 seconds of worker CPU time** and **120 seconds elapsed time**. Pages are rendered at up to 200 DPI, reduced to fit the pixel bound. Recognition uses English Tesseract data and is not a guarantee for handwriting, other scripts, faint scans, complex tables, stamps, signatures or page layout. Empty pages remain unestablished. Split longer scanned PDFs locally before processing. Password-protected PDFs require an unprotected local copy; OCR does not request a password or bypass encryption.

The original PDF is never changed. OCR produces a private reading cache, not a rewritten/searchable PDF. Public legal-source intake still uses its selectable-text reader; this feature applies to private document review. The twenty initial comparisons in engine `complyph-rules-1` retain the OCR input gate and recognition metadata. The additional scope/date/basis gates apply independently; recognizing text never establishes eligibility.

## Watch an intake folder

Open **Settings → Watched document folder → Choose intake folder…** and choose a local folder outside the ComplyPH workspace and its parent folders. A checkbox pauses/resumes intake. No folder is enabled by default.

While the app is open, a background scan runs about every five seconds. It examines this folder only, waits for at least ten seconds without a detected size/modified-time change, checks for file-completion markers and retries files that are still changing or locked. On Windows, a file must also permit an open that excludes other writers. These checks reduce partial imports; a quiet interval cannot guarantee that another program will never resume writing later. The normal bounded reader still checks file validity when a document is opened.

Supported files are local PDF/DOCX documents up to 32 MiB. Temporary/lock filenames, symbolic/reparse links, nested folders and oversized files are skipped. Windows network shares/mapped network drives are refused. Folders over 10,000 entries need a smaller intake folder. A fair rotating batch processes at most eight mature files per scan.

Content hashes avoid duplicate copies already represented by an unchanged local document. The private intake ledger remembers each observed path/signature/hash across restarts. A changed file at an existing path refreshes its document entry; it does not overwrite its historical reviews. Missing/moved originals are not silently relinked. Folder scans register document entries only: **they do not run OCR, create reviews, infer applicability or send anything to AI**. Open new/changed entries from **Documents** to read and review them deliberately. Intake does not switch away from a review you are working on.

Closing the app stops polling. At the next launch, the selected folder is scanned again with a fresh stability interval. Removing a document entry leaves that unchanged intake path excluded; changing the file can make it eligible again. The ledger retains local paths, signatures and hashes for this purpose, even after cached text/reviews are removed. **Forget folder and intake history** disables intake and clears that ledger while retaining imported document entries and originals. Choosing the folder again starts a new intake history. SQLite deletion is not secure erasure; backups/journals may retain bytes.

## Storage and implementation

The workspace stays at `C:\ProgramData\ComplyPH` on Windows with its existing owning-account protections. Private schema 5 adds watched-folder configuration and intake history without rewriting saved reviews. Updates/uninstall preserve the workspace and originals. The uninstaller removes only known bundled OCR files; user-added files are preserved.

The native Rust app supervises a separate local worker. The worker receives document bytes through a pipe and uses fixed C-API calls into bundled PDFium/Tesseract libraries, with memory/CPU/time limits and cancellation. PDFium renders from memory with V8/XFA disabled. Tesseract receives memory bitmaps and the hash-pinned English model; its Windows build disables curl/URL input and archive downloads. No document path, URL, external relationship, macro or script is passed to those APIs. Temporary page images are not written to disk. OCR resource limits are not a full OS filesystem sandbox.

Runtime components are PDFium build 8165, Tesseract 5.5.4 on Windows and hash-pinned `tessdata_fast` English data (SHA-256 `7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2`). The runtime manifest and bundled `OCR/licenses` directory retain component hashes/notices. Application code remains Rust; these native OCR/rendering dependencies contain C/C++ and retain their own licences. Tesseract/data use Apache 2.0; PDFium and dependencies have their included terms.

## Build and verify

Windows developers run `scripts/prepare-ocr-windows.ps1` before OCR tests/packaging. It builds pinned Tesseract through a pinned vcpkg checkout with static CRT, downloads hash-checked PDFium/English data and stages licences in `target/ocr-runtime`. Set `COMPLYPH_OCR_DIR` to that directory for debug builds/tests. Release builds load the sibling `OCR` folder. Public Windows packages already contain it.

Linux x64 development requires the system Tesseract 5 library. Run `bash scripts/prepare-ocr-linux.sh`, then set `COMPLYPH_OCR_DIR` to `target/ocr-runtime`. The script downloads only public, pinned runtime resources; it is a developer setup step, not an app runtime updater. Other architectures need a supported native runtime rather than silently fetching one.

The synthetic OCR cases, recognition provenance/gate tests and real executable watched-folder tests run with `cargo test --workspace --locked`. Installed Windows verification repeats OCR on three fixtures, verifies missing/present human confirmation, and checks partial-write/duplicate intake with no automatic review. Ordinary reader/checking fixtures remain separately validated.
