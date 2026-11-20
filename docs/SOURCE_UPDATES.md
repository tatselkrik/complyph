# Source update checks

ComplyPH checks explicitly approved official URLs and preserves changed downloads for review. Private documents are not updater inputs. The Windows release uses an AppContainer network worker; other-platform builds keep online checks disabled.

## Approve and check

1. Open **Legal library → Read local snapshot → Source updates and downloaded history**.
2. Inspect the official publication URL. Use a supported agency's HTTPS URL; it can be the source file or a publication index that you want to monitor.
3. Select **Approve daily checks**. No source is enabled by default, and importing a reference does not approve its URL.
4. The app checks due URLs while it is open. **Check approved sources now** runs an immediate batch, including retries of earlier failures.
5. Use **Update review needed** to find sources with changed downloads. Open their history, **Save downloaded copy…** to a new local file, compare it with the saved reference, and record your source review notes.

The supported agency roots are `dole.gov.ph`, `senate.gov.ph`, `privacy.gov.ph`, `bir.gov.ph`, `sss.gov.ph`, `philhealth.gov.ph`, `pagibigfund.gov.ph` and `officialgazette.gov.ph`, including their subdomains. Third-party mirrors and social-media URLs cannot be enabled. An imported mirror can instead use an explicitly approved official publication page.

**Disable checks** stops future requests and retains earlier downloads. Approval is separate from agency-download verification, source identity, applicability and checking-rule coverage.

## Schedule and outcomes

Daily means an attempt no more often than every 24 hours while the desktop is open. A missed/interrupted check becomes due after the next launch and expiry of its short worker lease. There is no installed background service or task that runs with ComplyPH closed. A batch handles up to 20 URLs; further due sources are picked up by subsequent batches. Manual checks can retry sooner.

Each source shows its last attempt, last successful check and any failure. A failed request does not advance its successful-check date. HTTP errors, connection/TLS/time limits, unexpected redirects and invalid/oversized bodies leave saved references and history intact.

The first successful response is compared with the preserved reference bytes. Later responses are compared with the last preserved server response. Matching bytes and valid 304 responses retain the existing copy; a different hash saves an immutable downloaded version and flags it for review. Repeated identical bodies do not duplicate a version. Returning to an earlier body is recorded as a change without deleting intervening versions. Interrupted/stale workers cannot commit after approval changes or a newer lease takes over.

## Review and legal currency

Downloads are public-reference candidates with their URL, resolved URL, time, type, byte count and SHA-256 hash. Earlier reference snapshots, downloads, citations and report hashes remain available. Export checks integrity and refuses to overwrite an existing destination. A recorded review note is retained with its time.

Recording a review does not promote a download to a verified agency reference, replace the reader's curated text or update an automated rule. The five numeric checks continue to cite their pinned source. If that publication has downloaded changes, Review displays a source-currency warning; existing reports still open their exact historical provisions. Updating a checking rule requires separate source/rule validation.

A webpage layout, navigation or protection-page change can alter bytes without amending the law. An unchanged URL does not prove that an agency published no other issuance. These checks do not crawl linked pages, discover all new laws, determine effectivity dates or reconcile amendments. Approving a publication index can help surface new links for human review. In-app source-diff interpretation and reviewed promotion of downloaded references remain future work.

## Privacy and limits

The coordinator opens the public catalog only. Its AppContainer child receives a small request containing the approved public URL and optional cache validators from that server. It receives no document path, text, hash, report, employer/employee data or review note. Requests are GET with no body, cookies, authentication or inherited proxy credentials. TLS validation remains enabled.

Windows provides the filesystem/network capability boundary. The child has InternetClient capability without private-network or document-library capabilities. It uses a read/execute-only temporary copy of application code and an explicit inherited-pipe list; it cannot inherit database/document handles. Tests verify denial of synthetic private-store, public-catalog and original-file canaries. Job objects stop the coordinator and its descendants when the desktop exits. The child also verifies its AppContainer token before constructing the HTTP client, so launching worker mode directly cannot bypass confinement.

The Rust HTTP policy further restricts requests to approved agency HTTPS origins on port 443. DNS answers containing private, loopback, link-local, reserved or mapped local addresses are rejected, and the checked addresses are passed directly to the connector. Redirects must stay on the same origin and are limited to three. Responses have a 16 KiB header limit and 32 MiB body limit; the worker has a 30-second wall deadline, CPU/memory limits and one-process limit. Unsupported payloads and compressed responses are rejected. Source downloads are preserved as PDF, DOCX, HTML or UTF-8 plain text; active HTML is never rendered by the app. External software used to open an exported file operates independently.

The public database migrates to schema 4. Private schema 3, `C:\ProgramData\ComplyPH`, original documents and earlier source/rule pins are preserved. Agency servers necessarily see normal IP/TLS connection metadata, the approved URL, the app user-agent and their own conditional-request headers. See [Privacy](../PRIVACY.md) and [verification](VERIFICATION.md).
