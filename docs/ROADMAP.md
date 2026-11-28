# Roadmap

Status reflects ComplyPH 0.15.0. Checked items are available; unchecked items are planned. Future items do not have assigned release dates.

## Available in 0.15.0

- [x] Local PDF/DOCX extraction with source locations, content-hash caching and bounded worker processes.
- [x] Side-by-side document and findings panes, highlighted evidence navigation and saved review history.
- [x] Complete supporting provisions and related rules from the exact historical local source.
- [x] Twenty focused numeric checks across seven domains, with confirmed applicability/periods, evidence, exact per-finding sources and preserved rule-pack identity.
- [x] Nineteen preserved references across seven compliance domains, with provenance and coverage gaps.
- [x] Approved-URL source checks on Windows, daily while open; immutable downloaded-change history, change flags, local export and source-review notes.
- [x] Manual PDF/DOCX legal-source intake with official citation URLs, domains, dates, user verification notes and immutable version history.
- [x] Saved local reviews, connected reopening, document/finding notes, Unresolved/Acknowledged/Resolved follow-up history, offline PDF export and full-report JSON copying.
- [x] Local PDF OCR with bundled Windows runtime and a separate accuracy gate.
- [x] Optional local watched-folder intake with pause/resume, stability checks, duplicate detection and persisted local history.
- [x] Windows installation, workspace migration, protected `C:\ProgramData\ComplyPH` storage and preserving uninstall.
- [x] Initial desktop-quality pass: compact layouts, bounded dialogs, keyboard shortcuts/focus, labelled edits and actual-application recovery tests.

## Six work areas

### 1. Initial validated checks across seven domains — complete technically

- [x] Complete the finite [initial twenty-check checklist](INITIAL_CHECKS.md): seven labour comparisons, two OSHS, one privacy/NPC, one BIR, three SSS, three PhilHealth and three Pag-IBIG comparisons.
- [x] Add coverage, exception, review-period and factual-basis gates for the fourteen new checks, including the mutually exclusive Pag-IBIG employee bands.
- [x] Preserve exact source hashes and complete offline provisions per finding, rule-pack identity, historical reviews and independently expected boundary/document cases.

This completes the initial technical milestone, not full coverage of each domain. Qualified legal review remains under item 6. Further labour coverage, OSHS staffing/facility matrices, privacy/security requirements, full tax/contribution calculations, remittance deadlines and special regimes are future expansion. Profiles beyond the supported CY2026 boundary require separate assessment. See [scope and source assessment](INITIAL_CHECKS.md).

### 2. Better reports and review tracking — implemented

PDF reports, document/finding notes and status history have been available since 0.8.0. Existing source-review notes concern downloaded legal references; they remain separate from document/finding notes. See [reports and tracking](REPORTS.md).

- [x] Export readable PDF reports with findings, evidence, scope, citations and document/source/rule versions.
- [x] Add reviewer notes to document reviews and individual findings.
- [x] Track issue statuses such as **unresolved**, **acknowledged** and **resolved**, with a history of reviewer actions.
- [x] Keep human review statuses separate from the original automated finding and its preserved evidence.
- [x] Compare revised document versions and retain the review history for each version.
- [x] Retain extracted readings for overwritten/moved originals, explicitly link/unlink revisions and bind new reviews to exact reading fingerprints.
- [x] Compare findings with scope/source/method differences visible; keep human statuses separate and export comparison PDF/full JSON.

The complete reporting milestone is available in 0.13.0. Comparison covers retained extracted text; images/layouts and legal meaning are not inferred. Missing historical readings remain explicitly unavailable. Broader extraction coverage belongs to item 3 and legal/desktop quality review to item 6.

### 3. Local OCR, folder monitoring and initial reading quality — implemented

Local PDF OCR and optional nonrecursive folder intake are implemented. The finite [supported reading-quality profile](READING_QUALITY.md), with twelve synthetic fixtures, is available in 0.14.0. See [controls and limits](OCR_AND_FOLDERS.md). Folder intake adds document entries; reading, OCR, scope confirmation and review remain deliberate actions.

- [x] Read scanned PDF documents using local OCR, with page locations, low-confidence highlights, visible quality limits and separate text confirmation before checking.
- [x] Improve the defined initial layout/font/embedded-content coverage: spatial PDF paragraphs/two-column text, labelled Word text boxes and alternatives, Unicode mappings, OCR rotation/contrast/mild-skew/layout controls, and explicit manual gates for unresolved content.
- [x] Preserve exact reading warnings/settings across reruns, histories, comparisons and local report exports.
- [x] Add a folder picker for local watched-folder intake.
- [x] Wait for stable writes/completion markers, preserve originals and avoid duplicate/unchanged intake; numeric reviews are not automatic.
- [x] Keep watched-folder processing local; it must never trigger an AI upload automatically.

### 4. Optional AI review

AI review is planned and is not included in 0.15.0. It is a separate action for a selected document, with an explicit disclosure and send step.

- [ ] Keep **offline review as the default**.
- [ ] Add a separate **AI review** action for each document.
- [ ] Prepare an editable local preview of exactly what will be sent, including approved document passages, instructions and supporting legal excerpts.
- [ ] Offer local redaction of names and identifiers, with manual corrections before submission. Redaction assistance must not promise that every sensitive detail has been removed.
- [ ] Require an explicit **Send to OpenAI** action for the approved preview; sign-in, document import and folder monitoring must not send document content.
- [ ] Integrate the documented **Sign in with ChatGPT** plan-usage route, including eligible Plus accounts, subject to preview availability, account/workspace policy and shared usage limits. Protect credentials and show connection/usage-limit failures without silently switching to paid API billing.
- [ ] Display AI results inside ComplyPH, clearly separated from validated rule findings, and check supporting citations against the saved legal references.
- [ ] Retain a local audit record of the approved submission and AI response, model and source versions; apply appropriate data-handling and retention information before enabling the feature.

The interface must make the privacy choice clear:

- **Offline review — document stays on this device.**
- **AI review — approved text is shared with OpenAI.**

Sending excerpts and redacting identifiers reduce disclosure; approved text still leaves the PC. Reduced context can also limit the AI review. This integration makes its own requests and does not automatically post into an existing ChatGPT/Codex conversation or access conversation history or memories. See OpenAI's [plan-usage integration](https://developers.openai.com/siwc/token-sharing-open-source) and [eligibility and usage controls](https://learn.chatgpt.com/docs/sign-in-with-chatgpt).

### 5. Library maintenance — implemented

The complete maintenance workflow is available in 0.12.0. See [library maintenance](LIBRARY_MAINTENANCE.md) for the reviewed actions, supported discovery scope and privacy boundary.

- [x] Edit version-bound source metadata, record identity/verification decisions and preserve their reviewer/date/basis history.
- [x] Compare candidate and historical reference versions in-app; deliberately adopt or restore reviewed versions while preserving every earlier snapshot and historical report citation.
- [x] Inventory qualifying issuance links from approved official publication pages, distinguish initial inventory from later discoveries and route citations to local source intake or rule-impact review.
- [x] Track exact rule/source dependencies, hold affected checks, require unchanged-support/scope/evaluation gates for revalidation, retain legal-change holds across unrelated edits/upgrades and bind future findings to the adopted source.
- [x] Add Linux Landlock ABI 3+/seccomp isolation alongside Windows AppContainer; fail closed on unsupported kernels/platforms.

Discovery is a bounded official-page link inventory, not an exhaustive legal search or automatic effectivity determination. A changed requirement still needs a newly implemented and tested rule profile; source adoption does not rewrite thresholds. Qualified Philippine legal review remains under item 6. Private document data never becomes source-updater input.

### 6. Validation and desktop quality — partly implemented

- [x] Complete the finite [initial desktop-quality profile](DESKTOP_QUALITY.md): twelve size/scaling combinations, compact review, bounded dialogs, keyboard focus/shortcuts, safe removal defaults and labelled edits.
- [x] Add actual-application missing/malformed-file retry and history-preservation cases, plus late-reader-reply and palette-contrast regressions. Require the installed Windows executable to pass the headless UI profile before publication.

- [ ] Broaden representative synthetic cases, including paraphrases, exceptions, ambiguous wording, missing evidence and extraction failures; investigate missed issues and false flags.
- [ ] Obtain qualified Philippine legal review of checking rules and their supporting interpretations; technical tests and legal review remain distinct.
- [ ] Evaluate any future AI review against independently reviewed cases and verify citation accuracy before making reliability claims.
- [ ] Continue target-device checks for Windows scaling, accessibility and document drag/drop.
