# Reports and review tracking

ComplyPH 0.8.0 preserves an automated review and records human follow-up separately. All processing and storage are local. No account, AI service or printer installation is required.

## Add notes or track an issue

1. Open a saved review in **Review** or **Reports**.
2. Expand **Document review notes** for a note about that saved review, or **Reviewer notes and status** under a finding.
3. Enter a **Reviewer name (self-reported)** and a required note/reason. For a finding, choose **Unresolved**, **Acknowledged** or **Resolved**.
4. Select **Save reviewer update**. The history shows the recorded name, time, note and any status transition.

| Follow-up status | Meaning |
| --- | --- |
| Unresolved | Follow-up remains open; the initial status for every finding. |
| Acknowledged | A reviewer has recorded awareness or an assigned follow-up. |
| Resolved | A reviewer has recorded completion of follow-up, with a reason. |

These labels do not change the automated result, supported passage, scope or citation. A resolved potential issue remains a potential issue in the original assessment. The app does not verify that a correction was made or that the document complies with law.

Notes belong to one saved review snapshot. A new review starts with fresh follow-up statuses and its own history. The history supports notes for any finding outcome, including missing evidence and unclear applicability. Reviewer names are entered by the user, not authenticated identities or digital signatures. The application remembers the current reviewer name only for the open session.

Saved entries are append-only through the app. Add a new note to correct an earlier entry, or choose Unresolved to reopen follow-up. Every status change requires a note. The interface accepts up to 2,000 characters per note; the storage API limits notes to 8,000 UTF-8 bytes. Reviewer names are required and limited to 120 characters. If another app window updated the same finding or document-note stream, select **Load latest status; keep my note**, inspect the latest history/status and then save deliberately.

## Export a PDF

Select **Export PDF…** on the saved review, choose a folder and a new filename, then save. The export runs locally with embedded Lato fonts, without a browser, print driver or online request. Save any pending notes first: exports include saved actions only.

The PDF includes:

- Document name, review ID, original review time and export snapshot time/history cutoff.
- Original outcomes, explanations, confirmed scope, evidence passages and their page/paragraph locations.
- Supporting article names and the exact legal quotations saved by the rule engine.
- All saved document-review notes and finding notes/status transitions, with names and timestamps.
- Scope limits, rule/source identifiers, source URL, document/source hashes and original-review fingerprint.

The PDF includes the saved legal quotation. The complete supporting article remains available through **Open supporting provision offline** in the app. No external source is fetched during export. Original absolute file paths are omitted; document names and excerpts can still identify private information. The export is an unencrypted local file, so choose its storage and recipients deliberately.

The report captures one consistent saved history, including every entry even when older entries are collapsed in the interface. It can be exported if the original document has moved, because the saved findings contain their own evidence. It does not reread the original, rerun checks or establish that cited law is current. Correcting a document requires a new review to evaluate its new text.

Existing files are never overwritten. PDF export supports up to 200 pages and 8 MiB of report text. Bundled fonts cover English/Filipino Latin text and the peso sign; unsupported scripts produce an explicit error without a partial destination file. **Copy full report JSON** retains all text and history as the alternative. Copying places the private report on the clipboard only when selected.

## Storage and upgrades

The private database adds schema 4 for reviewer actions. Existing documents, cached text, preferences and saved review JSON are retained. Historical three-rule reports keep their original findings; newer rules are not added retrospectively. Source catalogs and rule/source pins are unchanged by this update.

Windows data remains in `C:\ProgramData\ComplyPH`, restricted to the owning account, SYSTEM and Administrators. Updates and uninstall preserve it. Removing a document entry removes its cached text, saved reviews and tracking history while preserving its original file. Previously exported PDFs/JSON and backups remain where they were saved. The local database/history is not encrypted or a tamper-proof record against someone who controls the computer.
