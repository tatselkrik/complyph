# Supported document reading

ComplyPH 0.14.0 adds a bounded reading-quality profile for difficult PDF/DOCX files. Processing stays on the computer, and originals are never rewritten. This completes the initial OCR, folder-intake and reading-quality milestone; it does not establish accuracy for every document, font, layout or language.

## Supported layouts

| Input | Behavior |
| --- | --- |
| Horizontal English/LTR PDF text | Order text by character positions rather than PDF painting order; join close, aligned soft wraps within supported paragraph blocks. Keep original physical page numbers. |
| Two separated PDF columns | Read each supported column top to bottom, left before right, between full-width text. Inspect the original to confirm paragraph boundaries. |
| PDF tables or irregular columns | Show positioned rows with tab-separated gaps. Detected table/ambiguous-column relationships require manual review; they are not inferred from proximity. |
| Embedded fonts with usable Unicode mappings | Preserve decoded Unicode, including the tested Latin names and peso sign. Missing/private-use/replacement characters produce a reading warning; try local OCR or manual review. |
| Mixed text/image PDF pages | Retain selectable text and flag substantial unread image content, including images on otherwise readable pages. Local OCR reads the rendered pages. Image detection is a coverage heuristic; small images may still contain important text. |
| Word body, ordinary tables, headers, footers and notes | Retain XML part/paragraph/cell locations. Word page numbers are not inferred. |
| Word text boxes | Read supported `w:txbxContent` paragraphs and tables at separate text-box locations. Preserve the containing paragraph as one evidence location. Floating page placement is not inferred. |
| Alternative Word representations | Deduplicate matching text alternatives. Different text or unread embedded content produces a manual-review warning. XML location numbers include omitted branches, so visible numbers can have gaps. |
| Merged/nested tables, fields, revisions and embedded content | Retain supported text, label the uncertainty and block numeric conclusions. Field results may be stale. Embedded objects, external relationships and instructions are never opened, fetched or executed. |
| PDF forms and non-link annotations | Flag unestablished values/appearances, including during OCR. Use a flattened local PDF copy or manual review. |

The supported profile is tested with twelve fictional files: eight ordinary-reader cases and six OCR cases, including two files exercised through both paths. The files and independent expected results are in **Samples** / [examples](../examples/README.md). They are extraction fixtures, not legal templates or real employment records.

## OCR controls

Open a PDF in **Review**, expand **OCR options**, then choose **Run local OCR**:

- **Rotate page images clockwise:** no extra rotation, 90°, 180° or 270°. The PDF's own page rotation is respected first. The selected correction applies to every page; mixed-orientation scans may need a corrected local copy.
- **Reading layout:** automatic page layout, one text block, or scattered text. Scattered-text mode does not establish reading order and requires manual review.
- **Enhance faint scans and straighten slight skew:** grayscale pixel contrast and bounded mild-skew correction. Straightening is applied only when the alignment test finds a clear improvement within its tested range. It does not replace words, repair numbers or edit the PDF.

Inspect every original page and number, including words that have high recognition confidence. Amber highlights show low confidence, not every error. Compare the extracted text with the original and separately confirm OCR accuracy before checking. Table-like lines, poor recognition, empty pages, unsupported skew and other unresolved reading warnings keep numeric findings at **Needs review**. Use a corrected local copy or manual review rather than confirming incomplete text.

The selected settings, pixel-processing profile and per-page dimensions/contrast/straightening results are saved with the reading. **Read again** retains a cached OCR method's settings; **Run local OCR** applies the selected options as a deliberate rerun. An ordinary text-reader failure can still offer OCR when the PDF container is readable; encryption and resource limits remain enforced.

The unchanged limits are 20 OCR pages, 32 MiB input, 4 MiB recognized text, a 2,800-pixel maximum processed side, 90 seconds of worker CPU and 120 seconds elapsed. Ordinary PDF reading supports up to 300 pages and 100,000 decoded characters per page, within the total text limit. XML/ZIP/nesting and bounded display limits also apply. These are resource limits, not a promise that every file within them can be read.

Handwriting, signatures, stamps, charts, other scripts, complex/mixed layouts, missing glyph mappings, redactions and document appearance can require manual review. Empty pages cannot automatically be distinguished from recognition failures. Ordinary Word OCR is not included: export an image-only Word document to PDF locally, then use OCR. Public-source intake/maintenance retains its own explicit review and verification controls.

## Reviews, history and exports

Additional reading warnings block the twenty numeric checks from making threshold conclusions, even when text is available. Confirmed applicability and operative terms are still required; recognizing text never establishes legal coverage or compliance.

New readings use `complyph-text-2` / `complyph-ocr-2`. New review exports include the warnings and reading details from the exact retained reading, alongside OCR settings when applicable. They do not substitute the latest cache's quality for a historical assessment. Comparison PDF/full JSON exports retain the same distinction.

A rerun with different settings is a reading-method change. Version comparison labels it, keeps the original review/notes/statuses and does not call identical file bytes a document edit. Both accuracy confirmations must be established again for a fresh review.

Private schema 6 and public schema 5 remain unchanged. Earlier cached/report/note bytes and retained reading fingerprints are preserved. Older OCR settings that were never recorded remain unknown; unchanged earlier OCR caches can still reopen, and an explicit rerun uses current processing. Updating the ordinary reader can retain a new reading of unchanged bytes while preserving the earlier one.

Windows storage remains **C:\ProgramData\ComplyPH** with its existing account protections. Folder intake remains local and deliberate: it registers stable files, and never starts OCR, reviews or an AI upload. See [OCR and folder controls](OCR_AND_FOLDERS.md), [reports and version history](REPORTS.md) and [verification](VERIFICATION.md).
