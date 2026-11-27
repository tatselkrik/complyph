param([string]$Installer, [string]$FixtureRoot, [string]$InstallDirectory, [string]$OutputPath)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$fixture = [IO.Path]::GetFullPath($FixtureRoot).TrimEnd([IO.Path]::DirectorySeparatorChar)
$destination = [IO.Path]::GetFullPath($InstallDirectory)
if (!$destination.StartsWith($fixture + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath (Join-Path $destination 'ComplyPH.exe'))) { throw 'Use an unused application folder inside the isolated install fixture.' }
$data = Join-Path $fixture 'released-v013-reading-data'
$original = Join-Path $fixture 'Released OCR policy.pdf'
if ((Test-Path -LiteralPath $data) -or (Test-Path -LiteralPath $original) -or (Test-Path -LiteralPath $OutputPath)) { throw 'The reading-upgrade fixture already exists.' }
$previous = Join-Path $fixture 'ComplyPH-Setup-0.13.0.exe'
Invoke-WebRequest 'https://github.com/tatselkrik/complyph/releases/download/v0.13.0/ComplyPH-Setup-0.13.0.exe' -OutFile $previous
if ((Get-FileHash -LiteralPath $previous -Algorithm SHA256).Hash.ToLowerInvariant() -ne 'd908da2e292c0af0f7a84f17be79dfabd28a3c4486c52fe27fe22e03551e18d1') { throw 'Released 0.13.0 installer hash mismatch.' }
$installed = Start-Process -FilePath $previous -ArgumentList '/S', "/D=$destination" -Wait -PassThru
if ($installed.ExitCode -ne 0) { throw 'Could not install the released reading-history fixture.' }
$application = Join-Path $destination 'ComplyPH.exe'
function Invoke-ReadingUpgrade([string[]]$Arguments) {
    $info = [Diagnostics.ProcessStartInfo]::new(); $info.FileName = $application; $info.UseShellExecute = $false; $info.RedirectStandardError = $true
    foreach ($argument in $Arguments) { $info.ArgumentList.Add($argument) }
    $process = [Diagnostics.Process]::Start($info); $errorText = $process.StandardError.ReadToEndAsync()
    if (!$process.WaitForExit(150000)) { $process.Kill($true); throw 'Reading-upgrade diagnostic timed out.' }
    if ($process.ExitCode -ne 0) { throw ('Reading-upgrade diagnostic failed: ' + $errorText.Result) }
}
Copy-Item -LiteralPath (Join-Path $root 'examples/Synthetic OCR policy.pdf') -Destination $original
$oldOcr = Join-Path $fixture 'released-v013-ocr.json'
Invoke-ReadingUpgrade @('--ocr-file',$original,'--output',$oldOcr,'--data-dir',$data)
$scope = (Get-Content -LiteralPath (Join-Path $root 'examples/expected-checks.json') -Raw | ConvertFrom-Json -AsHashtable).context
$scope['ocr_text_confirmed'] = $true
$scopeFile = Join-Path $fixture 'released-v013-scope.json'
$scope | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $scopeFile -Encoding utf8NoBOM
$reviewFile = Join-Path $fixture 'released-v013-review.json'
Invoke-ReadingUpgrade @('--check-file',$original,'--context',$scopeFile,'--output',$reviewFile,'--data-dir',$data)
$review = Get-Content -LiteralPath $reviewFile -Raw | ConvertFrom-Json
$stateFile = Join-Path $fixture 'released-v013-state.json'
Invoke-ReadingUpgrade @('--review-state',"$($review.id)",'--output',$stateFile,'--data-dir',$data)
$state = Get-Content -LiteralPath $stateFile -Raw | ConvertFrom-Json
$noteFile = Join-Path $fixture 'released-v013-note.json'
@{ review_fingerprint=$state.review_fingerprint; rule_id='PH-LAB-083-01'; expected_revision=0; reviewer='Synthetic prior-version reviewer'; note='Preserve this fictional prior-version resolution.'; status='resolved' } | ConvertTo-Json | Set-Content -LiteralPath $noteFile -Encoding utf8NoBOM
Invoke-ReadingUpgrade @('--review-action',"$($review.id)",'--metadata',$noteFile,'--output',(Join-Path $fixture 'released-v013-noted.json'),'--data-dir',$data)
$env:COMPLYPH_READING_UPGRADE_DATA = $data
$env:COMPLYPH_READING_UPGRADE_BEFORE = Join-Path $fixture 'released-v013-before.json'
$env:COMPLYPH_READING_UPGRADE_OLD = $oldOcr
@'
import os,pathlib,sqlite3,json
from contextlib import closing
data=pathlib.Path(os.environ['COMPLYPH_READING_UPGRADE_DATA'])
old=json.loads(pathlib.Path(os.environ['COMPLYPH_READING_UPGRADE_OLD']).read_text(encoding='utf-8-sig'))
assert old['report']['engine']=='complyph-ocr-1' and 'processing' not in old['report']['ocr']
with closing(sqlite3.connect(data/'private/workspace.sqlite3')) as db:
 assert db.execute('PRAGMA user_version').fetchone()[0]==6
 tables=['documents','preferences','document_extractions','review_runs','review_actions','document_versions','document_readings','review_version_links']
 before={t:db.execute('SELECT * FROM '+t+' ORDER BY 1').fetchall() for t in tables}
 assert len(before['document_readings'])==1
 pathlib.Path(os.environ['COMPLYPH_READING_UPGRADE_BEFORE']).write_text(json.dumps(before),encoding='utf-8')
'@ | python -
if ($LASTEXITCODE -ne 0) { throw 'Could not capture released 0.13.0 OCR/history.' }
$catalog = Join-Path $data 'library/catalog.sqlite3'
$catalogHash = (Get-FileHash -LiteralPath $catalog -Algorithm SHA256).Hash
$originalHash = (Get-FileHash -LiteralPath $original -Algorithm SHA256).Hash
$upgrade = Start-Process -FilePath $Installer -ArgumentList '/S' -Wait -PassThru
if ($upgrade.ExitCode -ne 0 -or !(Test-Path -LiteralPath (Join-Path $destination 'Reading-quality.md'))) { throw 'Reading-quality application upgrade failed.' }
$reopenedFile = Join-Path $fixture 'released-v013-reopened.json'
Invoke-ReadingUpgrade @('--review-state',"$($review.id)",'--output',$reopenedFile,'--data-dir',$data)
$cacheFile = Join-Path $fixture 'released-v013-cache-reused.json'
Invoke-ReadingUpgrade @('--read-file',$original,'--output',$cacheFile,'--data-dir',$data)
$env:COMPLYPH_READING_UPGRADE_REOPENED = $reopenedFile
$env:COMPLYPH_READING_UPGRADE_REUSED = $cacheFile
@'
import os,pathlib,sqlite3,json
from contextlib import closing
before=json.loads(pathlib.Path(os.environ['COMPLYPH_READING_UPGRADE_BEFORE']).read_text())
state=json.loads(pathlib.Path(os.environ['COMPLYPH_READING_UPGRADE_REOPENED']).read_text(encoding='utf-8-sig'))
old=json.loads(pathlib.Path(os.environ['COMPLYPH_READING_UPGRADE_OLD']).read_text(encoding='utf-8-sig'))
reused=json.loads(pathlib.Path(os.environ['COMPLYPH_READING_UPGRADE_REUSED']).read_text(encoding='utf-8-sig'))
assert reused==old
assert state['reading_context']['engine']=='complyph-ocr-1'
assert state['reading_context']['warnings']==old['report']['warnings']
assert state['events'][-1]['new_status']=='resolved'
with closing(sqlite3.connect(pathlib.Path(os.environ['COMPLYPH_READING_UPGRADE_DATA'])/'private/workspace.sqlite3')) as db:
 for table,expected in before.items():assert [list(r) for r in db.execute('SELECT * FROM '+table+' ORDER BY 1')]==expected,table
'@ | python -
if ($LASTEXITCODE -ne 0) { throw 'Released OCR cache, reading fingerprints or review history changed during upgrade.' }
$options = Join-Path $fixture 'released-v013-rerun-options.json'
@{ rotation='none'; layout='single_column'; enhance_scan=$true } | ConvertTo-Json | Set-Content -LiteralPath $options -Encoding utf8NoBOM
$rerunFile = Join-Path $fixture 'released-v013-rerun.json'
Invoke-ReadingUpgrade @('--ocr-file',$original,'--options',$options,'--output',$rerunFile,'--data-dir',$data)
$historicalFile = Join-Path $fixture 'released-v013-after-rerun.json'
Invoke-ReadingUpgrade @('--review-state',"$($review.id)",'--output',$historicalFile,'--data-dir',$data)
$env:COMPLYPH_READING_UPGRADE_NEW = $rerunFile
$env:COMPLYPH_READING_UPGRADE_AFTER = $historicalFile
@'
import os,pathlib,sqlite3,json
from contextlib import closing
load=lambda key:json.loads(pathlib.Path(os.environ[key]).read_text(encoding='utf-8-sig'))
before=load('COMPLYPH_READING_UPGRADE_BEFORE')
new=load('COMPLYPH_READING_UPGRADE_NEW');old=load('COMPLYPH_READING_UPGRADE_OLD')
assert new['sha256']==old['sha256'] and new['report']['engine']=='complyph-ocr-2'
assert new['report']['ocr']['processing']['settings']['layout']=='single_column'
prior=load('COMPLYPH_READING_UPGRADE_REOPENED');after=load('COMPLYPH_READING_UPGRADE_AFTER')
for field in ['saved','review_fingerprint','document_version','reading_context','events']:assert prior[field]==after[field],field
with closing(sqlite3.connect(pathlib.Path(os.environ['COMPLYPH_READING_UPGRADE_DATA'])/'private/workspace.sqlite3')) as db:
 assert db.execute('PRAGMA user_version').fetchone()[0]==6
 for table,expected in before.items():
  if table=='document_extractions':continue
  rows=[list(r) for r in db.execute('SELECT * FROM '+table+' ORDER BY 1')]
  assert (rows[:len(expected)] if table=='document_readings' else rows)==expected,table
 assert db.execute('SELECT COUNT(*) FROM document_readings').fetchone()[0]==2
'@ | python -
if ($LASTEXITCODE -ne 0) { throw 'Current OCR rerun changed an original retained reading or its historical review.' }
if ((Get-FileHash -LiteralPath $catalog -Algorithm SHA256).Hash -ne $catalogHash -or (Get-FileHash -LiteralPath $original -Algorithm SHA256).Hash -ne $originalHash) { throw 'Reading-method upgrade changed the source catalog or original.' }
$remove = Start-Process -FilePath (Join-Path $destination 'Uninstall.exe') -ArgumentList '/S' -Wait -PassThru
if ($remove.ExitCode -ne 0) { throw 'Reading-upgrade fixture uninstall failed.' }
$deadline = [DateTime]::UtcNow.AddSeconds(15)
while ((Test-Path -LiteralPath $application) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 250 }
if (Test-Path -LiteralPath $application) { throw 'Reading-upgrade fixture executable remains.' }
@{ passed=$true; private_schema=6; public_schema=5; old_engine='complyph-ocr-1'; new_engine='complyph-ocr-2'; retained_readings=2; old_reports_preserved=$true; old_warning_context_preserved=$true; old_cached_reading_reused=$true; source_catalog_preserved=$true; originals_preserved=$true } | ConvertTo-Json | Set-Content -LiteralPath $OutputPath -Encoding utf8NoBOM
Write-Output 'Released 0.13.0 OCR/history preserved through upgrade, cache reuse and current-method rerun.'
