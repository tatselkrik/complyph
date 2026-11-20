# ComplyPH starter reference library

ComplyPH 0.7.0 includes **19 preserved references across seven domains**: 14 verified agency downloads, 3 identity-checked uploads and 2 provisional third-party copies. The app installs them locally without network requests. The collection is a starter library with explicit currency, amendment and interpretation gaps.

## Provenance and integrity

- **Verified agency download:** official publication channel and identity checked; preserved bytes and reading text pinned by SHA-256. This is not verification of complete/current law.
- **Identity-checked imported copy:** imported document identity/content structure checked and a claimed agency URL recorded; exact agency-byte comparison remains unavailable. Not promoted to a verified agency download.
- **Provisional third-party copy:** mirror or stitched social-media images. Government references corroborate identity/general content, not every supplied byte or OCR character.

All provenance classes receive preserved-byte/text integrity checks on import, reopen, reading and export. Reading text is displayed offline; active HTML is never rendered. **Save source copy** exports the exact preserved file, which may be a reduced/mirrored/stitched copy, not an agency original. Compare OCR text with page images for figures and matrices.

## Included snapshots

| Domain | Instrument and publication | Source |
| --- | --- | --- |
| Labor, DOLE OSHS | P.D. 442 — Labor Code — Senate reference compilation | [Official publication](https://ldr.senate.gov.ph/legislative%2Bissuances/Presidential%20Decree%20No.%20442%2C%20s.%201974) |
| Labor | R.A. 11210 — Expanded Maternity Leave Law | [Official publication](https://ldr.senate.gov.ph/legislative%2Bissuances/Republic%20Act%20No.%2011210) |
| DOLE OSHS | R.A. 11058 — Occupational Safety and Health Law | [Official publication](https://ldr.senate.gov.ph/legislative%2Bissuances/Republic%20Act%20No.%2011058) |
| Privacy / NPC | R.A. 10173 — Data Privacy Act of 2012 | [Official publication](https://privacy.gov.ph/data-privacy-act/) |
| Privacy / NPC | IRR of R.A. 10173 — Data Privacy Act implementing rules and regulations | [Official publication](https://privacy.gov.ph/wp-content/uploads/2023/06/IRR_RA-10173-as-amended.pdf) |
| Privacy / NPC | NPC Circular 2023-06 — Security of Personal Data in Government and the Private Sector | [Official publication](https://privacy.gov.ph/wp-content/uploads/2024/03/NPC-Circular-Repeal-16-01-Signed.pdf) |
| BIR / Tax | R.A. 10963 — Tax Reform for Acceleration and Inclusion (TRAIN) | [Official publication](https://ldr.senate.gov.ph/legislative%2Bissuances/Republic%20Act%20No.%2010963) |
| BIR / Tax | BIR RR 29-2025 — Amended de minimis benefit ceilings | [Official publication](https://bir-cdn.bir.gov.ph/BIR/pdf/RR%20No.%2029-2025.pdf) |
| SSS | R.A. 11199 — Social Security Act of 2018 | [Official publication](https://issuances-library.senate.gov.ph/sites/default/files/2022-03/ra%2011199.pdf) |
| SSS | SSS Circular 2024-006 — SSS contributions for business employers and employees | [Official publication](https://www.sss.gov.ph/wp-content/uploads/2024/12/CI-2024-006-Publication.pdf) |
| PhilHealth | R.A. 11223 — Universal Health Care Act | [Official publication](https://www.philhealth.gov.ph/about_us/RA11223_UHC.pdf) |
| PhilHealth | PhilHealth Advisory 2025-0002 — Premium contribution for direct contributors — CY 2025 | [Official publication](https://www.philhealth.gov.ph/advisories/2025/PA2025-0002.pdf) |
| PhilHealth | PhilHealth Advisory 2026-0050 — Electronic Premium Remittance System version 4.9.0 | [Official publication](https://www.philhealth.gov.ph/advisories/2026/PA2026-0050_v1.pdf) |
| Pag-IBIG | R.A. 9679 — Home Development Mutual Fund Law of 2009 | [Official publication](https://ldr.senate.gov.ph/sites/default/files/2023-02/ra%25209679.pdf) |

## Added imported and third-party copies

| Key | Reference | Provenance |
| --- | --- | --- |
| labor-code-2022-upload | DOLE 2022 renumbered Labor Code, 136 PDF pages | Identity-checked imported copy |
| dole-252-25-upload | D.O. 252-25, reduced copy, 110 PDF pages, OCR reading text | Identity-checked imported copy |
| oshs-2017-upload | OSHS compilation through 2017, 431 PDF pages | Identity-checked imported copy |
| pagibig-460-upload | Circular 460, 5-page third-party mirror | Provisional third-party copy |
| pagibig-499-upload | Circular 499, 4 stitched Facebook image pages, OCR reading text | Provisional third-party copy |

See [addition assessment](ADDITIONS.md) for source links, completeness observations, effectivity and limitations. The PwC tax compilation is a dated **external citation only**, not a bundled PDF or a verified source. It ends at 31 January 2020 and does not establish current NIRC coverage.

## Known gaps

The imported Labor Code and OSHS files add reference material; later amendments, special-sector applicability and amendment reconciliation remain outstanding. Agency originals for the provisional Pag-IBIG circulars still need exact comparison. No OSHS staffing/facilities matrix or contribution/remittance calculator has been validated. BIR/NIRC coverage remains incomplete. The 2025 PhilHealth premium advisory remains a dated reference.

Five supported labour rules use the pinned **agency-downloaded Senate P.D. 442** reference, not the provisional or OCR-heavy additions. See [rule scope and validation](../docs/VALIDATED_CHECKS.md). Acquiring a reference does not create a validated checking rule.

## Reproducibility

`starter/manifest.json` records stable keys, original/resolved URLs, provenance, optional official corroboration links, acquisition/assessment dates, effectivity notes, preserved raw and derived-text hashes, extraction method and coverage gaps. Raw files are in `starter/raw`, reading text in `starter/text`. `support/oshs-original-page-map.csv` records the preparation split's original page mapping. No private document content is included.

Public schema 4 preserves previous entries and snapshots and distinguishes manual pending/user-checked provenance. Bundle import is idempotent and transactional; Bundle updates preserve earlier source versions. Manual PDF/DOCX import is available from Legal library → Add legal source. It records citations locally and preserves earlier versions; see [intake instructions](../docs/LEGAL_SOURCES.md). Approve official URLs inside the source reader to enable daily checks while the Windows app is open. Changed response bytes are stored separately for review and never replace curated reference/rule pins automatically. Status queries do not make requests; see [source updates](../docs/SOURCE_UPDATES.md).
