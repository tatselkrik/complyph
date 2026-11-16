# Compliance domains

## Product direction

LaborLens PH is intended to grow into a local Philippine compliance document-review application. Labor compliance is the first review area. Future domains can reuse the same desktop workspace, document extraction, evidence handling, rule evaluation, and reporting pipeline.

This document is an architecture plan. v0.1.0 provides file registration and local storage; it does not yet implement checking rules or any of the domains below. The application and repository keep their current name.

| Planned domain | Examples of future review inputs |
| --- | --- |
| Labor law | Employment agreements, HR policies, and related records |
| Occupational safety and health — DOLE OSHS | Safety programs, policies, training records, and supporting safety records |
| Data privacy — Data Privacy Act and NPC issuances | Privacy notices, data-handling policies, and processing records |
| Tax — BIR requirements | Payroll and tax documents with their supporting records |
| Social security — SSS | Contribution schedules and supporting remittance records |
| Health insurance — PhilHealth | Contribution schedules and supporting remittance records |
| Housing fund — Pag-IBIG | Contribution schedules and supporting remittance records |

These are candidate areas of coverage, not a claim that every obligation can be checked from a document. Some checks require structured facts or supporting records, and some require human review.

## Shared engine

The common Rust pipeline should provide:

- Local document extraction with page, paragraph, and field references.
- A local review context and evidence store, including employer attributes and the period under review where relevant.
- A domain registry and applicability resolver.
- Deterministic rule evaluation, including validated domain-specific Rust evaluators when needed.
- Findings with evidence locations, source provisions, missing facts, and review status.
- Reports that preserve the evaluated domains, rule versions, source versions, and coverage.

Domain definitions supply their own approved sources, applicability conditions, required facts and evidence, checking rules, evaluator references, and validation cases. Adding a domain should reuse the shared intake, storage, and reporting flow. Financial checks should use exact decimal arithmetic and versioned calculation inputs.

Keep domain definitions as validated data and reviewed application code. Public source downloads must not introduce executable plugins or unreviewed rule logic.

## Sources, domains, and regulators

Use stable domain identifiers independent of display names, such as `ph.labor`, `ph.osh`, `ph.privacy`, `ph.tax`, `ph.sss`, `ph.philhealth`, and `ph.pagibig`. These are proposed identifiers; they are not an implemented registry or an exhaustive enum.

Keep the following relationships explicit in future storage migrations:

- A regulator or issuer can publish sources for several domains.
- A source can support several domains without duplicating its acquired content or provenance.
- A domain can include several statutes, regulations, orders, circulars, and other approved issuances.
- A rule version can cite several specific source versions and provisions.
- A review can select several domains and evaluate only the rules applicable to its facts and review period.

The current catalog has generic sources and source versions. Domain membership, rule definitions, rule-to-source references, and structured review evidence still need schema design and migrations before implementation.

## Applicability and evidence

Keep publication date, retrieval date, effective period, and the period under review distinct. The newest downloaded source may not be the version applicable to an earlier payroll or reporting period.

Each rule should declare the facts it needs. Shared employer context can include attributes such as location, industry, and coverage period; a domain can request additional local data when a supported check needs it. Preserve where a fact came from so the reviewer can inspect it.

Report missing evidence, missing context, unsupported checks, and findings separately. A missing remittance receipt yields a missing-evidence state; a conclusion about payment requires appropriate supporting records. An evaluated rule with no issue found must retain its scope and evidence references.

## Local processing and library updates

Domain expansion keeps private documents, extracted text, structured facts, and findings on the computer. The updater checks approved public sources independently of private reviews. It must not send document-derived queries or employer, employee, payroll, or review data to a regulator or other service.

Track source acquisition, verification, freshness, and rule coverage separately for each domain. Automatically preserving a new source version does not establish a correct new checking rule. Manual source addition should retain provenance and allow relevant domain assignments without silently activating an interpretation.

The updater still requires operating-system file and network restrictions before networking is enabled, as described in [Architecture](ARCHITECTURE.md).

## Interface and naming

Use domain selection inside the common Review workspace. A saved local review profile can select several domains and retain the context needed for that review. Show findings and coverage per domain, with library filters for domain and issuer. The navy-and-cream interface remains the shared visual language.

Keep domain, source, and rule identifiers independent of product branding. Any future product rename must account for the existing data directory, installer identity, and saved workspace. No rename or storage-path change is part of this plan.

## Delivery

Complete one supported labor review from intake to a cited report first. Validate its source mapping, applicability, evidence handling, findings, and missed or incorrect flags. Extend the same interfaces to additional domains as their rules and evidence requirements are ready. The [roadmap](ROADMAP.md) tracks the implementation sequence.
