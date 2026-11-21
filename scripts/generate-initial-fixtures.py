from pathlib import Path
import json
from io import BytesIO
from zipfile import ZipFile,ZIP_DEFLATED,ZipInfo
from xml.sax.saxutils import escape
from reportlab.pdfgen import canvas
R=Path(__file__).resolve().parents[1];O=R/'examples'
# Independent legal-threshold oracle, rather than deriving expected values from the engine.
rows=[
('PH-LAB-296-01','Probationary employment period is {} months.',6,'maximum'),
('PH-OSH-016-01','Mandatory worker safety and health seminar duration is {} hours.',8,'minimum'),
('PH-OSH-008-01','Worker charge for required PPE is PHP {}.',0,'maximum'),
('PH-NPC-038-01','Required breach notifications to the NPC and affected data subjects are sent within {} hours of knowledge or reasonable belief.',72,'maximum'),
('PH-BIR-024-01','Annual taxable compensation income zero-rate ceiling is PHP {}.',250000,'exact'),
('PH-SSS-TOTAL-01','SSS total contribution rate is {}% of monthly salary credit.',15,'exact'),
('PH-SSS-EMPLOYER-01','SSS employer contribution rate is {}% of monthly salary credit.',10,'exact'),
('PH-SSS-EMPLOYEE-01','SSS employee contribution rate is {}% of monthly salary credit.',5,'exact'),
('PH-PHIC-RATE-01','PhilHealth premium rate is {}% of monthly basic salary.',5,'exact'),
('PH-PHIC-FLOOR-01','PhilHealth income floor is PHP {}.',10000,'exact'),
('PH-PHIC-CEILING-01','PhilHealth income ceiling is PHP {}.',100000,'exact'),
('PH-HDMF-EE-HIGH-01','Pag-IBIG employee mandatory savings rate is {}% of applicable fund salary.',2,'exact'),
('PH-HDMF-EE-LOW-01','Pag-IBIG employee mandatory savings rate is {}% of applicable fund salary.',1,'exact'),
('PH-HDMF-ER-01','Pag-IBIG employer mandatory savings rate is {}% of applicable fund salary.',2,'exact'),
]
base=json.loads((O/'expected-checks.json').read_text())['context'];base['service_incentive_leave']=json.loads((O/'expected-sil.json').read_text())['Synthetic SIL issues.docx']['context']['service_incentive_leave']
base['additional_checks']={id:dict(coverage='covered',exceptions_reviewed=True,review_on='2026-11-01',basis='Fictional 2026 worker and policy review. Applicable private employment, standard regime, period/amendments and exceptions reviewed; source-specific employee category and base confirmed. SSS business employee MSC excludes EC; PhilHealth basic salary and category confirmed; Pag-IBIG compensation exceeds PHP 1500 and current fund-salary cap independently reviewed. No special scheme or voluntary top-up. Synthetic assertions only.') for id,*_ in rows}
base['additional_checks']['PH-HDMF-EE-LOW-01']['coverage']='outside_scope'
legacy=['Normal daily working hours are 8 hours.','Ordinary-day overtime premium is 25% of the regular hourly wage.','Night shift differential is 10% of the regular hourly wage for hours worked between 22:00 and 06:00.','Regular meal period is 60 minutes of time-off.','Weekly rest period is 24 consecutive hours after every six consecutive normal work days.','Paid service incentive leave is 5 days per year.']
B='SYNTHETIC TEST ONLY - NOT A LEGAL TEMPLATE.'
def docx(lines):
 stream=BytesIO();W='http://schemas.openxmlformats.org/wordprocessingml/2006/main'
 body=''.join('<w:p><w:r><w:t>'+escape(t)+'</w:t></w:r></w:p>' for t in [B,'Fictional seven-domain policy. Eligibility is confirmed separately.']+lines)
 parts={'[Content_Types].xml':'<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>','_rels/.rels':'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>','word/document.xml':'<w:document xmlns:w="'+W+'"><w:body>'+body+'</w:body></w:document>'}
 with ZipFile(stream,'w',ZIP_DEFLATED) as z:
  for name,text in parts.items():i=ZipInfo(name,(2026,1,1,0,0,0));i.compress_type=ZIP_DEFLATED;z.writestr(i,text.encode())
 return stream.getvalue()
# One worker/period cannot be in both employee compensation bands.
active=[r for r in rows if r[0]!='PH-HDMF-EE-LOW-01']
matched=legacy+[fmt.format(value) for id,fmt,value,kind in active]
issues=legacy+[fmt.format(value+1 if kind in ('maximum','exact') else value-1) for id,fmt,value,kind in active]
ambiguous=legacy+[fmt.format(value).rstrip('.')+'; unless management approves otherwise.' for id,fmt,value,kind in active]
expected={}
for name,lines,status in [('Synthetic seven-domain baseline.docx',matched,'stated_term_meets_minimum'),('Synthetic seven-domain issues.docx',issues,'potential_issue'),('Synthetic seven-domain ambiguous.docx',ambiguous,'needs_review')]:
 (O/name).write_bytes(docx(lines));outcomes=['stated_term_meets_minimum']*6+[('outside_scope' if id=='PH-HDMF-EE-LOW-01' else status) for id,*_ in rows]
 expected[name]={'context':base,'outcomes':outcomes}
stream=BytesIO();page=canvas.Canvas(stream,pagesize=(842,595),invariant=1,pageCompression=0);page.setTitle('ComplyPH synthetic seven-domain profile');page.setFont('Helvetica',9)
for start in range(0,len(matched),9):
 page.setFont('Helvetica',9)
 page.drawString(24,570,B)
 for i,line in enumerate(matched[start:start+9]):page.drawString(24,535-i*44,line)
 page.showPage()
page.save();(O/'Synthetic seven-domain baseline.pdf').write_bytes(stream.getvalue());expected['Synthetic seven-domain baseline.pdf']={'context':base,'outcomes':expected['Synthetic seven-domain baseline.docx']['outcomes']}
low_name='Synthetic Pag-IBIG low-income.docx'
low_lines=['Pag-IBIG employee mandatory savings rate is 1% of applicable fund salary.','Pag-IBIG employer mandatory savings rate is 2% of applicable fund salary.']
(O/low_name).write_bytes(docx(low_lines))
low_context={'operative_terms_confirmed':True,'normal_hours':'unknown','ordinary_overtime':'unknown','night_differential':'unknown','additional_checks':{}}
for id in ['PH-HDMF-EE-HIGH-01','PH-HDMF-EE-LOW-01','PH-HDMF-ER-01']:
 low_context['additional_checks'][id]={'coverage':'outside_scope' if 'HIGH' in id else 'covered','exceptions_reviewed':True,'review_on':'2026-11-01','basis':'Fictional covered mandatory member earning exactly PHP 1500 per month; correct fund salary and period, amendments and exceptions reviewed. No voluntary extras or special regime.'}
low_outcomes=['needs_context']*20
low_outcomes[17]='outside_scope';low_outcomes[18]='stated_term_meets_minimum';low_outcomes[19]='stated_term_meets_minimum'
expected[low_name]={'context':low_context,'outcomes':low_outcomes}
(O/'expected-initial.json').write_text(json.dumps({'cases':expected,'boundaries':[dict(id=id,statement=fmt,threshold=value,comparison=kind) for id,fmt,value,kind in rows]},indent=2)+'\n')
print('Created five cross-domain fixtures and independent boundaries.')
