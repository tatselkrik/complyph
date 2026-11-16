# Synthetic reader samples

All people, employers, amounts, policies, and arrangements in these files are **fictional test data**. They are not legal templates, guidance, or examples of compliant documents. No real personal identifiers are included.

Open ComplyPH and use **Try a synthetic sample: PDF / Word / Scan**, or choose any of these files through **Choose document**. The Windows installer and app ZIP include this folder as **Samples**.

| File | Expected reading result |
| --- | --- |
| Synthetic employment.pdf | Three text pages with preserved page numbers |
| Synthetic workplace policy.docx | Unicode, split text runs, tab/line breaks, a table, header, footer, visible link label, and inserted revision |
| Synthetic scan.pdf | Needs OCR; pixels are not read as text |
| Synthetic mixed pages.pdf | Page 1 text; page 2 has no selectable text and is flagged |
| Synthetic protected.pdf | Password protected; no password requested |
| Synthetic damaged.pdf | Friendly unreadable state |
| Synthetic damaged.docx | Friendly unreadable state |

`expected-results.json` defines statuses, required text, and content that must not be extracted. The Word external relationship targets `example.invalid` and must never be fetched. Deleted text and field instructions must not appear as document text. Word references identify XML parts, paragraph numbers, and table cells; they do not invent page numbers.

The two older `Synthetic sample` files remain for foundation compatibility. New tests use the seven cases above.

The development-only [generator](../scripts/generate-synthetic-documents.py) uses Python, ReportLab, Pillow, and pypdf. The installed application uses Rust readers; it needs none of those tools. The encrypted fixture may differ byte for byte when regenerated because encryption uses randomness.

A successful reading result means that text was extracted. The separate focused-check fixtures below test three numeric terms; reading alone produces no legal conclusion.

## Focused labour check fixtures (0.4.0)

`Synthetic labour baseline.pdf` states 8 net daily working hours, 25% additional ordinary-day overtime and 10% additional night differential for 22:00–06:00. `Synthetic labour issues.docx` states 9 hours / 20% / 5%. `Synthetic labour ambiguous.docx` contains a lunch-inclusive shift, unqualified all-in overtime rate and incomplete night window. With the separate reviewer-confirmed scope, expected outcomes are three meets / three potential issues / three unresolved. See `expected-checks.json`. These are fictional test statements, not legal templates or certified compliant policies.
