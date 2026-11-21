# Library maintenance

ComplyPH 0.12.0 separates preserved references, source-review decisions and executable checking rules. Maintenance is local. Only explicitly approved source URLs and official publication pages can be checked online; private documents are never updater inputs.

Open **Legal library → Maintain library**. The window has **Source details**, **Version comparison**, **Rule revalidation**, **Publication discovery** and **Audit history** tabs.

## Metadata and source identity

Choose a source in **Source details**. Edit its title, instrument, issuer, domains, official citation URL, issue/effective dates and applicability/effectivity notes. Blank dates mean unestablished; use real YYYY-MM-DD dates when known.

Enter a reviewer identifier, review date and factual basis, then **Save reviewed metadata**. The edit is an append-only annotation for that exact file version. Its original acquisition metadata, bytes, hashes and earlier citations are retained. Changing a citation does not enable requests to it. Relevant rule profiles require revalidation after a metadata change.

**Record identity decision** separately records awaiting verification, identity checked by the reviewer, or rejected/unsuitable. Confirm comparison with the official publication before recording identity checked. These are self-reported decisions, not agency certification, legal-currency guarantees, authenticated identities or digital signatures. An unconfirmed or rejected current identity holds dependent checks.

## Compare and adopt versions

1. Open a source's update history and choose **Compare and review candidate**, or select it in **Version comparison**.
2. Choose the current version on the left and a downloaded candidate on the right.
3. Select **Read candidate locally**. PDF/DOCX use the bounded document worker. A scanned PDF can use local OCR, subject to the existing page/time limits. HTML is reduced to inert UTF-8 reading text; scripts and external resources are not executed or fetched.
4. Inspect highlighted additions/removals and the complete reading sections. **Save right-hand original…** exports the exact public bytes to a new file for comparison with the source image/layout.
5. Record the reviewer, date and decision basis. Confirm the publication identity, the complete original/reading comparison and effectivity/amendment/scope review.
6. Choose **Adopt reviewed candidate**. A stale active-version selection, unreadable candidate, failed integrity check or missing confirmation prevents adoption.

The version pointer changes only after this reviewed action. Earlier original/text snapshots, candidates, decisions and saved report citations remain available. Adoption records the actual downloaded URL and a distinct **Official download · reviewed adoption** provenance. It does not become the starter collection's independently recorded agency-download verification.

Publication/effective dates are not inferred or copied as established dates for the adopted file. Review and record them in Source details. Source adoption does not edit thresholds, exceptions or supported legal-period profiles.

Dismiss an unsuitable download, duplicate or change with no rule impact using **Dismiss candidate with no rule impact** and a reason. The bytes remain preserved. An old source-review note is retained but does not substitute for this explicit adoption/dismissal decision.

Two previously preserved versions can also be compared. To roll back, compare the desired right-hand version against the current left-hand version, complete the same reviewed confirmations and choose **Restore this reviewed version**. Restoration is logged and dependent rules require assessment; it does not erase intervening history.

Diff computation has a 500 ms budget and a 20,000-line input ceiling. The display retains up to 2,000 difference/context rows, 2,000 characters per row and 60 rows per section. A visible truncation message directs the reviewer to the complete reading sections and original files. This is textual comparison, not an inference about legal meaning.

## Revalidate affected checks

The twenty compiled checks have explicit dependencies on seven source versions and their complete supporting provisions, including related provisions/implementing rules where applicable.

**Version comparison** lists the checks linked to the selected source; **Assess check** opens the corresponding rule assessment. **Compare this source's versions** returns from a rule to its source comparison.

**Rule revalidation** shows each check, its current source, pending candidates, complete-support comparison and assessment state:

| State | Behavior for a new local review |
| --- | --- |
| Current compiled profile | The active source matches its tested profile and has no unresolved source decision. |
| Revalidation required | A source, metadata or verification decision needs assessment; no numeric conclusion is activated. |
| Unchanged profile revalidated | A reviewer confirmed applicability/amendments and recorded an evaluation reference for the exact current source state. |
| Updated rule implementation required | Supporting text changed or a reviewed legal change requires rule work; the check remains held. |

Resolve pending source candidates and identity decisions first. To confirm an unchanged profile, every complete supporting provision must still match its compiled fingerprint. Record the reviewer/date/basis, review applicability, exceptions and amendments, and enter the test/evaluation reference and result. The assessment is bound to the source/text hashes, metadata/identity revisions, activation history, candidate decisions and compiled profile. Stale assessments cannot be applied to a changed state. A recorded future/invalid source effective date keeps profiles held, and an adopted source effective after the selected document period cannot support a numeric conclusion for that earlier period.

Changed supporting text cannot be authorized by a checkbox. A changed numeric requirement or interpretation needs a newly implemented, reviewed and tested app/rule release. Source metadata edits do not add checking rules or extend the initial profiles beyond their supported CY2026 period.

For a reviewed new issuance affecting an existing rule, use **Record rule update required** with its citation and legal impact in the basis. Publication discovery also provides **Assess rule impact…**, which carries the selected citation into the recorded assessment. These legal-change holds survive unrelated edits, restoration and app upgrades. Resolving a previous update requirement requires explicitly acknowledging its current assessment ID as well as the unchanged-support, scope, review-basis and evaluation gates. The old decision and its resolution remain in history.

New reviews mark affected findings **Needs human review**, retain document evidence and show the library-maintenance reason; explicitly excluded checks stay outside scope. An unchanged-profile revalidation binds future findings to the adopted source's actual hashes. Saved JSON and PDF reports preserve the assessment, source version and reviewer basis. Earlier reports are never rewritten and continue opening their own exact offline provisions.

Technical verification of these gates is separate from qualified Philippine legal review, which remains in roadmap item 6.

## Discover official publication links

In **Publication discovery**, enter an official publication/listing page, select its compliance domain and choose **Approve daily discovery**. No page is approved by default. Use **Check approved sources and pages** or let the open desktop check due approvals daily.

Discovery inventories qualifying links from the approved page's HTML: PDF/DOCX paths and labels containing supported issuance terms such as circular, advisory, department order, revenue regulation/memorandum, republic act, board resolution or implementing rules. Relative URLs are resolved, fragments/duplicates removed and unsupported/nonofficial targets rejected. A citation to another supported agency can appear, but is not automatically approved or fetched.

- The first scan is labelled **Initial inventory**.
- A link first seen on a later scan is labelled **Newly seen link**.
- Neither label establishes a publication date, new enactment, effectivity or applicability.
- Discovery never follows the links, executes page scripts, crawls linked archives or downloads/adopts the discovered documents.

Choose **Add downloaded legal reference…** to prefill the local Add legal source form. Supply the public file and review its issuer, instrument, dates and identity. Choose **Assess rule impact…** to evaluate particular checking rules, or record a dismissal/reopening with its basis. Disabling the page preserves its previous responses, links and decisions.

Scans require UTF-8 HTML of at most 1 MiB and retain up to 500 unique qualifying links per page. Reaching the limit is reported as partial. Dynamic portals, unlinked issuances, unsupported encodings and archives outside an approved page require manual intake or separately approved pages. This bounded inventory does not promise discovery of every relevant law.

## Storage, isolation and upgrades

Public schema 5 stores active-version pointers, immutable candidates/readings/snapshots, metadata and identity events, adoption/restoration/dismissal history, compiled dependencies, rule assessments/hold resolutions and publication inventories. Migration from public schema 4 keeps the same previously selected versions and all earlier bytes/notes. Older applications refuse the newer public schema.

Private schema 5 and Windows **C:\ProgramData\ComplyPH** remain unchanged. Source maintenance does not open private document data.

Windows keeps its AppContainer downloader. Linux source builds add Landlock ABI 3+ filesystem confinement and seccomp before input/HTTP, with exact protected DNS/NSS runtime-file access, cleared environment and inherited descriptors, memory/CPU/time limits, no local Unix sockets, execution, process-memory access or writable/private paths. Release CI verifies the Linux x86-64 file-denial boundary; unavailable kernel support fails closed before networking. Other unsupported platforms remain offline.

The public-only GET protocol, agency HTTPS allowlist, public-address resolver, same-origin redirect policy and no-body/no-cookie/no-credential behavior are retained. See [source update controls](SOURCE_UPDATES.md) and [Privacy](../PRIVACY.md).

## Verification

The suite includes independently expected text changes, malformed metadata/dates, absent confirmations, stale decisions, altered/ambiguous supporting text, durable legal-change holds, rollback, schema migration, historical-source/report preservation, real PDF preparation and synthetic official-listing discovery. Installed-Windows checks repeat six end-to-end maintenance workflows, verify PDF citations and compare private/original hashes. All test documents, listings and source changes are fictional.
