# Roadmap

Status reflects ComplyPH 0.11.0. Checked items are available; unchecked items are planned. Future items do not have assigned release dates.

## Available in 0.11.0

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

## Remaining work

### 1. Initial validated checks across seven domains — complete technically

- [x] Complete the finite [initial twenty-check checklist](INITIAL_CHECKS.md): seven labour comparisons, two OSHS, one privacy/NPC, one BIR, three SSS, three PhilHealth and three Pag-IBIG comparisons.
- [x] Add coverage, exception, review-period and factual-basis gates for the fourteen new checks, including the mutually exclusive Pag-IBIG employee bands.
- [x] Preserve exact source hashes and complete offline provisions per finding, rule-pack identity, historical reviews and independently expected boundary/document cases.

This completes the initial technical milestone, not full coverage of each domain. Qualified legal review remains under item 6. Further labour coverage, OSHS staffing/facility matrices, privacy/security requirements, full tax/contribution calculations, remittance deadlines and special regimes are future expansion. Profiles beyond the supported CY2026 boundary require separate assessment. See [scope and source assessment](INITIAL_CHECKS.md).

### 2. Better reports and review tracking

PDF reports, document/finding notes and status history have been available since 0.8.0. Existing source-review notes concern downloaded legal references; they remain separate from document/finding notes. See [reports and tracking](REPORTS.md).

- [x] Export readable PDF reports with findings, evidence, scope, citations and document/source/rule versions.
- [x] Add reviewer notes to document reviews and individual findings.
- [x] Track issue statuses such as **unresolved**, **acknowledged** and **resolved**, with a history of reviewer actions.
- [x] Keep human review statuses separate from the original automated finding and its preserved evidence.
- [ ] Compare revised document versions and retain the review history for each version.

### 3. Local OCR and folder monitoring

Local PDF OCR and optional nonrecursive folder intake are available in 0.11.0. See [controls and limits](OCR_AND_FOLDERS.md). Folder intake adds document entries; reading, OCR, scope confirmation and review remain deliberate actions.

- [x] Read scanned PDF documents using local OCR, with page locations, low-confidence highlights, visible quality limits and separate text confirmation before checking.
- [ ] Improve layout, font and embedded-content reading coverage.
- [x] Add a folder picker for local watched-folder intake.
- [x] Wait for stable writes/completion markers, preserve originals and avoid duplicate/unchanged intake; numeric reviews are not automatic.
- [x] Keep watched-folder processing local; it must never trigger an AI upload automatically.

### 4. Optional AI review

AI review is planned and is not included in 0.11.0. It is a separate action for a selected document, with an explicit disclosure and send step.

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

### 5. Library maintenance

Approved-URL checks, retained downloads and source-review notes are already implemented. The remaining work concerns assessing and adopting changes.

- [ ] Add source metadata editing and deliberate source-verification workflows.
- [ ] Add in-app comparison and reviewed promotion of downloaded reference candidates, preserving previous snapshots and historical report citations.
- [ ] Discover new issuances through approved official publication pages, beyond checking the bytes at selected URLs.
- [ ] Track reviewed legal changes that require particular checking rules to be revalidated before an updated rule is activated.
- [ ] Expand downloader isolation support beyond Windows while keeping unsupported platforms' networking disabled.

Retain the source updater's AppContainer boundary and public-only request protocol. Private document data must never become source-updater inputs. Optional AI submission uses its own explicit consent and data boundary.

### 6. Validation and desktop quality

- [ ] Broaden representative synthetic cases, including paraphrases, exceptions, ambiguous wording, missing evidence and extraction failures; investigate missed issues and false flags.
- [ ] Obtain qualified Philippine legal review of checking rules and their supporting interpretations; technical tests and legal review remain distinct.
- [ ] Evaluate any future AI review against independently reviewed cases and verify citation accuracy before making reliability claims.
- [ ] Continue target-device checks for Windows scaling, accessibility and document drag/drop.
