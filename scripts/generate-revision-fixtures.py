"""Generate fictional document-version fixtures using only Python's standard library."""
import json
from io import BytesIO
from pathlib import Path
from xml.sax.saxutils import escape
from zipfile import ZipFile, ZipInfo, ZIP_DEFLATED

OUT = Path(__file__).resolve().parents[1] / "examples"
BANNER = "SYNTHETIC TEST ONLY - NOT A LEGAL TEMPLATE"
ORIGINAL = [
    "Normal daily working hours are 9 hours.",
    "Ordinary-day overtime premium is 20% of the regular hourly wage.",
    "Night shift differential is 5% of the regular hourly wage for hours worked between 22:00 and 06:00.",
    "Regular meal period is 50 minutes of time-off.",
    "Weekly rest period is 20 consecutive hours after every six consecutive normal work days.",
]
CORRECTED = [
    "Normal daily working hours are 8 hours.",
    "Ordinary-day overtime premium is 25% of the regular hourly wage.",
    "Night shift differential is 10% of the regular hourly wage for hours worked between 22:00 and 06:00.",
    "Regular meal period is 60 minutes of time-off.",
    "Weekly rest period is 24 consecutive hours after every six consecutive normal work days.",
]


def docx(lines):
    parts = {
        "[Content_Types].xml": '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>',
        "_rels/.rels": '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>',
        "word/document.xml": '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>'
        + "".join("<w:p><w:r><w:t>" + escape(line) + "</w:t></w:r></w:p>"
                  for line in [BANNER, "Fictional worker: José Example. No real person or employer.", *lines])
        + "</w:body></w:document>",
    }
    stream = BytesIO()
    with ZipFile(stream, "w", ZIP_DEFLATED) as archive:
        for name, value in parts.items():
            info = ZipInfo(name, (2026, 1, 1, 0, 0, 0))
            info.compress_type = ZIP_DEFLATED
            archive.writestr(info, value.encode("utf-8"))
    return stream.getvalue()


def pdf(lines):
    text = "BT /F1 10 Tf 24 560 Td 24 TL\n" + "\nT*\n".join(
        "(" + line.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)") + ") Tj"
        for line in [BANNER, "Fictional policy. No real person or employer.", *lines]
    ) + "\nET"
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 842 595] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        f"<< /Length {len(text.encode('ascii'))} >>\nstream\n{text}\nendstream".encode("ascii"),
    ]
    out = bytearray(b"%PDF-1.4\n")
    offsets = [0]
    for i, value in enumerate(objects, 1):
        offsets.append(len(out))
        out.extend(f"{i} 0 obj\n".encode() + value + b"\nendobj\n")
    start = len(out)
    out.extend(f"xref\n0 {len(objects) + 1}\n0000000000 65535 f \n".encode())
    for offset in offsets[1:]:
        out.extend(f"{offset:010d} 00000 n \n".encode())
    out.extend(f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n".encode())
    return bytes(out)


for label, lines in [("original", ORIGINAL), ("corrected", CORRECTED),
                     ("missing term", [line for line in CORRECTED if not line.startswith("Ordinary-day")])]:
    (OUT / f"Synthetic revision {label}.docx").write_bytes(docx(lines))
    if label != "missing term":
        (OUT / f"Synthetic revision {label}.pdf").write_bytes(pdf(lines))

expected = {
    "context": {
        "operative_terms_confirmed": True,
        "normal_hours": "covered", "ordinary_overtime": "covered",
        "night_differential": "covered", "meal_period": "covered", "weekly_rest": "covered",
    },
    "rules": ["PH-LAB-083-01", "PH-LAB-087-01", "PH-LAB-086-01", "PH-LAB-085-01", "PH-LAB-091-01"],
    "original_outcomes": ["potential_issue"] * 5,
    "corrected_outcomes": ["stated_term_meets_minimum"] * 5,
    "corrected_changes": ["no_longer_flagged"] * 5,
    "missing_term_rule": "PH-LAB-087-01",
    "missing_term_outcome": "needs_evidence",
    "missing_term_change": "changed",
    "fixture_count": 5,
}
(OUT / "expected-revisions.json").write_text(json.dumps(expected, indent=2) + "\n", encoding="utf-8")
print("Created five fictional document-revision fixtures.")
