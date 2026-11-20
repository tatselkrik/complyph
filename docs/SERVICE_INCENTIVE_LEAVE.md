# Service incentive leave: source assessment and check limits

Implementation assessment: **21 November 2026**. This documents the technical interpretation used by `PH-LAB-095-01`; it is not a qualified Philippine legal opinion or a claim that the saved compilation incorporates every later amendment or special regime.

## Preserved primary provision

The existing agency-downloaded Senate P.D. 442 compilation, `labor-code-reference`, contains Article 95 and the complete implementing Book III Rule V. The original and reading-text SHA-256 pins are listed in [validated checks](VALIDATED_CHECKS.md). Neither the source bytes nor the library bundle changes for this release. The new offline mapping opens Article 95, Article 82 and Rule V from the exact hashes recorded by each review.

Article 95(a) and Rule V section 2 prescribe a yearly service incentive leave of **five days with pay** after at least one qualifying year. Article 95(b) and Rule V section 1 describe exclusions and existing-benefit exceptions. Rule V section 3 defines qualifying service; sections 5–6 address cash commutation and existing agreements/practices. Only the explicit annual paid-day threshold is automated.

## Eligibility questions the reviewer establishes

- **Employee coverage:** determine the applicable regime and actual work/supervision. Article 82 and Rule V list exclusions, but a job title or pay method alone does not decide them. Government employment, managerial work, field/unsupervised work and personal-service or special regimes require appropriate analysis. Probationary status is not itself an SIL exception.
- **Qualifying service:** at least twelve months, continuous or broken, reckoned from starting work, including authorized absences and paid regular holidays. An established/agreed shorter working year can count. Do not require twelve uninterrupted months of actual working days or infer tenure from document dates alone.
- **Exceptions and alternative benefits:** check existing SIL, vacation leave with pay of at least five days, regular establishment headcount below ten, any applicable Secretary-granted exemption, and other relevant exclusions. Exactly ten workers is not below ten. A pooled or generic leave number does not establish an equivalent paid vacation benefit. Record the evidence and review period in the eligibility basis.

An outside-scope result decides no entitlement under another law or contract. Actual leave accrual, proration, unused-leave cash conversion, wages, benefit use/payment and more favourable terms remain separate questions. A numeric match does not establish employer compliance.

## Official corroboration and interpretation

- [Senate legislative record for P.D. 442](https://ldr.senate.gov.ph/legislative%2Bissuances/Presidential%20Decree%20No.%20442%2C%20s.%201974) — provenance of the preserved compilation used by the rules.
- [DOLE Book III publication](https://dole.gov.ph/book-3-conditions-of-employment/) — official Article 95 reference. The live page returned an access rejection during this assessment; no fresh official-byte verification or comprehensive currency check is claimed.
- [Supreme Court electronic library: implementing rules](https://elibrary.judiciary.gov.ph/thebookshelf/showdocs/2/85819) — official Rule V reference.
- [David v. Macasio, G.R. 195466, 2 July 2014](https://elibrary.judiciary.gov.ph/thebookshelf/showdocs/1/57198) — discusses SIL and the field-personnel/unsupervised-work exclusion. Task or pakyaw payment alone does not automatically exclude a supervised employee. This guidance informs the form's caution; the app does not classify the worker or apply case law automatically.

Proposals to increase leave are not treated as enacted amendments. Any new issuance or changed library download requires legal assessment and rule revalidation before changing this pinned threshold.

## Validation

Independent rule tests cover 0, 4, 4.99, 5, 5.01 and favourable grants; written numbers; each unknown/excluded fact; empty/invalid eligibility bases; conditional/quoted/conflicting/unpaid/nonannual/pooled statements; incomplete readings; OCR confirmation/provenance; and historical five-rule context/review compatibility. The PDF and Word executable fixtures cover matching, problematic, ambiguous, excluded and unknown-service cases, preserve original bytes and verify eligibility/citation content in exported PDF reports. Native viewer tests verify complete Article 95 and Rule V boundaries, including service definitions, exceptions, commutation and existing-benefit protections.
