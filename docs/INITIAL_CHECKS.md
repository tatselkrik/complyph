# Initial rule coverage across seven domains

ComplyPH 0.11.0 completes the **initial 20-check technical milestone**. This is a finite checklist, not complete coverage of Philippine compliance law. Six existing labour comparisons are retained; fourteen new comparisons add probationary duration and initial OSHS, privacy, tax and contribution coverage. Qualified Philippine legal review remains outstanding under roadmap item 6.

## Checklist

All twenty checks are implemented with applicability gates, source pins, matched evidence, exact offline provisions, persisted historical reports and independent expected cases.

| Check | Supported comparison | Supporting provision |
| --- | --- | --- |
| PH-LAB-083-01 | Normal daily working hours at most 8 | Labor Code Article 83 |
| PH-LAB-087-01 | Additional ordinary-day OT at least 25% | Article 87 |
| PH-LAB-086-01 | Night differential at least 10%, 22:00–06:00 | Article 86 / Rule II |
| PH-LAB-085-01 | Standard meal time-off at least 60 minutes | Article 85 / Rule I |
| PH-LAB-091-01 | Weekly rest at least 24 consecutive hours after six consecutive normal work days | Article 91 / Rule III |
| PH-LAB-095-01 | Annual paid SIL at least 5 days after separately established eligibility | Article 95 / Rule V |
| PH-LAB-296-01 | Standard probationary period at most 6 whole calendar months | Labor Code Article 296 [281 in preserved compilation] |
| PH-OSH-016-01 | Mandatory worker OSH seminar at least 8 hours | R.A. 11058 Section 16(b) |
| PH-OSH-008-01 | Worker charge for required PPE: PHP 0 | R.A. 11058 Section 8 |
| PH-NPC-038-01 | Required NPC and affected-data-subject notifications within 72 hours of knowledge or reasonable belief | DPA IRR Sections 38–40 |
| PH-BIR-024-01 | Annual taxable-compensation zero-rate ceiling: PHP 250,000 | R.A. 10963 Section 5 / NIRC Section 24(A)(2)(a) |
| PH-SSS-TOTAL-01 | Standard mandatory SSS total rate: 15% of MSC | R.A. 11199 Section 4(a)(9), 2025 schedule |
| PH-SSS-EMPLOYER-01 | SSS employer rate: 10% of MSC, excluding EC | R.A. 11199 Section 4(a)(9), 2025 schedule |
| PH-SSS-EMPLOYEE-01 | SSS employee rate: 5% of MSC | R.A. 11199 Section 4(a)(9), 2025 schedule |
| PH-PHIC-RATE-01 | PhilHealth premium rate: 5% of monthly basic salary | PhilHealth Advisory 2025-0002, premium schedule |
| PH-PHIC-FLOOR-01 | PhilHealth income floor: PHP 10,000 | PhilHealth Advisory 2025-0002, premium schedule |
| PH-PHIC-CEILING-01 | PhilHealth income ceiling: PHP 100,000 | PhilHealth Advisory 2025-0002, premium schedule |
| PH-HDMF-EE-HIGH-01 | Pag-IBIG employee rate above PHP 1,500 monthly compensation: 2% of applicable fund salary | R.A. 9679 Section 7 (rate only) |
| PH-HDMF-EE-LOW-01 | Pag-IBIG employee rate at most PHP 1,500: 1% of applicable fund salary | R.A. 9679 Section 7 (rate only) |
| PH-HDMF-ER-01 | Pag-IBIG employer rate: 2% of applicable fund salary | R.A. 9679 Section 7 (rate only) |

## Applicability and period

Each new comparison requires a reviewer to establish coverage, review applicable amendments and exceptions, enter an actual date within the period being assessed, and record a factual basis. A date is not inferred from document metadata. The new profiles support their recorded start dates through **31 December 2026**; later periods need separately assessed profiles. These are conservative initial technical coverage bounds, not statutory commencement or expiry dates. New labour/privacy profiles start in 2025; OSH profiles start with the assessed DO 252-25 period on 16 May 2025. Earlier periods need separate profiles. The six earlier rules retain their existing scope gates; SIL retains its separate three-fact eligibility form.

Ordinary probation is not inferred from a title or converted from days. Apprenticeship, learnership, academic/special regimes and exceptional extensions need separate assessment. The cited article also includes regularization standards and continuing-work provisions; the app does not decide actual regularization or termination.

The OSH seminar check is the worker seminar, not the safety-officer BOSH course. It does not prove attendance, content or training-provider compliance. PPE must be required for confirmed hazards; a zero stated charge does not prove suitable equipment was actually supplied. DOLE staffing/facility matrices and other DO 252-25 requirements remain future coverage.

Breach notification requires a reportable breach, PIC responsibility, the correct knowledge/reasonable-belief trigger and assessment of lawful delays/exceptions. The app neither classifies actual incidents nor notifies anyone. The supporting section opens the notification criteria and delay provisions, not only the 72-hour excerpt.

The BIR check concerns the annual **taxable** compensation band under the 2023 graduated schedule. It does not classify taxpayers, calculate tax/withholding, handle minimum-wage exemptions or apply the 8% business-income election. Proposed legislative thresholds are not treated as enacted changes.

SSS uses the confirmed monthly salary credit, with the SS/MPF schedule and employer-only EC separated. PhilHealth uses the defined monthly basic salary and applicable category, floor and ceiling. Pag-IBIG uses an independently established applicable fund salary; its original Act's PHP 5,000 cap has been superseded and is **not used** by these rate-only checks. Confirm the current cap under Circular 460 and agency guidance. Only one Pag-IBIG employee compensation band can be marked covered for the same worker/period. The exactly-PHP-1,500 case belongs to the lower band. Voluntary top-ups, MP2, special schedules, contribution amounts, bracket selection, payment and remittance are outside the initial comparisons.

## Sources and assessment

Assessment date: **21 November 2026**. All check sources are existing agency-downloaded, hash-pinned snapshots. The nineteen-reference starter bundle, its provenance labels, private schema 5, the then-current public schema 4 and `C:\ProgramData\ComplyPH` remain unchanged. No provisional source was promoted. The exact pins and section boundaries are committed in `crates/checks/src/initial_rules.json`; each new saved finding carries its own source key, raw/text hashes, official URL and provision locator. The review records the SHA-256 of the fourteen compiled additional-profile definitions, alongside its engine version. The original six comparisons remain identified by the engine and release source. Earlier reports keep their original findings and raw stored JSON.

Current official corroboration consulted during assessment:

- [SSS contribution schedules](https://www.sss.gov.ph/pay-contribution): confirms the standard 15% / 10% / 5% schedule from January 2025, separate EC and membership-specific schedules. The RA 11199 table was also visually compared with its preserved PDF page 7.
- [NPC implementing rules](https://privacy.gov.ph/implementing-rules-regulations-data-privacy-act-2012/) and [breach-reporting guidance](https://privacy.gov.ph/pips-and-pics/breach-reporting/): notification criteria, the 72-hour trigger and exceptions. Recent [NPC public statement](https://privacy.gov.ph/statement-of-the-national-privacy-commission-on-reported-data-leaks-involving-government-agencies/) distinguishes allegations from established reportable breaches.
- [BIR official form, RMC 34-2025 Annex A](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2034-2025%20Annex%20A.pdf): 2023-onward graduated table with the PHP 250,000 zero-rate boundary.
- [PhilHealth Advisory 2026-0042](https://www.philhealth.gov.ph/advisories/2026/PA2026-0042.pdf), signed 21 July 2026: reiterates 5% from January 2025. The preserved 2025-0002 advisory provides the basic-salary definition, floor and ceiling. [PIA's PhilHealth statement](https://pia.gov.ph/news/no-hike-in-premium-rates-for-2026-says-philhealth/) corroborates continuation in 2026. External corroboration is not a new bundled snapshot or a representation of exhaustive amendment review.
- [Pag-IBIG regular-savings guidance](https://pagibigfund.gov.ph/Membership_RegularSavings.html) and [circular index](https://www.pagibigfund.gov.ph/about_publication_circulars_memberservices.html): the February 2024 maximum-fund-salary change. The rate-only rules cite RA 9679; the provisional Circular 460 copy is not used as an approved calculation table.

Publication origin and hash preservation establish provenance/integrity. They do not replace qualified legal interpretation, establish every amendment or verify the reviewer-supplied employment/payroll facts.

## Wording, outcomes and validation

Supported statement formats appear in `examples/expected-initial.json`, for example `SSS employer contribution rate is 10% of monthly salary credit.` and `Worker charge for required PPE is PHP 0.` These are fictional test statements, not recommended employment templates. Strict complete-clause matching retains semicolons and wrapped lines. Examples, quotations, conditions, conflicts, unsupported paraphrases or missing evidence stay unresolved. A following explicit condition in the next paragraph is retained as evidence. Decimal boundaries use integer hundredths; whole probationary months are not rounded or converted from days. Monetary comma groups are checked before parsing.

Independent tests transcribe thresholds separately and cover both sides of boundaries; exact-rate mismatches; exclusions; every missing fact/date/basis gate; valid and invalid dates; old six-finding reports; mutually exclusive Pag-IBIG bands; OCR confirmation/provenance; pin/quote integrity; and complete offline provision boundaries. Five real PDF/Word cases exercise a baseline, problems, ambiguity and the lower-income Pag-IBIG band. Executable tests preserve originals and saved findings, add human follow-up to a tax finding, and verify all domains' source identifiers/hashes, periods and basis in PDF export. Windows installation checks repeat the cases, scope gates and exported PDF reading from the packaged executable.

## Completion boundary

Roadmap item 1 is complete for this **initial twenty-check set**. Broader labour rules, OSH matrices, privacy-notice/security requirements, full tax/payroll calculations, remittance deadlines and other special regimes are future expansion. Source adoption/revalidation remains item 5; qualified legal review and broader reliability evaluation remain item 6. AI review remains unimplemented.

In 0.12.0, [library maintenance](LIBRARY_MAINTENANCE.md) upgrades the public catalog to schema 5 without changing the initial numeric definitions. Current-source changes and metadata/identity/period assessments can hold an affected finding. Complete unchanged supporting text plus a reviewed scope/evaluation assessment can bind future findings to an adopted source; changed support requires a new tested profile. Earlier reports retain their original source versions and conclusions.
