# ComplyPH starter reference library

Version 0.2.0 bundles **14 official-publication snapshots across seven domains**, checked on **2026-11-16 UTC**. The application installs them into its public SQLite catalog without making network requests.

## What verified means

Verification covers the official publication channel, document identity, preserved source bytes, and SHA-256 integrity. It does not certify a complete amendment chain, present applicability, OCR accuracy, legal interpretation, or employer compliance. No checking rules are bundled.

Original PDF/HTML responses are stored under `starter/raw/`; derived reading text is under `starter/text/`. The app renders reading text, never active HTML. Use **Save original copy** in the library reader for the exact publication file. OCR, table layout, and publisher text layers can contain errors; consult the original for exact wording and figures.

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

## Known gaps

The selection is a starting reference collection. Each source carries its own applicability notes and conditional effectivity information; an unknown effective date stays unknown.

- **DOLE Labor Code 2022 edition:** Official download returned HTTP 403 in this acquisition environment. Not imported or counted as verified. [Publication page](https://bwc.dole.gov.ph/wp-content/uploads/2025/08/Labor-Code-of-the-Philippines.pdf).
- **DOLE D.O. 252-25 and full OSHS technical standards:** DOLE/BWC/OSHC and the DPWH-hosted attachment were inaccessible here. The existence of D.O. 252-25 was corroborated in official indexed listings; its document bytes are not bundled or counted as verified. [Publication page](https://dole.gov.ph/news/department-order-252-25-revised-implementing-rules-and-regulations-of-republic-act-no.-11058/).
- **Pag-IBIG Circular 460 and later remittance issuances:** Agency site presented a browser verification challenge. No attempt was made to bypass it. Circulars 460, 499, and other later requirements still need acquisition and review. [Publication page](https://www.pagibigfund.gov.ph/about_publication_circulars_memberservices.html).
- **Complete NIRC amendment and implementing-issuance coverage:** This starter contains TRAIN and RR 29-2025, not the full consolidated NIRC or its amendment chain; the BIR landing page alone is not imported as legal text. [Publication page](https://www.bir.gov.ph/Tax-Code).

The Labor Code entry is a Senate reference compilation with an expressly stated amendment set, not the inaccessible DOLE 2022 edition. The PhilHealth 2025 premium advisory is explicitly a 2025 reference. Dates or rates in these files must not be applied to another period without assessment.

## Provenance and integrity

[manifest.json](starter/manifest.json) records the requested and resolved official URLs, acquisition and verification timestamps, document identifiers, domains, signed/issued dates where established, conditional effectivity notes, extraction method, raw and reading-text hashes, and available HTTP provenance headers. The bundled snapshot payloads are checked before import, after database reopen, and before export.

The database stores source originals and reading text transactionally. Reopening does not duplicate a bundle. Schema 1 catalogs migrate to schema 2 while retaining existing entries and versions. Damaged snapshots cause an integrity error instead of being displayed as verified.

## Updates and additions

This release does not run daily source checks or provide manual source import yet. Bundled verification is separate from a runtime successful-update timestamp; the latter remains unset. The future updater must keep the public-source/private-workspace boundary and enforce its operating-system restrictions.

Government-issued legal texts and archived official publications are included for reference with attribution. Agency names and marks identify their publishers and do not imply endorsement of ComplyPH.
