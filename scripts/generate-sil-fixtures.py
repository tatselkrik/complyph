"""Generate fictional SIL fixtures only. Development dependency: ReportLab."""
from io import BytesIO
import json
from pathlib import Path
from xml.sax.saxutils import escape
from zipfile import ZipFile, ZipInfo, ZIP_DEFLATED

from reportlab.pdfgen import canvas

OUT = Path(__file__).resolve().parents[1] / 'examples'
BANNER = 'SYNTHETIC TEST ONLY - NOT A LEGAL TEMPLATE.'
W = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'
SCOPE = {
    'employee_coverage': 'covered', 'qualifying_service': 'covered',
    'exceptions': 'covered',
    'basis': 'Fictional 2026 review: covered non-managerial supervised employee; '
             '12 qualifying months including authorized absences confirmed; '
             'establishment regularly employs 10 workers; no existing equivalent '
             'paid benefit or applicable exemption established. Synthetic facts only.'
}
cases = [
    ('Synthetic SIL baseline.pdf', 'Paid service incentive leave is 5 days per year.',
     'stated_term_meets_minimum', {}),
    ('Synthetic SIL issues.docx', 'Annual service incentive leave is four days with pay.',
     'potential_issue', {}),
    ('Synthetic SIL ambiguous.docx', 'Service incentive leave is 5 days per year, subject to approval.',
     'needs_review', {}),
    ('Synthetic SIL excluded.docx', 'Paid service incentive leave is 0 days per year.',
     'outside_scope', {'exceptions': 'outside_scope',
                      'basis': 'Fictional 2026 review: establishment regularly employs nine workers. '
                               'Small-establishment exception confirmed for this standard comparison; '
                               'contractual or other leave rights require separate review.'}),
    ('Synthetic SIL unknown service.docx', 'Paid service incentive leave is 0 days per year.',
     'needs_context', {'qualifying_service': 'unknown',
                       'basis': 'Fictional 2026 review: covered employment and exceptions reviewed; '
                                'qualifying service records have not been provided.'}),
]

def docx(statement):
    paragraphs = [BANNER, 'Fictional review fixture. No real agreement or person.', statement]
    document = '<w:document xmlns:w="' + W + '"><w:body>' + ''.join(
        '<w:p><w:r><w:t>' + escape(text) + '</w:t></w:r></w:p>'
        for text in paragraphs) + '</w:body></w:document>'
    parts = {
        '[Content_Types].xml': '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>',
        '_rels/.rels': '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>',
        'word/document.xml': document,
    }
    stream = BytesIO()
    with ZipFile(stream, 'w', ZIP_DEFLATED) as archive:
        for name, content in parts.items():
            info = ZipInfo(name, (2026, 1, 1, 0, 0, 0))
            info.compress_type = ZIP_DEFLATED
            archive.writestr(info, content.encode('utf-8'))
    return stream.getvalue()

expected = {}
for name, statement, outcome, overrides in cases:
    if name.endswith('.pdf'):
        stream = BytesIO()
        page = canvas.Canvas(stream, pagesize=(842, 595), invariant=1, pageCompression=0)
        page.setTitle('ComplyPH fictional SIL boundary fixture')
        page.setAuthor('ComplyPH synthetic tests')
        page.setFont('Helvetica', 11)
        for index, line in enumerate([BANNER, 'Fictional fixture. Eligibility is confirmed separately.', statement]):
            page.drawString(30, 550-index*30, line)
        page.showPage()
        page.save()
        data = stream.getvalue()
    else:
        data = docx(statement)
    (OUT / name).write_bytes(data)
    expected[name] = {
        'context': {'operative_terms_confirmed': True,
                    'normal_hours': 'unknown', 'ordinary_overtime': 'unknown',
                    'night_differential': 'unknown',
                    'service_incentive_leave': dict(SCOPE, **overrides)},
        'outcome': outcome,
    }
(OUT / 'expected-sil.json').write_text(json.dumps(expected, indent=2) + '\n', encoding='utf-8')
print('Created five fictional SIL fixtures and independent expectations.')
