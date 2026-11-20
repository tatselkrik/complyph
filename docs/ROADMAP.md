# Roadmap

Status reflects ComplyPH 0.7.0. Checked items are available; unchecked items are planned. Future items do not have assigned release dates.

## Available in 0.7.0

- [x] Local PDF/DOCX extraction with source locations, content-hash caching and bounded worker processes.
- [x] Side-by-side document and findings panes, highlighted evidence navigation and saved review history.
- [x] Complete supporting provisions and related rules from the exact historical local source.
- [x] Five focused labour numeric checks: normal daily working hours, ordinary-day overtime premium, night shift differential, regular meal periods and consecutive weekly rest, with explicit applicability and evidence requirements.
- [x] Nineteen preserved references across seven compliance domains, with provenance and coverage gaps.
- [x] Approved-URL source checks on Windows, daily while open; immutable downloaded-change history, change flags, local export and source-review notes.
- [x] Manual PDF/DOCX legal-source intake with official citation URLs, domains, dates, user verification notes and immutable version history.
- [x] Saved local review reports, connected reopening and report JSON copying/diagnostic export.
- [x] Windows installation, workspace migration, protected `C:\ProgramData\ComplyPH` storage and preserving uninstall.

## Remaining work

### 1. More validated checks

Only the five labour comparisons listed above are implemented. Other saved laws are reference material awaiting checking rules.

- [x] First labour expansion in 0.7.0: meal-period and weekly-rest comparisons, exception-aware scope controls, exact offline provisions, boundary/ambiguity tests and historical-report compatibility.
- [ ] Expand labour coverage first, beyond the current five numeric comparisons.
- [ ] Add reviewed and tested DOLE OSHS checks.
- [ ] Add reviewed and tested privacy/NPC checks.
- [ ] Add BIR/tax and SSS, PhilHealth and Pag-IBIG contribution/compliance checks, including applicable periods and versioned rates or tables.

Each new check requires reviewed source provisions, applicability and exception handling, effective-period handling, evidence requirements, exact offline citations and independently expected validation cases. Importing a law or downloading changed bytes does not create or activate a rule. See [current checks and validation](VALIDATED_CHECKS.md).

### 2. Better reports and review tracking

Saved reports and JSON copying/diagnostic export are available. The following report features remain planned. Existing source-review notes concern downloaded legal references; they are separate from document/finding review notes.

- [ ] Export readable PDF reports with findings, evidence, scope, citations and document/source/rule versions.
- [ ] Add reviewer notes to document reviews and individual findings.
- [ ] Track issue statuses such as **unresolved**, **acknowledged** and **resolved**, with a history of reviewer actions.
- [ ] Keep human review statuses separate from the original automated finding and its preserved evidence.
- [ ] Compare revised document versions and retain the review history for each version.

### 3. Local OCR and folder monitoring

The current reader extracts supported PDF/DOCX text and identifies pages that need OCR. OCR processing and watched-folder intake are not implemented.

- [ ] Read scanned documents using local OCR, with page locations, visible quality limits and confirmation of uncertain text.
- [ ] Improve layout, font and embedded-content reading coverage.
- [ ] Add a folder picker for local watched-folder intake.
- [ ] Wait for completed file writes, preserve originals and avoid rechecking unchanged content.
- [ ] Keep watched-folder processing local; it must never trigger an AI upload automatically.

### 4. Optional AI review

AI review is planned and is not included in 0.7.0. It is a separate action for a selected document, with an explicit disclosure and send step.

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
