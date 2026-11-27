"""Fictional layout/OCR fixtures. Development-only reportlab, Pillow and pypdf.
No real records, legal templates, online services or downloaded test documents.
"""
from io import BytesIO
from pathlib import Path
from zipfile import ZipFile, ZipInfo, ZIP_DEFLATED
from xml.sax.saxutils import escape
import json
import textwrap
from reportlab.pdfgen import canvas
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.lib.utils import ImageReader
from PIL import Image, ImageDraw, ImageFont
from pypdf import PdfReader, PdfWriter
from pypdf.generic import NameObject, DecodedStreamObject

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'examples'
BANNER='SYNTHETIC TEST ONLY - NOT A LEGAL TEMPLATE'
ISSUES=[
 'Normal daily working hours are 9 hours.',
 'Ordinary-day overtime premium is 20% of the regular hourly wage.',
 'Night shift differential is 5% of the regular hourly wage for hours worked between 22:00 and 06:00.',
 'Regular meal period is 30 minutes of time-off.',
 'Weekly rest period is 12 consecutive hours after every six consecutive normal work days.',
]
def start():
 s=BytesIO();c=canvas.Canvas(s,pagesize=(1000,760),invariant=1,pageCompression=0)
 c.setTitle('ComplyPH fictional reading-quality fixture');c.setAuthor('ComplyPH synthetic tests')
 return s,c
def save(name,s,c):
 c.save();(OUT/name).write_bytes(s.getvalue())

# Two physically separated columns, deliberately painted in the wrong text order.
s,c=start();c.setFont('Helvetica',8)
left=['LEFT-ONE. '+ISSUES[0], 'LEFT-TWO. '+ISSUES[1], 'LEFT-THREE. '+ISSUES[3]]
right=['RIGHT-ONE. '+ISSUES[2], 'RIGHT-TWO. '+ISSUES[4], 'RIGHT-THREE. All values are fictional and require separate scope confirmation.']
for x,paragraphs in [(540,right),(40,left)]:
 y=680
 for p in paragraphs:
  for line in textwrap.wrap(p,80):c.drawString(x,y,line);y-=15
  y-=15
c.setFont('Helvetica-Bold',12);c.drawString(280,730,'FULL-WIDTH-HEADER. '+BANNER)
c.setFont('Helvetica',10);c.drawCentredString(500,60,'FULL-WIDTH-FOOTER. No real agreement is represented.')
c.showPage();c.setFont('Helvetica',12);c.drawString(40,700,BANNER);c.drawString(40,650,'SECOND-PHYSICAL-PAGE. Retain page locations.');c.showPage()
save('Synthetic positioned columns.pdf',s,c)

s,c=start();c.setFont('Helvetica',12);c.drawString(40,720,BANNER)
for y in [640,580,520,460]:c.line(40,y,700,y)
for x in [40,370,700]:c.line(x,460,x,640)
for y,label,value in [(600,'NORMAL-HOURS-CELL','9 hours'),(540,'OVERTIME-CELL','20% additional'),(480,'MEAL-CELL','30 minutes')]:
 c.drawString(55,y,label);c.drawString(385,y,value)
c.showPage();save('Synthetic positioned table.pdf',s,c)

pdfmetrics.registerFont(TTFont('FixtureLato',str(ROOT/'assets/fonts/Lato-Regular.ttf')))
s,c=start();c.setFont('FixtureLato',14)
for i,line in enumerate([BANNER,'José Example · ₱12,345.67 · Café','Normal daily working hours are 8 hours.']):c.drawString(40,700-i*50,line)
c.showPage();save('Synthetic mapped font.pdf',s,c)

font=ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf',32)
def policy(background=255,ink=0):
 image=Image.new('L',(2200,1500),background);d=ImageDraw.Draw(image)
 for i,line in enumerate([BANNER,'Fictional worker: José Example. No real agreement.']+ISSUES):d.text((55,70+i*145),line,font=font,fill=ink)
 return image
def scan_pdf(name,image,blank=False):
 # Keep the scan's pixels proportional, including portrait pages after rotation.
 # Stretching a rotated portrait into a landscape page tests distorted glyphs,
 # rather than the intended cardinal-rotation case.
 s,c=start();width,height=image.width*72/180,image.height*72/180
 c.setPageSize((width,height));c.drawImage(ImageReader(image),0,0,width=width,height=height);c.showPage()
 if blank:c.showPage()
 save(name,s,c)
scan_pdf('Synthetic sideways scan.pdf',policy().rotate(-90,expand=True,fillcolor=255))
scan_pdf('Synthetic faint skew scan.pdf',policy(238,220).rotate(-1.5,expand=True,fillcolor=238))
scan_pdf('Synthetic blank OCR page.pdf',policy(),blank=True)
s,c=start();c.setFont('Helvetica',12);c.drawString(40,735,BANNER);c.drawString(40,705,'SELECTABLE-HEADING. The policy below is an image.')
c.drawImage(ImageReader(policy()),20,20,width=960,height=650);c.showPage();save('Synthetic mixed image text.pdf',s,c)

image=Image.new('L',(2200,1500),255);d=ImageDraw.Draw(image);d.text((55,70),BANNER,font=font,fill=0)
for y in [350,600,850,1100]:d.line((100,y,2050,y),fill=0,width=4)
for x in [100,1000,2050]:d.line((x,350,x,1100),fill=0,width=4)
for y,label,value in [(430,'NORMAL-HOURS-CELL','9 hours'),(680,'OVERTIME-CELL','20% additional'),(930,'MEAL-CELL','30 minutes')]:
 d.text((140,y),label,font=font,fill=0);d.text((1040,y),value,font=font,fill=0)
scan_pdf('Synthetic scanned table.pdf',image)

# A visible Helvetica digit whose selectable Unicode mapping is deliberately bad.
s,c=start();c.setFont('Helvetica',14);c.drawString(40,700,BANNER);c.drawString(40,650,'Normal daily working hours are 8 hours.');c.showPage();c.save()
reader=PdfReader(BytesIO(s.getvalue()));writer=PdfWriter();writer.append_pages_from_reader(reader)
cmap='''/CIDInit /ProcSet findresource begin 12 dict begin begincmap
/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def
/CMapName /FictionalBrokenMap def /CMapType 2 def
1 begincodespacerange <00> <FF> endcodespacerange
95 beginbfchar
'''+ '\n'.join(f'<{i:02X}> <{(0xE000 if i==56 else i):04X}>' for i in range(32,127))+'''
endbfchar endcmap CMapName currentdict /CMap defineresource pop end end'''
mapping=DecodedStreamObject();mapping.set_data(cmap.encode('ascii'));reference=writer._add_object(mapping)
for f in writer.pages[0]['/Resources']['/Font'].values():f.get_object()[NameObject('/ToUnicode')]=reference
with (OUT/'Synthetic unsupported font.pdf').open('wb')as output:writer.write(output)

s,c=start();c.setFont('Helvetica',9);c.drawString(40,720,BANNER)
for x,label in [(40,'FIRST'),(370,'SECOND'),(700,'THIRD')]:
 for row in range(4):c.drawString(x,650-row*30,f'{label}-COLUMN-{row+1}. Fictional layout sample.')
c.showPage();save('Synthetic irregular columns.pdf',s,c)

W='http://schemas.openxmlformats.org/wordprocessingml/2006/main'
R='http://schemas.openxmlformats.org/officeDocument/2006/relationships'
NS=f'xmlns:w="{W}" xmlns:r="{R}" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:o="urn:schemas-microsoft-com:office:office"'
def p(text):return '<w:p><w:r><w:t xml:space="preserve">'+escape(text)+'</w:t></w:r></w:p>'
def box(text):return '<w:txbxContent>'+p(text)+'</w:txbxContent>'
def alternatives(choice,fallback):return '<mc:AlternateContent><mc:Choice Requires="wps"><wps:txbx>'+box(choice)+'</wps:txbx></mc:Choice><mc:Fallback><v:shape id="FictionalBox" style="width:400pt;height:80pt"><v:textbox>'+box(fallback)+'</v:textbox></v:shape></mc:Fallback></mc:AlternateContent>'
def docx(name,body,extras=None):
 parts={'[Content_Types].xml':'<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>',
 '_rels/.rels':f'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="document" Type="{R}/officeDocument" Target="word/document.xml"/></Relationships>',
 'word/document.xml':f'<w:document {NS}><w:body>'+p(BANNER)+body+'</w:body></w:document>'}
 parts.update(extras or {})
 with ZipFile(OUT/name,'w',ZIP_DEFLATED)as z:
  for filename,text in parts.items():
   info=ZipInfo(filename,date_time=(2026,1,1,0,0,0));info.compress_type=ZIP_DEFLATED;z.writestr(info,text.encode('utf-8'))
body='<w:p><w:r><w:t xml:space="preserve">BODY-BEFORE </w:t></w:r>'+alternatives(ISSUES[0],ISSUES[0])+'<w:r><w:t> BODY-AFTER</w:t></w:r></w:p>'
body+=''.join(p(line)for line in ISSUES[1:])
body+='<w:p><w:r><w:pict><v:shape id="TableBox"><v:textbox><w:txbxContent><w:tbl><w:tr><w:tc>'+p('BOX-TABLE-PASSAGE')+'</w:tc></w:tr></w:tbl></w:txbxContent></v:textbox></v:shape></w:pict></w:r></w:p>'
docx('Synthetic Word text boxes.docx',body,{'word/header1.xml':f'<w:hdr {NS}><w:p><w:r><w:pict><v:shape><v:textbox>'+box('HEADER-TEXT-BOX-PASSAGE')+'</v:textbox></v:shape></w:pict></w:r></w:p></w:hdr>'})
body=p('Fictional embedded-content policy. No real employee.')
body+='<w:p>'+alternatives(ISSUES[1],ISSUES[1].replace('20%','25%'))+'</w:p>'
body+='<w:p><w:del><w:r><w:delText>DELETED-DO-NOT-READ</w:delText></w:r></w:del><w:ins><w:r><w:t>INSERTED-VISIBLE-TEXT.</w:t></w:r></w:ins></w:p>'
body+='<w:p><w:r><w:fldChar w:fldCharType="begin"/><w:instrText>FIELD-INSTRUCTION-DO-NOT-RUN</w:instrText><w:fldChar w:fldCharType="separate"/><w:t>DISPLAYED-FIELD-RESULT.</w:t><w:fldChar w:fldCharType="end"/></w:r></w:p>'
body+='<w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr>'+p('MERGED-CELL-TEXT')+'</w:tc></w:tr></w:tbl>'
body+='<w:p><w:r><w:object><o:OLEObject r:id="embedded"/></w:object></w:r></w:p><w:altChunk r:id="external"/>'
docx('Synthetic Word embedded content.docx',body,{'word/embeddings/Fictional.bin':'EMBEDDED-PAYLOAD-DO-NOT-OPEN','word/_rels/document.xml.rels':f'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="external" Type="{R}/aFChunk" Target="https://example.invalid/never-fetch-private-document-content" TargetMode="External"/><Relationship Id="embedded" Type="{R}/oleObject" Target="embeddings/Fictional.bin"/></Relationships>'})

reader_cases={
 'Synthetic positioned columns.pdf':{'pages':2,'contains':['LEFT-ONE','LEFT-TWO','LEFT-THREE','RIGHT-ONE','RIGHT-TWO','SECOND-PHYSICAL-PAGE'],'manual':False,'ordered':['FULL-WIDTH-HEADER','LEFT-ONE','LEFT-TWO','LEFT-THREE','RIGHT-ONE','RIGHT-TWO','RIGHT-THREE','FULL-WIDTH-FOOTER']},
 'Synthetic positioned table.pdf':{'pages':1,'contains':['NORMAL-HOURS-CELL','9 hours','OVERTIME-CELL','20% additional'],'manual':True,'warning':'table or column'},
 'Synthetic mapped font.pdf':{'pages':1,'contains':['José Example','₱12,345.67','Café','8 hours'],'manual':False},
 'Synthetic mixed image text.pdf':{'pages':1,'contains':['SELECTABLE-HEADING'],'manual':True,'warning':'substantial image'},
 'Synthetic unsupported font.pdf':{'pages':1,'contains':['Normal daily working hours'],'manual':True,'warning':'font characters'},
 'Synthetic irregular columns.pdf':{'pages':1,'contains':['FIRST-COLUMN-4','SECOND-COLUMN-4','THIRD-COLUMN-4'],'manual':True,'warning':'table or column'},
 'Synthetic Word text boxes.docx':{'contains':['BODY-BEFORE','BODY-AFTER','BOX-TABLE-PASSAGE','HEADER-TEXT-BOX-PASSAGE'],'manual':False,'text_box':True},
 'Synthetic Word embedded content.docx':{'contains':['INSERTED-VISIBLE-TEXT','DISPLAYED-FIELD-RESULT','MERGED-CELL-TEXT'],'excludes':['FIELD-INSTRUCTION-DO-NOT-RUN','DELETED-DO-NOT-READ','EMBEDDED-PAYLOAD-DO-NOT-OPEN','https://example.invalid'],'manual':True,'warning':'Alternative Word representations'},
}
ocr_cases={
 'Synthetic sideways scan.pdf':{'options':{'rotation':'clockwise_270','layout':'auto','enhance_scan':True},'contains':['Normal daily working hours are 9 hours','Regular meal period is 30 minutes','12 consecutive hours'],'manual':False,'pages':1},
 'Synthetic faint skew scan.pdf':{'options':{'rotation':'none','layout':'auto','enhance_scan':True},'contains':['Normal daily working hours are 9 hours','Regular meal period is 30 minutes','12 consecutive hours'],'manual':False,'pages':1,'enhanced':True},
 'Synthetic scanned table.pdf':{'options':{'rotation':'none','layout':'sparse_text','enhance_scan':True},'contains':['9 hours','20% additional','30 minutes'],'manual':True,'pages':1,'warning':'table-like lines'},
 'Synthetic blank OCR page.pdf':{'options':{'rotation':'none','layout':'auto','enhance_scan':True},'contains':['Normal daily working hours are 9 hours'],'manual':True,'pages':2,'warning':'no OCR text'},
 'Synthetic mixed image text.pdf':{'options':{'rotation':'none','layout':'auto','enhance_scan':True},'contains':['SELECTABLE-HEADING','Normal daily working hours are 9 hours','12 consecutive hours'],'manual':False,'pages':1},
 'Synthetic unsupported font.pdf':{'options':{'rotation':'none','layout':'auto','enhance_scan':True},'contains':['Normal daily working hours are 8 hours'],'manual':False,'pages':1},
}
for name in ['Synthetic sideways scan.pdf','Synthetic faint skew scan.pdf','Synthetic mixed image text.pdf']:
 ocr_cases[name]['contains'].append(ISSUES[2])
# Independent expected outcomes for the existing first five labour rules.
for cases in [reader_cases,ocr_cases]:
 for name,case in cases.items():
  case['outcomes']=(['needs_review']*5 if case['manual'] else
       ['stated_term_meets_minimum']+['needs_evidence']*4 if name in ['Synthetic mapped font.pdf','Synthetic unsupported font.pdf'] else
       ['potential_issue']*5)
manifest={'fixture_count':12,'reader_cases':reader_cases,'ocr_cases':ocr_cases,'limitation':'Fictional fixtures and a bounded supported-layout milestone; no promise of arbitrary layout, font or handwriting accuracy.'}
(OUT/'expected-reading-quality.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
assert len(set(reader_cases)|set(ocr_cases))==manifest['fixture_count']
print('Generated twelve fictional reading-quality fixtures.')
