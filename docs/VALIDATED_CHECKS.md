# Focused labour checks, version 1

“Validated” means source-anchored, technically tested numeric comparisons with explicit scope gates. No qualified Philippine lawyer has certified these rules. They are not a legal opinion or a complete compliance assessment.

## Source and thresholds

All three rules pin the preserved Senate P.D. 442 reference compilation, `labor-code-reference`. Original SHA-256: `99fd38f1434225504d0685fc5a92c4bb305a9e95bc63875d0781423d4acfa4c0`; reading text SHA-256: `c9e5a66dafa4aa6fc55c01347059c7abe15af841ac30b0ec0d74ba60823bd31f`. Both must match before desktop/diagnostic checking. The validation suite checks every quoted provision against that source. The uploaded DOLE 2022 edition corroborates the articles but does not replace the agency download pin.

| Rule | Provision and comparison | Required human-confirmed scope |
| --- | --- | --- |
| PH-LAB-083-01 | Article 83, read with 82/84/85: normal hours at most 8 per day | Covered adult employment; standard arrangement; net working hours; no compressed/flexible, health-personnel or other special arrangement |
| PH-LAB-087-01 | Article 87: ordinary-day overtime additional premium at least 25% | Covered employment, beyond 8 hours on ordinary working days; additional premium on regular hourly wage, not an all-in rate |
| PH-LAB-086-01 | Article 86 / Book III Rule II: additional night differential at least 10% | Covered employment and exceptions checked; each worked hour 22:00–06:00; regular hourly wage basis |

Coverage is not inferred from a job title. The UI lists Article 82 exclusions and the night-differential retail/service exception for establishments with not more than five workers. The reviewer must establish applicable amendments, employment coverage and review period. Government, managerial/staff, field/unsupervised, dependent family, domestic/personal service and qualifying paid-by-results work need appropriate exclusion analysis. More favourable contract/CBA terms, holiday/rest-day pay, payroll execution and other domains are outside these comparisons.

## Supported statements

Examples of recognized wording (illustrations, **not employment templates**):

```text
Normal daily working hours are 8 hours.
Ordinary-day overtime premium is 25% of the regular hourly wage.
Night shift differential is 10% of the regular hourly wage for hours worked between 22:00 and 06:00.
```

Other supported patterns are listed directly in `crates/checks/src/lib.rs`. Unsupported clauses stay unresolved. The parser deliberately does not infer 25% from an unqualified “125% overtime rate”, subtract lunch from a shift span, convert weekly hours, or resolve conflicting/conditional provisions. It is case-insensitive and compares decimals as integer hundredths, without rounding.

A human must compare extraction with the original and confirm that statements are operative terms, not quoted examples. No rule can establish that a statement is actually implemented. Layout warnings remain visible; extra reading warnings, empty/unread pages, scans and failed extraction prevent a threshold conclusion. Unrecognized paraphrases may produce “needs evidence” or “needs human review”.

## Results and audit record

Results are potential issue, stated term meets this numeric threshold, needs context, needs evidence, needs human review, or outside scope. There is no overall document pass or compliance score. Every report records the document hash, source/raw-text hashes, engine/rule versions, human-confirmed scope, matched passages with original locations, provision quotes and limitations. Reports are historical snapshots; a changed original or rule requires a new review.

## Independent validation cases

| Cases | Expected result |
| --- | --- |
| Normal hours 7.99 / 8 / 8.01 | Meets / meets / potential issue |
| Additional ordinary OT 24.99 / 25 / 25.01% | Potential issue / meets / meets |
| Additional night premium 9.99 / 10 / 10.01% | Potential issue / meets / meets |
| More favourable 7.5 hours / 30% / 15% | Meets these numeric thresholds only |
| Unknown applicability or operative-term confirmation | Needs context |
| Outside scope / compressed or special arrangement | Outside scope when marked by reviewer |
| Missing statement, weekly-only hours | Needs evidence |
| All-in OT rate, incomplete night window, lunch-inclusive shift | Needs human review |
| Negation, conditional/example/quoted clause, conflicting values | Needs human review |
| Partial extraction / extra warning / scan | Needs human review |
| Changed source or derived-text hash | Desktop/diagnostic refuses to run |
| Actual PDF baseline / DOCX issues / ambiguous DOCX fixtures | Three meets / three potential issues / three unresolved |

Tests live in `crates/checks/tests/validation.rs`. Fixtures and independent expected outcomes live in `examples/expected-checks.json`. Installed-executable checks also exercise the reader worker, rule evaluation and private report persistence. The engine has no filesystem, network or AI dependency; SHA-256 is used only on bytes passed by its caller.
