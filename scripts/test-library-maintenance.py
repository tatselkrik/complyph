"""Installed-app smoke test. Fictional inputs, explicit new directory, no HTTP requests."""
import argparse
import hashlib
import json
import sqlite3
import subprocess
from pathlib import Path


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, required=True)
    parser.add_argument("--samples", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    assert args.app.is_file() and args.samples.is_dir()
    args.work.mkdir(parents=True, exist_ok=False)
    root = args.work / "data"
    library = root / "library"
    private = root / "private" / "workspace.sqlite3"
    sequence = 0

    def invoke(arguments, output, success=True):
        result = subprocess.run([str(args.app), *map(str, arguments)],
                                capture_output=True, timeout=90)
        assert (result.returncode == 0) == success, (
            f"Installed operation failed ({result.returncode}): "
            + result.stderr.decode(errors="replace")[:1000])
        return json.loads(output.read_text(encoding="utf-8-sig")) if success else None

    def action(request, success=True):
        nonlocal sequence
        sequence += 1
        request_file = args.work / f"{sequence}-request.json"
        output = args.work / f"{sequence}-result.json"
        request_file.write_text(json.dumps(request), encoding="utf-8")
        return invoke(["--library-action", "--metadata", request_file,
                       "--output", output, "--library-dir", library], output, success)

    def state():
        return action({"action": "state", "source_id": None})

    reviewer = {"actor": "Synthetic installed library reviewer", "review_on": "2026-11-21",
                "basis": "Fictional smoke case only. Compared complete support, scope and amendments; "
                         "the tested numeric model is unchanged. No real legal certification."}

    def approval(current_hash):
        return {"reviewer": reviewer, "expected_active_sha256": current_hash,
                "identity_confirmed": True, "original_and_reading_compared": True,
                "effectivity_and_scope_reviewed": True}

    def check(label):
        cases = json.loads((args.samples / "expected-initial.json").read_text())
        context = args.work / f"{label}-scope.json"
        output = args.work / f"{label}-review.json"
        context.write_text(json.dumps(cases["cases"]["Synthetic seven-domain baseline.docx"]["context"]))
        return invoke(["--check-file", original, "--context", context, "--output", output,
                       "--data-dir", root], output)

    def seeded_candidate(source_id, text):
        # Deliver known fictional public bytes into this isolated fixture catalog.
        # Never run the updater or label the fixture as a real agency response.
        with sqlite3.connect(library / "catalog.sqlite3") as db:
            old = json.loads(db.execute(
                "SELECT p.metadata_json FROM active_source_versions a "
                "JOIN source_snapshots p ON p.version_id=a.version_id WHERE a.source_id=?",
                (source_id,)).fetchone()[0])
            raw = text.encode("utf-8")
            db.execute("INSERT INTO downloaded_source_changes(source_id,url,resolved_url,"
                       "downloaded_at,content_sha256,baseline_sha256,raw_bytes,mime) "
                       "VALUES(?,?,?,?,?,?,?,?)",
                       (source_id, old["source_url"], old["source_url"], 1795219200,
                        sha(raw), old["raw_sha256"], raw, "text/plain"))
            return db.execute("SELECT last_insert_rowid()").fetchone()[0]

    def assess(rule, success=True, supersedes=None):
        review = {"expected_token": rule["token"], "unchanged_profile": True,
                  "applicability_and_amendments_reviewed": True,
                  "test_reference": "Installed synthetic matrix: SSS 15% / 10% / 5%; boundary "
                                    "and seven-domain fixture results unchanged.",
                  "reviewer": reviewer}
        if supersedes is not None:
            review["supersedes_update_assessment"] = supersedes
        return action({"action": "revalidate", "rule_id": rule["dependency"]["rule_id"],
                       "review": review}, success)

    original = args.samples / "Synthetic seven-domain baseline.docx"
    original_hash = sha(original.read_bytes())
    first = check("original")
    first_review = first["review"]
    assert len(first_review["findings"]) == 20
    initial = state()
    sss = next(s for s in initial["sources"] if s["metadata"]["key"] == "ra-11199")
    source_id = sss["id"]
    original_source_hash = sss["metadata"]["raw_sha256"]
    with sqlite3.connect(library / "catalog.sqlite3") as db:
        original_source_metadata, original_raw, original_text = db.execute(
            "SELECT p.metadata_json,p.raw_bytes,p.reading_text FROM active_source_versions a "
            "JOIN source_snapshots p ON p.version_id=a.version_id WHERE a.source_id=?",
            (source_id,)).fetchone()
    private_before = sha(private.read_bytes())
    details = {"title": "SYNTHETIC reviewed SSS annotation", "instrument": sss["metadata"]["instrument"],
               "issuer": sss["metadata"]["issuer"], "official_url": sss["metadata"]["source_url"],
               "domains": sss["metadata"]["domains"], "issued_on": "2026-11-01",
               "applicability_note": "Fictional annotation. Original acquisition remains preserved."}
    action({"action": "metadata", "source_id": source_id,
            "expected_sha256": original_source_hash, "details": details, "reviewer": reviewer})
    for decision in ["rejected", "identity_checked"]:
        action({"action": "identity", "source_id": source_id, "expected_sha256": original_source_hash,
                "decision": decision, "reviewer": reviewer})
        if decision == "rejected":
            assert all(not r["identity_ready"] for r in state()["rules"]
                       if r["dependency"]["source_key"] == "ra-11199")
    history = action({"action": "state", "source_id": source_id})["source"]["history"]
    assert {h["kind"] for h in history} >= {"metadata", "identity"}
    with sqlite3.connect(library / "catalog.sqlite3") as db:
        assert db.execute("SELECT metadata_json FROM source_snapshots p JOIN source_versions v "
                          "ON v.id=p.version_id WHERE v.source_id=? AND v.content_sha256=?",
                          (source_id, original_source_hash)).fetchone()[0] == original_source_metadata
    assert sha(private.read_bytes()) == private_before
    change = seeded_candidate(source_id, original_text + "\nSYNTHETIC editorial fixture footer. NOT A LAW.")
    held = check("pending")
    for before, after in zip(first_review["findings"], held["review"]["findings"]):
        assert after["outcome"] == ("needs_review" if before["rule_id"].startswith("PH-SSS-")
                                    else before["outcome"])
    private_before = sha(private.read_bytes())
    action({"action": "prepare", "change_id": change})
    difference = action({"action": "compare_candidate", "change_id": change})
    assert difference["added_lines"] > 0 and difference["removed_lines"] == 0
    denied = approval(original_source_hash)
    denied["identity_confirmed"] = False
    action({"action": "adopt", "change_id": change, "review": denied}, False)
    adopted = action({"action": "adopt", "change_id": change, "review": approval(original_source_hash)})
    adopted_hash = adopted["raw_sha256"]
    assert adopted["provenance"] == "reviewed_download"
    assert adopted["issued_on"] is None and adopted["effective_from"] is None
    before_revalidation = state()
    stale = next(r for r in before_revalidation["rules"] if r["dependency"]["rule_id"] == "PH-SSS-TOTAL-01")
    for rule in before_revalidation["rules"]:
        if rule["dependency"]["source_key"] == "ra-11199":
            assert rule["status"] == "revalidation_required"
            assess(rule)
    assert sha(private.read_bytes()) == private_before
    revalidated = check("revalidated")
    for before, after in zip(first_review["findings"], revalidated["review"]["findings"]):
        assert after["outcome"] == before["outcome"]
        if before["rule_id"].startswith("PH-SSS-"):
            assert after["source"]["sha256"] == adopted_hash
            assert after["library_review"]["assessment_id"] > 0
    version_diff = action({"action": "compare_versions", "source_id": source_id,
                           "left_sha256": original_source_hash, "right_sha256": adopted_hash})
    assert version_diff["added_lines"] > 0
    report = args.work / "revalidated-report.pdf"
    result = subprocess.run([str(args.app), "--export-review-pdf", str(revalidated["id"]),
                             "--output", str(report), "--data-dir", str(root)],
                            capture_output=True, timeout=90)
    assert result.returncode == 0
    reading_output = args.work / "report-reading.json"
    reading = invoke(["--read-file", report, "--output", reading_output, "--data-dir", root], reading_output)
    assert reading["report"]["status"] == "ready"
    report_text = " ".join(u["text"] for u in reading["report"]["units"])
    report_text = " ".join(report_text.split())
    for needle in [adopted_hash, reviewer["actor"], "Library decision token", "Installed synthetic matrix"]:
        assert needle in report_text, f"Report missing {needle}"
    private_before = sha(private.read_bytes())
    details["applicability_note"] = "SYNTHETIC changed review context; stale assessments must fail."
    action({"action": "metadata", "source_id": source_id, "expected_sha256": adopted_hash,
            "details": details, "reviewer": reviewer})
    assess(stale, False)
    with sqlite3.connect(library / "catalog.sqlite3") as db:
        text = db.execute("SELECT p.reading_text FROM active_source_versions a JOIN source_snapshots p "
                          "ON p.version_id=a.version_id WHERE a.source_id=?", (source_id,)).fetchone()[0]
    rule_source = next(r for r in state()["rules"] if r["dependency"]["rule_id"] == "PH-SSS-TOTAL-01")
    start = rule_source["dependency"]["provisions"][0]["start"]
    altered = text.replace(start, "SYNTHETIC ALTERED SUPPORT — NOT REAL LAW", 1)
    assert altered != text
    changed = seeded_candidate(source_id, altered)
    action({"action": "prepare", "change_id": changed})
    action({"action": "adopt", "change_id": changed, "review": approval(adopted_hash)})
    changed_state = state()
    rule = next(r for r in changed_state["rules"] if r["dependency"]["rule_id"] == "PH-SSS-TOTAL-01")
    assert rule["status"] == "update_required"
    assess(rule, False)
    update = {"expected_token": rule["token"], "unchanged_profile": False,
              "applicability_and_amendments_reviewed": True, "test_reference": "Synthetic updated rule work required.",
              "reviewer": reviewer}
    action({"action": "revalidate", "rule_id": rule["dependency"]["rule_id"], "review": update})
    current_hash = next(r for r in state()["rules"] if r["dependency"]["rule_id"] == "PH-SSS-TOTAL-01")["active_sha256"]
    action({"action": "restore", "source_id": source_id, "sha256": original_source_hash,
            "review": approval(current_hash)})
    restored = next(r for r in state()["rules"] if r["dependency"]["rule_id"] == "PH-SSS-TOTAL-01")
    assert restored["update_hold_id"] is not None and restored["status"] == "update_required"
    assess(restored, False)
    assess(restored, supersedes=restored["update_hold_id"])
    assert sha(private.read_bytes()) == private_before

    page = "https://privacy.gov.ph/complyph-fixtures/listing/"
    parsed = []
    for filename, count in [("Synthetic publication listing.html", 3),
                            ("Synthetic updated publication listing.html", 4)]:
        preview = action({"action": "preview_publication", "url": page,
                          "file": str(args.samples / filename)})
        assert len(preview["links"]) == count and not preview["truncated"]
        parsed.append(preview)
    action({"action": "approve_publication_page", "url": page, "domain": "ph.privacy"})
    page_id = state()["publication_pages"][0]["id"]
    # Synthetic delivery uses the installed parser's output; no link is fetched.
    with sqlite3.connect(library / "catalog.sqlite3") as db:
        for index, (filename, inventory) in enumerate(zip(
                ["Synthetic publication listing.html", "Synthetic updated publication listing.html"], parsed)):
            raw = (args.samples / filename).read_bytes()
            db.execute("INSERT INTO publication_versions(page_id,sha256,raw_bytes,retrieved_at) VALUES(?,?,?,?)",
                       (page_id, sha(raw), raw, 1795219200 + index))
            for url, title in inventory["links"]:
                db.execute("INSERT OR IGNORE INTO publication_links(page_id,url,title,first_seen,last_seen,initial_scan) "
                           "VALUES(?,?,?,?,?,?)", (page_id, url, title, 1795219200 + index, 1795219200 + index, index == 0))
    inventory = state()
    assert len(inventory["discovered_issuances"]) == 4
    assert sum(not l["initial_scan"] for l in inventory["discovered_issuances"]) == 1
    link = inventory["discovered_issuances"][0]
    for dismiss in [True, False]:
        action({"action": "review_citation", "link_id": link["id"], "dismiss": dismiss, "reviewer": reviewer})
        assert next(l for l in state()["discovered_issuances"] if l["id"] == link["id"])["dismissed"] == dismiss
    action({"action": "disable_publication_page", "page_id": page_id})
    assert not state()["publication_pages"][0]["enabled"]
    assert state()["summary"]["source_count"] == 19
    assert sha(private.read_bytes()) == private_before
    assert sha(original.read_bytes()) == original_hash
    old_output = args.work / "historical-review.json"
    old_state = invoke(["--review-state", first["id"], "--output", old_output, "--data-dir", root], old_output)
    assert old_state["saved"]["review"] == first_review
    with sqlite3.connect(library / "catalog.sqlite3") as db:
        assert db.execute("PRAGMA user_version").fetchone()[0] == 5
        saved = db.execute("SELECT p.raw_bytes FROM source_snapshots p JOIN source_versions v ON v.id=p.version_id "
                           "WHERE v.source_id=? AND v.content_sha256=?", (source_id, original_source_hash)).fetchone()[0]
        assert saved == original_raw
    names = ["metadata_and_identity", "reviewed_version_lifecycle", "source_rule_revalidation",
             "changed_support_and_stale_guards", "durable_legal_hold", "official_publication_discovery"]
    flags = ["installed_metadata_history_passed", "installed_source_identity_passed",
             "installed_source_comparison_passed", "installed_reviewed_adoption_passed",
             "installed_rule_revalidation_passed", "installed_legal_hold_persistence_passed",
             "installed_publication_discovery_passed", "maintenance_reports_preserved",
             "maintenance_private_boundary_passed", "maintenance_pdf_citations_passed"]
    report = {"public_schema": 5, "workflow_cases": [{"name": n, "passed": True} for n in names],
              "checks": {name: True for name in flags}, "synthetic_publication_fixture_count": 2,
              "source_original_sha256": original_source_hash, "source_adopted_sha256": adopted_hash}
    with args.output.open("x", encoding="utf-8") as output:
        json.dump(report, output, indent=2)
    print("Six installed public-library workflows passed; private data and historical reviews preserved.")


if __name__ == "__main__":
    main()
