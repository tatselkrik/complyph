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

The development-only [generator](../scripts/generate-synthetic-documents.py) uses Python, ReportLab, Pillow, and pypdf. The installed application uses Rust readers; it needs none of those tools. The encrypted fixture may differ byte for byte when regenerated because encryption uses randomness.

A successful reading result means that text was extracted. The separate focused-check fixtures below test the original five numeric terms; reading alone produces no legal conclusion.

## Original labour check fixtures

`Synthetic labour baseline.pdf` states 8 net daily working hours, 25% additional ordinary-day overtime, 10% additional night differential for 22:00–06:00, 60 regular-meal minutes and 24 consecutive weekly-rest hours after six consecutive normal work days. `Synthetic labour issues.docx` states 9 hours / 20% / 5% / 30 meal minutes / 12 consecutive rest hours. `Synthetic labour ambiguous.docx` contains a lunch-inclusive shift, unqualified all-in overtime rate, incomplete night window, shortened paid meal and split rest. With the separate reviewer-confirmed standard scope, expected outcomes are five meets / five potential issues / five unresolved. Shortened paid meals are not automatically unlawful; the fixture stays unresolved pending human review. The added SIL finding and fourteen newer checks remain unconfirmed because these earlier contexts have no SIL eligibility facts. See `expected-checks.json`. These are fictional test statements, not legal templates or certified compliant policies.

## Legal-source intake fixture (0.5.0)

`synthetic-source.json` contains fictional citation metadata for use with `Synthetic employment.pdf`. Its `example.invalid` URL is reserved test data and must not be fetched. This intentionally fictional source starts awaiting verification and tests local intake, duplicate handling and source history. Importing it does not create a legal checking rule. No sample is an authoritative legal publication.

## Service incentive leave fixtures (0.10.0)

`expected-sil.json` supplies separate fictional eligibility contexts and expected SIL outcomes for five cases: five annual paid days (meets this threshold), four paid days (potential issue), unsupported/conditional leave (needs human review), a confirmed small-establishment exception with nine regular employees (outside scope), and missing qualifying service (needs context). Other rules remain unconfirmed. These are test cases, not eligibility decisions or employment templates. The installed Samples folder includes all five.

Regenerate only the SIL cases with `python scripts/generate-sil-fixtures.py` (development only, requires ReportLab). The application does not need Python.

## Initial seven-domain fixtures (0.11.0)

`expected-initial.json` defines independent boundaries and five real document cases: a Word/PDF baseline, problematic stated values, ambiguous/conditional wording, and a separate lower-income Pag-IBIG worker. The main baseline establishes the higher employee band and excludes the lower band; the lower-income case reverses that choice. Fictional bases and dates are explicit. Documents are never legal templates or proof of actual worker eligibility.

The development generator is `scripts/generate-initial-fixtures.py` (ReportLab plus Python standard library). The installed app needs neither Python nor ReportLab. All cases ship in Samples; selecting a document alone never establishes scope or starts a review automatically.

## Fictional publication listings

Synthetic publication listing.html and Synthetic updated publication listing.html
are inert discovery fixtures with no real issuances. They exercise official-link
resolution, duplicate/unsafe/hidden/script exclusion, initial inventory and one
later newly seen citation. Parsing them never fetches their linked documents.

## Document revision fixtures

Five clearly fictional revision files (original/corrected Word and PDF pairs, plus
a Word revision with a missing overtime clause) exercise retained text, explicit
links, changed passages, independent finding expectations, separate human statuses
and local exports. `expected-revisions.json` defines the expected outcomes.
Generate them with the standard-library-only `scripts/generate-revision-fixtures.py`.
These files are test inputs, not legal templates or real employment records.
