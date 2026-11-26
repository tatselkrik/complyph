# Reports, document versions and follow-up

ComplyPH 0.13.0 retains document readings and keeps each saved review's automated findings and human follow-up separate. All processing and storage are local. No account, AI service, printer or command prompt is required.

## Compare revised documents

1. Read the earlier PDF/DOCX in **Review** and save its assessment if you want to compare findings.
2. Select **Version history / compare…** from the document header or a saved report.
3. Choose **Add revised document…** and select the revised local PDF/DOCX. Alternatively, expand **Link an existing document as a revision** and select a standalone entry.
4. Select earlier and revised readings under **Compare versions**. Select a saved review for each side, or leave a side as text only.
5. Choose **Compare selected versions**. Inspect additions/removals with page/paragraph locations, both automated outcomes and their separate human statuses.
6. Use **Read complete retained texts** to search both full readings. A finding's **Show passage in retained text** jumps to and highlights its evidence. **Open review and its notes** opens that saved assessment with its own offline legal provisions and follow-up history.
7. Export **comparison PDF** or **full comparison JSON** to a new local filename.

Adding a revision reads text but never starts a review or OCR automatically. If the revised file needs OCR, open its current file in Review and deliberately run local OCR, compare the recognized text with the original, then save a new assessment.

**Review revised file** opens the current original for a fresh assessment. **Use earlier scope for a fresh review…** copies the selected earlier scope as a starting point and clears both operative-text and OCR confirmations. Review its coverage, exceptions, dates and factual basis again. No earlier notes or follow-up statuses are copied.

### Version retention and link corrections

Reading a changed file at the same path also captures a new file version. The same bytes in the same document entry reuse their version; an OCR rerun or changed reader result can retain a different reading of those bytes. Different named entries can contain identical file bytes; the comparison labels that explicitly.

Retained text includes structured locations, reading state, warnings and OCR metadata. A file SHA-256 identifies the input bytes; a separate reading fingerprint covers the complete extraction report. New reviews are bound to the actual reading used, including when another app window refreshes the current cache.

Original PDF/DOCX bytes remain in their original files and are not copied into version history. Retained readings remain available when originals move or change, but cannot reproduce original page images, signatures or layout. Keep originals when those matter.

Use **Linked files and link corrections → Unlink file; keep its history** to correct an accidental link. Each entry keeps its readings, reviews and notes. Existing multi-file histories are not merged automatically; unlink an individual entry first. Stale link selections require a refresh. Capture order is not a signing date or determination of legal effect.

### What finding comparisons mean

Findings are matched by stable check IDs. A direct comparison requires matching tested rule/pack identity, supporting source/provision/library assessment, per-check scope and confirmations, and reading/OCR method. Both selected reviews must be associated with their exact retained readings. Additional extraction warnings or incomplete text prevent a direct correction classification.

| Label | Meaning |
| --- | --- |
| Still flagged | Both comparable reviews contain a potential issue for this check. |
| Newly flagged | The revised comparable review flags a potential issue. |
| Numeric issue no longer flagged | The earlier comparable potential issue becomes a stated term meeting that specific numeric threshold. This is not a compliance certificate or human resolution. |
| Result or evidence changed | Another outcome/passage changed; a missing clause or uncertain result does not establish a correction. |
| Review basis differs | Scope, dates/basis, source, rule, reading method or exact-reading evidence differs; inspect both assessments manually. |
| Check absent from earlier/revised review | A check was not included in one selected rule set. Absence is not resolution. |
| Unchanged | The comparable automated outcome and saved evidence are unchanged. |

Human **Resolved** on the earlier side stays with that earlier review. A fresh review starts with **Unresolved**, even if its numeric result improves. Comparing or exporting never changes either review's notes/statuses. A captured comparison includes one consistent snapshot of both histories; compare again to include later follow-up.

Text comparison covers extracted lines, with two context lines, a 500 ms edit-search budget and a 20,000-line input ceiling. The display retains up to 2,000 rows and 2,000 characters per row, paged in sections. Approximate edit alignment can be nonminimal. Truncation and unavailable totals are labelled; complete retained readings remain available in the app and JSON. Missing/unreadable text is not reported as a deletion. This does not compare images, layout, handwriting, signatures or legal meaning.

### Earlier workspaces

Private schema 5 upgrades transactionally to schema 6. Existing document references, preferences, cached extraction JSON, original review JSON, notes/status history and folder settings are preserved.

The latest valid old cache can be retained as a version reading. Earlier full text that was never saved cannot be recovered from a changed original. Old reviews did not record an exact reading fingerprint; those associations remain explicitly unknown rather than being invented from the latest OCR/cache. Their findings, source hashes and notes remain available, but a direct finding comparison labels the missing association.

All new normal reviews record their exact reading. A legacy review without one can still try to open its current original; passage navigation requires the matching file hash and saved evidence. A review with a retained reading opens that exact saved text without requiring the original file.

## Add notes or track an issue

1. Open a saved review in **Review**, **Reports** or **Version history**.
2. Expand **Document review notes**, or a finding's **Reviewer notes and status**.
3. Enter a **Reviewer name (self-reported)** and required note/reason. For a finding, choose **Unresolved**, **Acknowledged** or **Resolved**.
4. Select **Save reviewer update**. Earlier names, times, notes and transitions remain visible.

| Follow-up status | Meaning |
| --- | --- |
| Unresolved | Follow-up is open; the initial status for every finding in a fresh review. |
| Acknowledged | A reviewer has recorded awareness or assigned follow-up. |
| Resolved | A reviewer recorded completion with a reason, without changing the original automated result. |

Names are self-reported, not authenticated identities or digital signatures. A resolved potential issue remains a potential issue in its original assessment. Add a correction note or reopen follow-up instead of editing old entries. Notes belong to one saved review, not every review of the file.

The interface accepts up to 2,000 characters per note; storage accepts up to 8,000 UTF-8 bytes. Names are required and limited to 120 characters. For a concurrent-update conflict, select **Load latest status; keep my note**, inspect the current history and save deliberately.

## Export review and comparison reports

**Export PDF…** on a saved review includes its original outcomes, evidence/locations, legal quotations and source identifiers, confirmed scope and complete saved human history. New reviews also include retained version/reading identifiers and a reading fingerprint. It can be exported without reopening the original. The complete supporting provision remains available offline in the app.

**Export comparison PDF…** includes displayed differences, comparison limitations, both selected results/statuses, file/reading hashes and both selected saved review histories in full. It does not include every original document line when the difference display is truncated. **Export full comparison JSON…** also includes both complete retained readings, source/rule/scope details and captured histories.

Exports use embedded local Lato fonts, not a browser, print service or online request. They contain saved data only; save pending notes and refresh a comparison first. They do not reread originals, rerun rules or establish that cited law is current. Existing destinations are never overwritten, and failed exports leave no partial destination file.

PDF export supports up to 200 pages and 8 MiB of report text. Unsupported scripts produce an explicit error; JSON preserves their text. Original absolute file paths are omitted, but names, excerpts, notes and user-entered bases can identify private information. Exports are unencrypted local files; choose their storage and recipients deliberately. **Copy full report JSON** uses the clipboard only when explicitly selected.

## Storage and removal

Windows data remains in **C:\ProgramData\ComplyPH**, restricted to the owning account, SYSTEM and Administrators. Private schema 6 contains retained readings/revision links alongside reviews and notes; the public legal library remains separate at schema 5. Private comparison does not open or modify that public catalog.

Updates and uninstall preserve workspace data and originals. **Remove entry** removes that file's cached/retained readings, saved reviews and follow-up history; other linked files keep their own histories. Previously exported files and backups remain where saved. SQLite deletion/journals are not secure erasure, encryption or a tamper-proof audit against someone controlling the computer.
