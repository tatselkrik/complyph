# Focused labour checks, version 3

“Validated” means source-anchored, technically tested numeric comparisons with explicit scope gates. No qualified Philippine lawyer has certified these rules. They are not a legal opinion or a complete compliance assessment.

## Source and thresholds

All six rules pin the preserved Senate P.D. 442 reference compilation, `labor-code-reference`. Original SHA-256: `99fd38f1434225504d0685fc5a92c4bb305a9e95bc63875d0781423d4acfa4c0`; reading text SHA-256: `c9e5a66dafa4aa6fc55c01347059c7abe15af841ac30b0ec0d74ba60823bd31f`. Both must match before desktop/diagnostic checking. The validation suite checks every quoted provision against that source. The uploaded DOLE 2022 edition corroborates the articles but does not replace the agency download pin. The reference bundle and its provenance have not changed in 0.10.0.

| Rule | Provision and comparison | Required human-confirmed scope |
| --- | --- | --- |
| PH-LAB-083-01 | Article 83, read with 82/84/85: normal hours at most 8 per day | Covered adult employment; standard arrangement; net working hours; no compressed/flexible, health-personnel or other special arrangement |
| PH-LAB-087-01 | Article 87: ordinary-day overtime additional premium at least 25% | Covered employment, beyond 8 hours on ordinary working days; additional premium on regular hourly wage, not an all-in rate |
| PH-LAB-086-01 | Article 86 / Book III Rule II: additional night differential at least 10% | Covered employment and exceptions checked; each worked hour 22:00–06:00; regular hourly wage basis |
| PH-LAB-085-01 | Article 85 / Book III Rule I Section 7: regular-meal time-off at least 60 minutes | Covered employment; the ordinary 60-minute requirement applies; shorter compensable meals and other special arrangements are outside this comparison |
| PH-LAB-091-01 | Article 91 / Book III Rule III: at least 24 consecutive hours of weekly rest after every six consecutive normal work days | Covered employment; standard uninterrupted weekly-rest arrangement; Article 92 and implementing-rule rest-day work exceptions reviewed separately |
| PH-LAB-095-01 | Article 95 / Book III Rule V: explicit annual service incentive leave of at least 5 days with pay | Three separately established facts: standard employee coverage, at least one qualifying year, applicable exceptions/alternative benefits reviewed; a recorded review-period/eligibility basis is required |

Coverage is not inferred from a job title. The UI lists Article 82 exclusions and the night-differential retail/service exception for establishments with not more than five workers. The reviewer must establish applicable amendments, employment coverage and review period. Government, managerial/staff, field/unsupervised, dependent family, domestic/personal service and qualifying paid-by-results work need appropriate exclusion analysis. More favourable contract/CBA terms, actual provision of meals/rest, religious rest-day preferences, holiday/rest-day pay, payroll execution and other domains are outside these comparisons.

The meal rule does not declare every meal shorter than an hour unlawful. Book III Rule I Section 7 permits shorter meals of at least 20 minutes in specified circumstances when credited as compensable work. Paid, interrupted/on-call, emergency or other exceptional meal arrangements require human review; their legality is not inferred from a duration alone. The offline viewer includes the full implementing rule and its conditions.

The weekly-rest rule requires an explicit consecutive duration and the six-consecutive-normal-work-day cadence. It does not total split breaks, assume Sunday must be the rest day, assess actual rosters or decide whether compulsory/voluntary rest-day work is lawful. Its offline viewer includes Article 92 and Book III Rule III for that separate review.

## Supported statements

Examples of recognized wording (illustrations, **not employment templates**):

```text
Normal daily working hours are 8 hours.
Ordinary-day overtime premium is 25% of the regular hourly wage.
Night shift differential is 10% of the regular hourly wage for hours worked between 22:00 and 06:00.
Regular meal period is 60 minutes of time-off.
Weekly rest period is 24 consecutive hours after every six consecutive normal work days.
Paid service incentive leave is 5 days per year.
```

Other supported patterns are listed directly in `crates/checks/src/lib.rs`. Unsupported clauses stay unresolved. The parser deliberately does not infer 25% from an unqualified “125% overtime rate”, subtract lunch from a shift span, convert written-out hours to minutes, add separate breaks, convert weekly hours, or resolve conflicting/conditional provisions. A five- or seven-day rest cadence is unsupported, rather than silently treated as the six-day comparison. It is case-insensitive and compares decimals as integer hundredths, without rounding.

A human must compare extraction with the original and confirm that statements are operative terms, not quoted examples. No rule can establish that a statement is actually implemented. Layout warnings remain visible; extra reading warnings, empty/unread pages and failed extraction prevent a threshold conclusion. Scanned PDF text can support a comparison only after separate OCR accuracy confirmation and validated model/renderer/page provenance. Unrecognized paraphrases may produce “needs evidence” or “needs human review”.

## Results and audit record

Results are potential issue, stated term meets this numeric threshold, needs context, needs evidence, needs human review, or outside scope. There is no overall document pass or compliance score. Every report records the document hash, source/raw-text hashes, engine/rule versions, human-confirmed scope, matched passages with original locations, provision quotes and limitations. Reports are historical snapshots; a changed original or rule requires a new review.

New reports use `complyph-labor-4` and contain six findings. Earlier reports keep their original findings, scope, engine and source identifiers. Missing meal/weekly-rest context or any SIL fact defaults to **not established**; rerunning an earlier context never assumes new eligibility. Loading does not append findings or rewrite historical review JSON/fingerprints. Scope display resolves by rule ID rather than finding position. Private schema 5 and public schema 4 remain unchanged.

## Service incentive leave scope

The SIL form requires employee coverage, qualifying service and exception/benefit review separately, plus a nonempty eligibility basis (at most 4,000 UTF-8 bytes, no control characters except line breaks/tabs). A confirmed exclusion or unmet qualifying service yields **outside scope**, without a threshold verdict. All three covered plus the basis allow the comparison; any uncertain fact/basis leaves it needing context. These facts are reviewer assertions, not independently verified records. The basis should identify the review period and the service, coverage, regular headcount and benefit/exemption evidence.

Rule V counts at least twelve months, continuous or broken, including authorized absences and paid regular holidays, and recognizes an agreed/established shorter work year. Article 95 / Rule V exceptions include existing SIL or vacation leave with pay of at least five days, establishments regularly employing **fewer than ten** employees and relevant exclusions; Article 95 also addresses Secretary-granted financial-viability exemptions. Ten employees does not satisfy the small-establishment exception. The reviewer must establish coverage and exceptions under the applicable law; job titles, probationary status and task/piece/commission pay alone are not automatic exclusions. Household work and other special regimes require their own analysis; outside this standard comparison does not mean no leave rights. See [source assessment and judicial guidance](SERVICE_INCENTIVE_LEAVE.md).

Supported SIL formats include `Annual service incentive leave is 5 days with pay.` and `Paid service incentive leave entitlement shall be five days per year.` Values zero through ten may be written out; decimals use exact hundredths. Payment and an annual period must be explicit. The parser retains semicolons/wrapped lines and rejects attached or explicit following conditions; unsupported headings, paraphrases, quotes, conflicting grants, unpaid/monthly leave and generic pooled vacation/sick leave require manual review. It does not infer eligibility from document text, calculate accrual/proration or cash conversion, determine actual leave use/payment, or enforce more favourable contractual terms. The offline viewer includes full Article 95, Article 82 and Book III Rule V sections 1–6, including commutation and benefit-protection provisions, without automating those additional questions.

## Independent validation cases

| Cases | Expected result |
| --- | --- |
| Normal hours 7.99 / 8 / 8.01 | Meets / meets / potential issue |
| Additional ordinary OT 24.99 / 25 / 25.01% | Potential issue / meets / meets |
| Additional night premium 9.99 / 10 / 10.01% | Potential issue / meets / meets |
| Regular meal time-off 59.99 / 60 / 60.01 minutes | Potential issue / meets / meets under confirmed standard scope |
| Weekly rest 23.99 / 24 / 24.01 consecutive hours after six consecutive normal work days | Potential issue / meets / meets under confirmed standard scope |
| Regular meal 90 minutes / weekly rest 48 consecutive hours after six days | Meets these numeric thresholds only |
| More favourable 7.5 hours / 30% / 15% | Meets these numeric thresholds only |
| SIL 4.99 / 5 / 5.01 annual paid days under established eligibility | Potential issue / meets / meets |
| SIL missing any eligibility fact or basis; unsupported/unpaid/pooled/conditional wording | Needs context / human review; no numeric verdict |
| SIL confirmed exemption/unmet service; earlier five-rule context/report | Outside scope; new eligibility unknown / original five findings retained |
| Unknown applicability or operative-term confirmation | Needs context |
| Outside scope / compressed or special arrangement | Outside scope when marked by reviewer |
| Missing statement, weekly-only hours | Needs evidence |
| All-in OT rate, incomplete night window, lunch-inclusive shift | Needs human review |
| Negation, conditional/example/quoted clause, conflicting values | Needs human review |
| Paid/exceptional meal, meal stated as a clock range or written-out hour; split rest or missing/different rest cadence | Needs human review |
| Shortened meal/rest marked outside the supported scope | Outside scope; no threshold conclusion |
| Missing meal/rest statement | Needs evidence |
| Earlier three-rule context / saved report | New scope remains unknown / historical report retains exactly three findings |
| Partial extraction / extra warning / scan | Needs human review |
| Changed source or derived-text hash | Desktop/diagnostic refuses to run |
| Actual PDF baseline / DOCX issues / ambiguous DOCX fixtures | Five meets / five potential issues / five unresolved, plus SIL needs context |

Tests live in `crates/checks/tests/validation.rs` and `crates/checks/tests/service_incentive_leave.rs`. Fixtures and independent expected outcomes live in `examples/expected-checks.json` and `examples/expected-sil.json`. Installed-executable checks also exercise the reader worker, rule evaluation and private report persistence. The engine has no filesystem, network or AI dependency; SHA-256 is used only on bytes passed by its caller.
