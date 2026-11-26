"""Installed-app document revision checks. New isolated workspace, synthetic files, no HTTP."""
import argparse
import hashlib
import json
import shutil
import sqlite3
import subprocess
from pathlib import Path


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def public_snapshot_hash(path):
    digest = hashlib.sha256()
    with sqlite3.connect(path) as db:
        for row in db.execute("SELECT version_id,raw_bytes,reading_text,metadata_json,verified_at FROM source_snapshots ORDER BY version_id"):
            for field in row:
                value = field if isinstance(field, bytes) else str(field).encode("utf-8")
                digest.update(len(value).to_bytes(8, "big"))
                digest.update(value)
    return digest.hexdigest()


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
    sequence = 0
    expected = json.loads((args.samples / "expected-revisions.json").read_text())
    context = args.work / "scope.json"
    context.write_text(json.dumps(expected["context"]), encoding="utf-8")
    fixtures = {name: sha(args.samples / name) for name in [
        "Synthetic revision original.docx", "Synthetic revision corrected.docx",
        "Synthetic revision missing term.docx",
        "Synthetic revision original.pdf", "Synthetic revision corrected.pdf",
    ]}

    def check(path, scope=context, data=root):
        # Developer switches preserve the same positional ordering used by the installer.
        nonlocal sequence
        sequence += 1
        output = args.work / f"{sequence}-check.json"
        result = subprocess.run([str(args.app), "--check-file", str(path),
                                 "--context", str(scope), "--output", str(output),
                                 "--data-dir", str(data)], capture_output=True, timeout=120)
        assert result.returncode == 0, result.stderr.decode(errors="replace")[:1200]
        return json.loads(output.read_text(encoding="utf-8-sig"))

    def action(request, data=root, success=True):
        nonlocal sequence
        sequence += 1
        metadata = args.work / f"{sequence}-request.json"
        output = args.work / f"{sequence}-action.json"
        metadata.write_text(json.dumps(request), encoding="utf-8")
        public_path = data / "library/catalog.sqlite3"
        public_hash = sha(public_path) if public_path.exists() else None
        result = subprocess.run([str(args.app), "--document-action", "--metadata", str(metadata),
                                 "--output", str(output), "--data-dir", str(data)],
                                capture_output=True, timeout=120)
        assert (result.returncode == 0) == success, (
            f"Unexpected document-action exit {result.returncode}: "
            + result.stderr.decode(errors="replace")[:1200])
        assert (sha(public_path) if public_path.exists() else None) == public_hash, (
            "A private revision action changed or created a public source catalog")
        return json.loads(output.read_text(encoding="utf-8-sig")) if success else None

    def history(document_id, data=root):
        return action({"action": "history", "document_id": document_id}, data)

    def reading_id(document_id, review_id):
        return next(r["reading_id"] for v in history(document_id)["versions"]
                    for r in v["reviews"] if r["id"] == review_id)

    def review_state(review_id):
        nonlocal sequence
        sequence += 1
        output = args.work / f"{sequence}-state.json"
        result = subprocess.run([str(args.app), "--review-state", str(review_id),
                                 "--output", str(output), "--data-dir", str(root)],
                                capture_output=True, timeout=120)
        assert result.returncode == 0, result.stderr.decode(errors="replace")[:1200]
        return json.loads(output.read_text(encoding="utf-8-sig"))

    def mark_resolved(review_id):
        nonlocal sequence
        state = review_state(review_id)
        sequence += 1
        metadata = args.work / f"{sequence}-note.json"
        output = args.work / f"{sequence}-noted.json"
        request = {"review_fingerprint": state["review_fingerprint"], "rule_id": expected["rules"][0],
                   "expected_revision": 0, "reviewer": "Synthetic José reviewer",
                   "note": "Earlier fictional follow-up resolved; never transfer this status.",
                   "status": "resolved"}
        metadata.write_text(json.dumps(request), encoding="utf-8")
        result = subprocess.run([str(args.app), "--review-action", str(review_id),
                                 "--metadata", str(metadata), "--output", str(output),
                                 "--data-dir", str(root)], capture_output=True, timeout=120)
        assert result.returncode == 0, result.stderr.decode(errors="replace")[:1200]
        return json.loads(output.read_text(encoding="utf-8-sig"))

    def compare(request, data=root):
        return action({"action": "compare", "request": request}, data)

    def raw_reviews(data=root):
        with sqlite3.connect(data / "private/workspace.sqlite3") as db:
            return db.execute("SELECT id,document_id,created_at,value FROM review_runs ORDER BY id").fetchall()

    pair_results = []
    first_pair = None
    for extension in ["docx", "pdf"]:
        original = args.work / f"Earlier.{extension}"
        revised = args.work / f"Revised.{extension}"
        shutil.copyfile(args.samples / f"Synthetic revision original.{extension}", original)
        shutil.copyfile(args.samples / f"Synthetic revision corrected.{extension}", revised)
        before = check(original)
        assert [f["outcome"] for f in before["review"]["findings"][:5]] == expected["original_outcomes"]
        action({"action": "add_revision", "document_id": before["document_id"], "path": str(revised),
                "expected_group_id": history(before["document_id"])["group_id"]})
        after = check(revised)
        assert [f["outcome"] for f in after["review"]["findings"][:5]] == expected["corrected_outcomes"]
        assert len(before["review"]["findings"]) == len(after["review"]["findings"]) == 20
        mark_resolved(before["id"])
        request = {"left_reading_id": reading_id(before["document_id"], before["id"]),
                   "right_reading_id": reading_id(after["document_id"], after["id"]),
                   "left_review_id": before["id"], "right_review_id": after["id"]}
        comparison = compare(request)
        by_id = {f["rule_id"]: f for f in comparison["findings"]}
        assert [by_id[r]["change"] for r in expected["rules"]] == expected["corrected_changes"]
        assert by_id[expected["rules"][0]]["left_status"] == "resolved"
        assert by_id[expected["rules"][0]]["right_status"] == "unresolved"
        assert comparison["left"]["review"]["exact_reading"]
        assert comparison["right"]["review"]["exact_reading"]
        assert comparison["text"]["added_lines"] > 0 and comparison["text"]["removed_lines"] > 0
        assert comparison["left"]["reading"]["saved"]["sha256"] == sha(original)
        assert comparison["right"]["reading"]["saved"]["sha256"] == sha(revised)
        pair_results.append({"format": extension, "changes": [by_id[r]["change"] for r in expected["rules"]],
                             "left_status": "resolved", "right_status": "unresolved"})
        if first_pair is None:
            first_pair = (original, revised, before, after, request)
    original, revised, before, after, request = first_pair
    public = root / "library/catalog.sqlite3"
    public_before = public_snapshot_hash(public)
    reviews_before = raw_reviews()

    # Moving an original never prevents comparison of its complete retained reading.
    moved = args.work / "Moved earlier.docx"
    original.rename(moved)
    retained = action({"action": "reading", "reading_id": request["left_reading_id"]})
    assert "9 hours" in "\n".join(u["text"] for u in retained["saved"]["report"]["units"])
    assert compare(request)["text"]["available"]

    # Explicit link corrections are reversible and refuse stale histories.
    group = history(before["document_id"])["group_id"]
    action({"action": "unlink", "document_id": after["document_id"], "expected_group_id": group})
    compare_failure = {"action": "compare", "request": request}
    action(compare_failure, success=False)
    new_group = history(after["document_id"])["group_id"]
    action({"action": "link", "request": {"document_id": before["document_id"],
            "revised_document_id": after["document_id"], "expected_group_id": group,
            "expected_revision_group_id": group}}, success=False)
    action({"action": "link", "request": {"document_id": before["document_id"],
            "revised_document_id": after["document_id"], "expected_group_id": group,
            "expected_revision_group_id": new_group}})
    assert raw_reviews() == reviews_before

    # Changing the same file retains the old text and review, not just the latest cache.
    same_path = args.work / "Saved over.docx"
    shutil.copyfile(args.samples / "Synthetic revision original.docx", same_path)
    prior = check(same_path)
    old_reading = reading_id(prior["document_id"], prior["id"])
    shutil.copyfile(args.samples / "Synthetic revision corrected.docx", same_path)
    later = check(same_path)
    assert later["document_id"] == prior["document_id"]
    assert len(history(prior["document_id"])["versions"]) == 2
    current_reading = reading_id(later["document_id"], later["id"])
    overwritten = compare({"left_reading_id": old_reading, "right_reading_id": current_reading,
                          "left_review_id": prior["id"], "right_review_id": later["id"]})
    assert sum(f["change"] == "no_longer_flagged" for f in overwritten["findings"]) == 5

    # Removing an unsupported clause is not a correction; changing scope is separate.
    missing = args.work / "Missing term.docx"
    shutil.copyfile(args.samples / "Synthetic revision missing term.docx", missing)
    action({"action": "add_revision", "document_id": before["document_id"], "path": str(missing),
            "expected_group_id": history(before["document_id"])["group_id"]})
    missing_review = check(missing)
    missing_reading = reading_id(missing_review["document_id"], missing_review["id"])
    missing_request = dict(request, right_reading_id=missing_reading, right_review_id=missing_review["id"])
    missing_result = compare(missing_request)
    row = next(f for f in missing_result["findings"] if f["rule_id"] == expected["missing_term_rule"])
    assert row["right_outcome"] == expected["missing_term_outcome"]
    assert row["change"] == expected["missing_term_change"]
    changed_scope = args.work / "changed-scope.json"
    altered = dict(expected["context"], normal_hours="outside_scope")
    changed_scope.write_text(json.dumps(altered), encoding="utf-8")
    changed_review = check(revised, changed_scope)
    changed_result = compare(dict(request, right_review_id=changed_review["id"]))
    assert next(f for f in changed_result["findings"] if f["rule_id"] == expected["rules"][0])["change"] == "not_comparable"
    action({"action": "compare", "request": dict(request, right_review_id=missing_review["id"])}, success=False)
    action({"action": "history", "document_id": before["document_id"], "upload": True}, success=False)

    pdf = args.work / "Version comparison.pdf"
    exported = args.work / "Version comparison.json"
    action({"action": "export_pdf", "request": request, "destination": str(pdf)})
    action({"action": "export_json", "request": request, "destination": str(exported)})
    pdf_hash, json_hash = sha(pdf), sha(exported)
    action({"action": "export_pdf", "request": request, "destination": str(pdf)}, success=False)
    action({"action": "export_json", "request": request, "destination": str(exported)}, success=False)
    assert sha(pdf) == pdf_hash and sha(exported) == json_hash
    full = json.loads(exported.read_text())
    assert full["left"]["review"]["workflow"]["events"][0]["new_status"] == "resolved"
    assert not full["right"]["review"]["workflow"]["events"]
    assert "9 hours" in "\n".join(u["text"] for u in full["left"]["reading"]["saved"]["report"]["units"])
    pdf_reading = args.work / "comparison-reading.json"
    result = subprocess.run([str(args.app), "--read-file", str(pdf), "--output", str(pdf_reading),
                             "--data-dir", str(args.work / "pdf-reader-data")],
                            capture_output=True, timeout=120)
    assert result.returncode == 0, result.stderr.decode(errors="replace")[:1200]
    parsed = json.loads(pdf_reading.read_text())
    pdf_text = "\n".join(u["text"] for u in parsed["report"]["units"])
    for phrase in ["Numeric issue no longer flagged", "Resolved", "Unresolved",
                   "Earlier fictional follow-up resolved; never transfer this status.",
                   full["left"]["reading"]["saved"]["sha256"], full["right"]["reading"]["saved"]["sha256"]]:
        assert phrase in pdf_text, phrase
    assert public_snapshot_hash(public) == public_before

    # An actual schema-5 fixture retains raw cache/review/history bytes during upgrade.
    # Let the real app establish its owning SID and protected directories first.
    # Python mkdir on an elevated Windows runner can assign Administrators as
    # owner; opening such a manually created root must correctly be refused.
    legacy_root = args.work / "legacy-data"
    action({"action": "history", "document_id": -1}, legacy_root, success=False)
    legacy = legacy_root / "private/workspace.sqlite3"
    assert legacy.is_file(), "The app did not initialize the isolated private workspace"
    with sqlite3.connect(legacy) as empty:
        assert empty.execute("PRAGMA user_version").fetchone()[0] == 6
        assert empty.execute("SELECT COUNT(*) FROM review_runs").fetchone()[0] == 0
    with sqlite3.connect(root / "private/workspace.sqlite3") as source:
        with sqlite3.connect(legacy) as target:
            source.backup(target)
            target.executescript("PRAGMA foreign_keys=OFF; DROP TABLE review_version_links; "
                                "DROP TABLE document_readings; DROP TABLE document_versions; "
                                "DROP TABLE document_link_events; DROP TABLE document_group_members; "
                                "DROP TABLE document_groups; PRAGMA user_version=5;")
    with sqlite3.connect(legacy) as db:
        old_reviews = db.execute("SELECT * FROM review_runs ORDER BY id").fetchall()
        old_actions = db.execute("SELECT * FROM review_actions ORDER BY id").fetchall()
        old_caches = db.execute("SELECT * FROM document_extractions ORDER BY document_id").fetchall()
    action({"action": "history", "document_id": before["document_id"]}, legacy_root)
    with sqlite3.connect(legacy) as db:
        assert db.execute("PRAGMA user_version").fetchone()[0] == 6
        assert db.execute("SELECT * FROM review_runs ORDER BY id").fetchall() == old_reviews
        assert db.execute("SELECT * FROM review_actions ORDER BY id").fetchall() == old_actions
        assert db.execute("SELECT * FROM document_extractions ORDER BY document_id").fetchall() == old_caches
        assert db.execute("SELECT COUNT(*) FROM review_version_links WHERE reading_id IS NOT NULL").fetchone()[0] == 0
    for name, digest in fixtures.items():
        assert sha(args.samples / name) == digest
    assert public_snapshot_hash(public) == public_before
    with sqlite3.connect(root / "private/workspace.sqlite3") as db:
        assert db.execute("PRAGMA user_version").fetchone()[0] == 6
    with sqlite3.connect(public) as db:
        assert db.execute("PRAGMA user_version").fetchone()[0] == 5
    checks = {name: True for name in [
        "installed_revision_linking_passed", "installed_retained_readings_passed",
        "installed_document_diff_passed", "installed_finding_comparison_passed",
        "installed_version_history_passed", "installed_status_separation_passed",
        "installed_comparison_pdf_passed", "installed_comparison_json_passed",
        "installed_revision_guards_passed", "installed_revision_migration_passed",
        "revision_originals_preserved", "revision_public_catalog_preserved",
    ]}
    output = {"private_schema": 6, "public_schema": 5, "synthetic_revision_fixture_count": 5,
              "workflow_cases": [{"name": name, "passed": True} for name in [
                  "revision_link_and_text_diff", "finding_comparison_and_history",
                  "same_path_version_retention", "missing_statement_and_basis_guards",
                  "comparison_exports", "migration_and_private_boundary"]],
              "checks": checks, "pair_results": pair_results}
    with args.output.open("x", encoding="utf-8") as file:
        json.dump(output, file, indent=2)
    print("Installed document revisions passed: six workflows, Word/PDF pairs, retained histories.")


if __name__ == "__main__":
    main()
