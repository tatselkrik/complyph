"""Generate clearly fictional reader fixtures. Development only; not an app dependency.
Requires reportlab, Pillow, pypdf. No legal templates or real people/identifiers.
"""
from pathlib import Path
from io import BytesIO
from zipfile import ZipFile, ZIP_DEFLATED, ZipInfo
from xml.sax.saxutils import escape
import json
from reportlab.pdfgen import canvas
from reportlab.lib.utils import ImageReader
from PIL import Image, ImageDraw
from pypdf import PdfReader, PdfWriter

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'examples'
BANNER = 'SYNTHETIC TEST ONLY - NOT A LEGAL TEMPLATE'

def pdf(pages):
    stream = BytesIO()
    c = canvas.Canvas(stream, pagesize=(612, 792), invariant=1, pageCompression=0)
    c.setTitle('ComplyPH synthetic reader fixture')
    c.setAuthor('ComplyPH synthetic tests')
    for lines in pages:
        c.setFont('Helvetica-Bold', 11)
        c.drawString(42, 750, BANNER)
        c.setFont('Helvetica', 11)
        for n, line in enumerate(lines):
            c.drawString(42, 710 - n * 24, line)
        c.showPage()
    c.save()
    return stream.getvalue()

text_pdf = pdf([
    ['Fictional employment document - Sample Company Alpha',
     'Fictional worker: Alex Example. No real person or employer is represented.',
     'Fixture ID: SYN-PDF-001. Page one contains the introduction.',
     'This sample tests text reading only. It makes no compliance claims.'],
    ['Working arrangements - synthetic section',
     'Sample schedule: Monday to Friday; invented figures for extraction tests.',
     'Monthly amount: PHP 12345.67. This is not wage advice.',
     'Fixture marker: PAGE-TWO-TERMS.'],
    ['Acknowledgment - synthetic section',
     'Unsigned. No real agreement is created.',
     'Fixture marker: PAGE-THREE-END.',
     'All legal conclusions must remain unavailable in this reader milestone.'],
])
(OUT / 'Synthetic employment.pdf').write_bytes(text_pdf)
image = Image.new('RGB', (1000, 600), '#fffdf7')
draw = ImageDraw.Draw(image)
draw.text((40, 60), BANNER, fill='#142b49', font_size=28)
draw.text((40, 160), 'IMAGE-ONLY-FIXTURE\nThis text is pixels, not PDF text.\nExpected result: Needs OCR.', fill='#142b49', font_size=26)
def image_pdf(mixed=False):
    stream=BytesIO(); c=canvas.Canvas(stream, pagesize=(612,792), invariant=1, pageCompression=0)
    if mixed:
        c.setFont('Helvetica',11)
        c.drawString(42,750,BANNER)
        c.drawString(42,710,'MIXED-TEXT-PAGE: page two is an unread image.')
        c.showPage()
    c.drawImage(ImageReader(image), 30, 300, width=552, height=331)
    c.showPage();c.save()
    return stream.getvalue()
(OUT / 'Synthetic scan.pdf').write_bytes(image_pdf())
(OUT / 'Synthetic mixed pages.pdf').write_bytes(image_pdf(True))
writer = PdfWriter()
writer.append_pages_from_reader(PdfReader(BytesIO(text_pdf)))
writer.add_metadata({'/Title': BANNER})
writer.encrypt('synthetic-password-not-for-real-documents', algorithm='RC4-128')
with (OUT / 'Synthetic protected.pdf').open('wb') as f: writer.write(f)
(OUT / 'Synthetic damaged.pdf').write_bytes(b'%PDF-1.7\nSYNTHETIC DAMAGED FIXTURE. No private content.\n%%EOF\n')

W='http://schemas.openxmlformats.org/wordprocessingml/2006/main'
R='http://schemas.openxmlformats.org/officeDocument/2006/relationships'
def p(text): return '<w:p><w:r><w:t xml:space="preserve">'+escape(text)+'</w:t></w:r></w:p>'
body = p(BANNER) + p('Fictional workplace policy — Sample Company Beta')
body += '<w:p><w:r><w:t>Unicode: José Example · ₱12,345.67 · </w:t></w:r><w:r><w:t>kaligtasan &amp; privacy.</w:t></w:r></w:p>'
body += '<w:p><w:r><w:t>Tab</w:t><w:tab/><w:t>separated</w:t><w:br/><w:t>Next line.</w:t></w:r></w:p>'
body += '<w:tbl><w:tr><w:tc>'+p('TOPIC-CELL')+'</w:tc><w:tc>'+p('DETAIL-CELL')+'</w:tc></w:tr><w:tr><w:tc>'+p('Safety - invented scenario')+'</w:tc><w:tc>'+p('No regulatory conclusion is supplied.')+'</w:tc></w:tr></w:tbl>'
body += '<w:p><w:hyperlink r:id="rIdExternal"><w:r><w:t>External link label is read; target is never fetched.</w:t></w:r></w:hyperlink></w:p>'
body += '<w:p><w:r><w:instrText>PRIVATE-FIELD-INSTRUCTION-DO-NOT-READ</w:instrText></w:r><w:r><w:t>Displayed field value.</w:t></w:r><w:del><w:r><w:delText>DELETED-TEXT-DO-NOT-READ</w:delText></w:r></w:del><w:ins><w:r><w:t> Inserted revision.</w:t></w:r></w:ins></w:p>'
body += p('Fixture marker: WORD-POLICY-END. This is not a legal template.')
body += '<w:sectPr><w:headerReference w:type="default" r:id="rIdHeader"/><w:footerReference w:type="default" r:id="rIdFooter"/></w:sectPr>'
parts = {
 '[Content_Types].xml': '<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>',
 '_rels/.rels': '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDocument" Type="'+R+'/officeDocument" Target="word/document.xml"/></Relationships>',
 'word/document.xml': '<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="'+W+'" xmlns:r="'+R+'"><w:body>'+body+'</w:body></w:document>',
 'word/header1.xml': '<w:hdr xmlns:w="'+W+'">'+p('SYNTHETIC-HEADER - not a company policy')+'</w:hdr>',
 'word/footer1.xml': '<w:ftr xmlns:w="'+W+'">'+p('SYNTHETIC-FOOTER - no real personal information')+'</w:ftr>',
 'word/_rels/document.xml.rels': '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdExternal" Type="'+R+'/hyperlink" Target="https://example.invalid/never-fetch-this-document-link" TargetMode="External"/><Relationship Id="rIdHeader" Type="'+R+'/header" Target="header1.xml"/><Relationship Id="rIdFooter" Type="'+R+'/footer" Target="footer1.xml"/></Relationships>',
}
with ZipFile(OUT / 'Synthetic workplace policy.docx', 'w', ZIP_DEFLATED) as z:
    for name, content in parts.items():
        info=ZipInfo(name, date_time=(2026,1,1,0,0,0)); info.compress_type=ZIP_DEFLATED
        z.writestr(info, content.encode('utf-8'))
(OUT / 'Synthetic damaged.docx').write_bytes(b'SYNTHETIC DAMAGED DOCX FIXTURE - not a ZIP file.\n')
expected = {
 'Synthetic employment.pdf': {'status':'ready','units':3,'contains':['PAGE-TWO-TERMS','PAGE-THREE-END','PHP 12345.67']},
 'Synthetic workplace policy.docx': {'status':'ready','contains':['José Example','₱12,345.67','kaligtasan & privacy.','Tab\tseparated\nNext line.','TOPIC-CELL','DETAIL-CELL','SYNTHETIC-HEADER','SYNTHETIC-FOOTER','Inserted revision.'],'excludes':['PRIVATE-FIELD-INSTRUCTION-DO-NOT-READ','DELETED-TEXT-DO-NOT-READ','https://example.invalid']},
 'Synthetic scan.pdf': {'status':'needs_ocr','units':1,'excludes':['IMAGE-ONLY-FIXTURE']},
 'Synthetic mixed pages.pdf': {'status':'ready','units':2,'contains':['MIXED-TEXT-PAGE'],'excludes':['IMAGE-ONLY-FIXTURE']},
 'Synthetic protected.pdf': {'status':'password_protected'},
 'Synthetic damaged.pdf': {'status':'unreadable'},
 'Synthetic damaged.docx': {'status':'unreadable'},
}
(OUT / 'expected-results.json').write_text(json.dumps(expected,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print('Created seven synthetic reader fixtures and expected results in',OUT)

# Focused rule fixtures: deliberately fictional, with independently stated outcomes.
baseline=['Normal daily working hours are 8 hours.',
 'Ordinary-day overtime premium is 25% of the regular hourly wage.',
 'Night shift differential is 10% of the regular hourly wage for hours worked between 22:00 and 06:00.',
 'Regular meal period is 60 minutes of time-off.',
 'Weekly rest period is 24 consecutive hours after every six consecutive normal work days.']
# Keep every complete clause on one PDF line within the page width.
stream=BytesIO(); c=canvas.Canvas(stream,pagesize=(842,595),invariant=1,pageCompression=0)
c.setFont('Helvetica',10)
for n,line in enumerate([BANNER,'Fictional rule fixture; coverage is confirmed separately.']+baseline): c.drawString(30,550-n*24,line)
c.showPage();c.save()
(OUT/'Synthetic labour baseline.pdf').write_bytes(stream.getvalue())
issues=[baseline[0].replace('8 hours','9 hours'),baseline[1].replace('25%','20%'),baseline[2].replace('10%','5%'),baseline[3].replace('60 minutes','30 minutes'),baseline[4].replace('24 consecutive hours','12 consecutive hours')]
ambiguous=['Normal daily working hours are 9 hours including unpaid lunch.',
 'Overtime is paid at 125%.','Night shift differential is 10% of the regular hourly wage.',
 'Regular meal period is 20 minutes paid as working time.',
 'Weekly rest period is 24 total hours in two separate breaks.']
for name,lines in [('Synthetic labour issues.docx',issues),('Synthetic labour ambiguous.docx',ambiguous)]:
 with ZipFile(OUT/name,'w',ZIP_DEFLATED) as z:
  for part in ['[Content_Types].xml','_rels/.rels']:
   info=ZipInfo(part,date_time=(2026,1,1,0,0,0));info.compress_type=ZIP_DEFLATED;z.writestr(info,parts[part])
  xml='<w:document xmlns:w="'+W+'"><w:body>'+p(BANNER)+p('Fictional fixture; no real agreement.')+''.join(p(line) for line in lines)+'</w:body></w:document>'
  info=ZipInfo('word/document.xml',date_time=(2026,1,1,0,0,0));info.compress_type=ZIP_DEFLATED;z.writestr(info,xml)
(OUT/'expected-checks.json').write_text(json.dumps({
 'context':{'operative_terms_confirmed':True,'normal_hours':'covered','ordinary_overtime':'covered','night_differential':'covered','meal_period':'covered','weekly_rest':'covered'},
 'expected':{'Synthetic labour baseline.pdf':['stated_term_meets_minimum']*5+['needs_context'],'Synthetic labour issues.docx':['potential_issue']*5+['needs_context'],'Synthetic labour ambiguous.docx':['needs_review']*5+['needs_context']},
 'limitation':'Synthetic fixtures only. Reviewer-confirmed coverage, net hours and operative statements; no legal certification.'},indent=2)+'\n')
print('Created three synthetic labour-check fixtures.')

# Separate raster-only and mixed-page cases for offline OCR validation.
from PIL import ImageFont
ocr_image=Image.new('RGB',(2200,1500),'white')
ocr_draw=ImageDraw.Draw(ocr_image)
ocr_font=ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf',30)
for n,line in enumerate([BANNER,'Fictional OCR policy. Applicability is confirmed separately.']+issues):
    ocr_draw.text((50,70+n*140),line,font=ocr_font,fill='black')
stream=BytesIO();c=canvas.Canvas(stream,pagesize=(792,540),invariant=1,pageCompression=0)
c.drawImage(ImageReader(ocr_image),0,0,width=792,height=540);c.showPage()
c.setFont('Helvetica',15);c.drawString(35,490,'SYNTHETIC SECOND PAGE - NO REAL AGREEMENT');c.showPage();c.save()
(OUT/'Synthetic OCR policy.pdf').write_bytes(stream.getvalue())
(OUT/'expected-ocr.json').write_text(json.dumps({
 'Synthetic scan.pdf':{'pages':1,'contains':['IMAGE-ONLY-FIXTURE','pixels']},
 'Synthetic mixed pages.pdf':{'pages':2,'contains':['MIXED-TEXT-PAGE','IMAGE-ONLY-FIXTURE']},
 'Synthetic OCR policy.pdf':{'pages':2,'contains':['Normal daily working hours are 9 hours','Regular meal period is 30 minutes','12 consecutive hours','SECOND PAGE']},
},indent=2)+'\n')
