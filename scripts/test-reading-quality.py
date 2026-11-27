"""Actual-app reading-quality acceptance: fictional files, new local workspace, no HTTP."""
import argparse
from contextlib import closing
import hashlib
import json
from pathlib import Path
import sqlite3
import subprocess


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['app','samples','work','output']:parser.add_argument('--'+name,type=Path,required=True)
    args=parser.parse_args();args.app=args.app.resolve();args.samples=args.samples.resolve()
    args.work.mkdir(parents=True,exist_ok=False)
    root=args.work/'data';sequence=0
    expected=json.loads((args.samples/'expected-reading-quality.json').read_text(encoding='utf-8'))
    files=set(expected['reader_cases'])|set(expected['ocr_cases'])
    assert len(files)==expected['fixture_count']==12
    original_hashes={name:sha(args.samples/name)for name in files}
    context=json.loads((args.samples/'expected-checks.json').read_text())['context']
    context['ocr_text_confirmed']=True
    scope=args.work/'scope.json';scope.write_text(json.dumps(context),encoding='utf-8')

    def invoke(mode,arguments=(),data=root,success=True,pdf=False):
        nonlocal sequence
        sequence+=1;output=args.work/f'{sequence}-result.{"pdf" if pdf else "json"}'
        catalog=data/'library/catalog.sqlite3'
        before=sha(catalog)if catalog.exists()else None
        command=[str(args.app),mode,*map(str,arguments),'--output',str(output),'--data-dir',str(data)]
        run=subprocess.run(command,capture_output=True,timeout=150)
        assert (run.returncode==0)==success,(mode,run.returncode,run.stderr.decode(errors='replace')[:1500])
        if mode!='--check-file':assert (sha(catalog)if catalog.exists()else None)==before,'Private reading/export changed the public catalog'
        if not success:return None
        return output if pdf else json.loads(output.read_text(encoding='utf-8-sig'))

    def query(statement,parameters=()):
        with closing(sqlite3.connect((root/'private/workspace.sqlite3').resolve().as_uri()+'?mode=ro',uri=True))as db:return db.execute(statement,parameters).fetchall()

    def action(request):
        nonlocal sequence
        sequence+=1;metadata=args.work/f'{sequence}-request.json';metadata.write_text(json.dumps(request),encoding='utf-8')
        return invoke('--document-action',['--metadata',metadata])

    def review(path,case):
        saved=invoke('--check-file',[path,'--context',scope])
        assert len(saved['review']['findings'])==20
        assert [f['outcome']for f in saved['review']['findings'][:5]]==case['outcomes'],(path.name,saved['review']['findings'][:5],case['outcomes'])
        return saved

    def inspect(saved,name,case):
        report=saved['report'];assert report['status']=='ready',(name,report['summary'])
        assert saved['sha256']==original_hashes[name]
        text='\n'.join(u['text']for u in report['units']);normalized=' '.join(text.split())
        for phrase in case['contains']:assert phrase in normalized,(name,phrase,normalized)
        for phrase in case.get('excludes',[]):assert phrase not in text,(name,phrase)
        if 'warning'in case:assert any(case['warning'].lower()in w.lower()for w in report['warnings']),(name,report['warnings'])
        if 'pages'in case:
            assert len(report['units'])==case['pages']
            assert [u['location']for u in report['units']]==[{'type':'pdf_page','page':i+1}for i in range(case['pages'])]
        if 'ordered'in case:
            locations=[text.index(marker)for marker in case['ordered']]
            assert locations==sorted(locations),(name,text)
        return report

    reader_results=[];ocr_results=[];reader_reviews={};ocr_reviews={};states={};public_before=None
    for name,case in expected['reader_cases'].items():
        path=args.samples/name;saved=invoke('--read-file',[path]);report=inspect(saved,name,case)
        assert report['engine']=='complyph-text-2'
        assessment=review(path,case);reader_reviews[name]=assessment
        state=invoke('--review-state',[assessment['id']]);states[assessment['id']]=state
        assert state['reading_context']['warnings']==report['warnings']
        if name.endswith('.docx')and case.get('text_box'):
            finding=assessment['review']['findings'][0];evidence=finding['evidence'][0]
            assert evidence['location']['type']=='word_text_box'
            assert evidence['location']['paragraph']==4 and evidence['location']['text_box']==2
            assert text_count(report,'Normal daily working hours are 9 hours.')==1
        reader_results.append({'name':name,'engine':report['engine'],'outcomes':[f['outcome']for f in assessment['review']['findings'][:5]],'passed':True})
        if public_before is None:public_before=source_hash(root/'library/catalog.sqlite3')

    raw_before=dict(query('SELECT id,value FROM review_runs ORDER BY id'))
    for name,case in expected['ocr_cases'].items():
        sequence+=1;options=args.work/f'{sequence}-options.json';options.write_text(json.dumps(case['options']),encoding='utf-8')
        saved=invoke('--ocr-file',[args.samples/name,'--options',options]);report=inspect(saved,name,case)
        assert report['engine']=='complyph-ocr-2'
        assert report['ocr']['processing']=={'profile':'complyph-raster-1','settings':case['options']}
        for page in report['ocr']['pages']:
            assert 0<page['processing']['width']<=2800 and 0<page['processing']['height']<=2800
            assert -300<=page['processing']['deskew_hundredths']<=300
        if case.get('enhanced'):assert any(p['processing']['contrast_stretched']for p in report['ocr']['pages'])
        assessment=review(args.samples/name,case);ocr_reviews[name]=assessment
        state=invoke('--review-state',[assessment['id']]);states[assessment['id']]=state
        assert state['reading_context']['warnings']==report['warnings']
        assert assessment['review']['ocr']['processing']['settings']==case['options']
        ocr_results.append({'name':name,'engine':report['engine'],'options':case['options'],'pages':len(report['units']),'outcomes':[f['outcome']for f in assessment['review']['findings'][:5]],'passed':True})

    # Reading changes never rewrite an earlier review's warnings or exact text association.
    for identifier,state in states.items():
        reopened=invoke('--review-state',[identifier])
        assert reopened['saved']==state['saved']
        assert reopened['reading_context']==state['reading_context']
        assert reopened['document_version']==state['document_version']
    for identifier,value in raw_before.items():assert query('SELECT value FROM review_runs WHERE id=?',(identifier,))[0][0]==value

    # Rerun identical bytes using a distinct supported mode; this is a method change.
    name='Synthetic sideways scan.pdf';old=ocr_reviews[name]
    settings=dict(expected['ocr_cases'][name]['options'],layout='single_column')
    options=args.work/'one-block.json';options.write_text(json.dumps(settings),encoding='utf-8')
    updated=invoke('--ocr-file',[args.samples/name,'--options',options])
    newer=review(args.samples/name,expected['ocr_cases'][name])
    old_state=invoke('--review-state',[old['id']]);new_state=invoke('--review-state',[newer['id']])
    comparison=action({'action':'compare','request':{'left_reading_id':old_state['document_version']['reading_id'],'right_reading_id':new_state['document_version']['reading_id'],'left_review_id':old['id'],'right_review_id':newer['id']}})
    assert comparison['reading_only_change']and comparison['same_file_bytes']
    assert all(row['change']=='not_comparable'for row in comparison['findings'])
    assert old_state['saved']==old and old_state['reading_context']==states[old['id']]['reading_context']
    cached=invoke('--read-file',[args.samples/name]);assert cached['report']==updated['report']

    # Export the exact historical low-quality reading, despite later cache changes.
    blank=ocr_reviews['Synthetic blank OCR page.pdf']
    pdf=invoke('--export-review-pdf',[blank['id']],pdf=True)
    reread=invoke('--read-file',[pdf],data=args.work/'pdf-reader-data')
    exported=' '.join(' '.join(u['text']for u in reread['report']['units']).split())
    for phrase in ['Saved reading warning','no OCR text','scan enhancement on','Pixel-processing profile','complyph-raster-1']:
        assert phrase in exported,(phrase,exported[:1000])
    assert original_hashes['Synthetic blank OCR page.pdf']in exported

    # Reject invalid options before creating a private root or touching an original.
    invalid=args.work/'invalid-options.json';invalid.write_text('{"upload":"https://example.invalid"}',encoding='utf-8')
    invalid_root=args.work/'invalid-data'
    invoke('--ocr-file',[args.samples/name,'--options',invalid],data=invalid_root,success=False)
    assert not invalid_root.exists()
    result=subprocess.run([str(args.app),'--ocr-file',str(args.samples/name),'--output',str(args.samples/name),'--data-dir',str(root)],capture_output=True,timeout=30)
    assert result.returncode!=0
    assert not invoke('--ocr-file',[args.samples/'Synthetic protected.pdf'],success=False)
    for name,digest in original_hashes.items():assert sha(args.samples/name)==digest,name
    assert source_hash(root/'library/catalog.sqlite3')==public_before
    assert query('PRAGMA user_version')[0][0]==6
    checks={name:True for name in [
        'installed_spatial_pdf_passed','installed_word_text_boxes_passed','installed_font_image_warnings_passed',
        'installed_ocr_options_passed','installed_rotation_enhancement_passed','installed_reading_warning_gates_passed',
        'installed_reading_history_passed','installed_reading_pdf_provenance_passed','quality_originals_preserved','quality_public_catalog_preserved']}
    output={'private_schema':6,'public_schema':5,'profile':'initial-reading-quality-v1','synthetic_quality_fixture_count':12,
       'workflow_cases':[{'name':name,'passed':True}for name in ['spatial_pdf_and_fonts','word_text_boxes_and_omissions','local_ocr_processing','numeric_review_quality_gates','reading_history_and_local_exports','offline_originals_and_source_boundary']],
       'checks':checks,'reader_results':reader_results,'ocr_results':ocr_results}
    with args.output.open('x',encoding='utf-8')as file:json.dump(output,file,indent=2)
    print('Reading quality passed: eight ordinary cases, six OCR cases and six workflows.')


def text_count(report,phrase):
    return '\n'.join(u['text']for u in report['units']).count(phrase)


def source_hash(path):
    digest=hashlib.sha256()
    with closing(sqlite3.connect(path.resolve().as_uri()+'?mode=ro',uri=True))as db:
        for row in db.execute('SELECT version_id,raw_bytes,reading_text,metadata_json,verified_at FROM source_snapshots ORDER BY version_id'):
            for value in row:
                value=value if isinstance(value,bytes)else str(value).encode('utf-8')
                digest.update(len(value).to_bytes(8,'big'));digest.update(value)
    return digest.hexdigest()


if __name__=='__main__':main()
