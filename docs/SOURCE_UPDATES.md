# Source update checks

ComplyPH checks explicitly approved official URLs and preserves changed downloads for review. It can also inventory publication links from separately approved agency pages. Private documents are never updater inputs. Windows uses AppContainer; supported Linux kernels use Landlock/seccomp. Unsupported platforms or unavailable isolation stay offline.

## Approve and check

1. Open **Legal library → Read local snapshot → Source updates and downloaded history**.
2. Inspect the official source URL and choose **Approve daily checks**. Use the actual publication URL for reference versions; **Discover official issuances** separately configures publication/listing pages.
3. No URL or publication page is approved by default. Importing a source or editing its citation does not approve a network request.
4. The open desktop checks due approvals. **Check approved sources now** runs an immediate batch, including earlier failures.
5. Use **Update review needed → Compare and review candidate** to inspect, read and deliberately adopt or dismiss changed bytes. See the [maintenance guide](LIBRARY_MAINTENANCE.md).

Allowed agency roots are dole.gov.ph, senate.gov.ph, privacy.gov.ph, bir.gov.ph, sss.gov.ph, philhealth.gov.ph, pagibigfund.gov.ph and officialgazette.gov.ph, including their subdomains. Third-party mirrors and social-media URLs cannot be enabled. A mirror reference may use a separately approved official URL after reviewing its identity.

**Disable checks** stops future requests and retains downloads. Approval remains separate from source identity, legal applicability and checking-rule coverage.

## Schedule and outcomes

A completed attempt becomes due after 24 hours while the desktop is open. Interrupted leases can retry after expiry; missed checks are handled at the next launch. There is no background service when the app is closed. Manual checks can retry sooner.

A batch makes at most twenty source/page requests. When publication pages are due, it handles up to ten source URLs and ten pages; further due entries are picked up by later batches. Short-lived leases prevent concurrent duplicate work, and late/revoked responses cannot commit.

Source and page status show attempts, successful checks and failures. HTTP/TLS/time/size/origin failures retain earlier references and do not advance the successful-check date. Different source bytes are saved immutably with their URL, resolved URL, time, MIME type, size and SHA-256. Identical bodies do not duplicate a version; a valid conditional 304 retains the previous copy. Returning to earlier bytes does not erase intervening versions.

## Reviewed changes and rule currency

A download is a reference candidate. Metadata editing, identity review, textual comparison, adoption/restoration, dismissal and rule assessment are separate deliberate local actions with a reviewer/date/basis history. Old review notes remain, but do not substitute for a new adoption/dismissal decision.

Source changes hold affected checks until the appropriate decision/revalidation. A complete unchanged supporting provision plus applicability/amendment/evaluation review can revalidate an existing compiled profile against the adopted source. Changed supporting text needs a new reviewed/tested rule implementation; editing source metadata cannot change numeric thresholds or legal-period support. Recorded future effectivity also prevents premature activation. Earlier reports continue opening their exact historical offline source.

## Official publication discovery

Approve an official HTML listing in **Publication discovery**. Scans preserve page versions and qualifying official PDF/DOCX/issuance links. Initial inventory is distinguished from later newly seen links; neither establishes a new enactment or issue/effective date. Links can be dismissed/reopened with notes, used to prefill local Add legal source, or attached to a particular rule-impact assessment.

Discovery does not follow the links, fetch their documents, execute page scripts or crawl other archives. It accepts UTF-8 HTML up to 1 MiB and inventories up to 500 qualifying unique links per page. Limits, empty/static results and failures are visible. Dynamic portals, unlinked issuances and unsupported pages require manual intake or another explicitly approved page. This is bounded inventory, not exhaustive legal research.

## Privacy boundary

The coordinator opens the public catalog only. Its network child receives an approved public URL and optional ETag/Last-Modified values from that server. Requests are HTTPS GET without a body, cookies, authentication or inherited proxy credentials. Document paths, text, hashes, findings, employer/employee information and reviewer notes are not request inputs. TLS validation remains enabled.

Windows confines the worker in AppContainer with InternetClient capability, read/execute access to application code and an explicit inherited-pipe list. It cannot inherit document/catalog handles. File-denial probes cover synthetic private-store, public-catalog and original-file canaries; job objects stop the coordinator/descendants with the desktop. The child verifies its token before reading a request or creating an HTTP client.

Linux source builds require working Landlock ABI 3+ and seccomp. Before input/HTTP, the worker applies deny-by-default filesystem access, permitting only exact protected system DNS/NSS runtime files. Its environment and nonstandard inherited descriptors are cleared; seccomp blocks Unix sockets, child executables and process-memory/handle bypasses. Time/CPU/memory limits and parent-death guards bound its lifetime. Unavailable confinement fails before an online request. Release CI exercises real file denial on Linux x86-64.

The HTTP policy further requires approved agency HTTPS origins on port 443. DNS answers containing private/reserved/local addresses are rejected; the checked addresses are used directly by the connector. Redirects stay on the approved origin and are limited to three. Responses have a 16 KiB header and 32 MiB body limit, with time/resource bounds. Unsupported/compressed responses are rejected. Public PDF/DOCX/HTML/UTF-8 text is preserved; active HTML is never rendered. Other software opening exported originals operates independently.

Public schema 5 preserves earlier source versions, approvals, downloads and notes while adding reviewed maintenance/discovery. Private schema 5, C:\ProgramData\ComplyPH and original documents remain unchanged. Agencies see normal IP/TLS metadata, the approved URL, app user-agent and their own conditional-request headers. See [Privacy](../PRIVACY.md) and [verification](VERIFICATION.md).
