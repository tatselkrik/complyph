$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or !$env:RUNNER_TEMP) {
    throw 'This installer check requires an isolated GitHub Actions runner.'
}
$root = Split-Path -Parent $PSScriptRoot
$fixtureRoot = Join-Path $env:RUNNER_TEMP 'complyph-install-smoke'
$installDirectory = Join-Path $fixtureRoot 'upgraded-app'
$legacyData = Join-Path $env:LOCALAPPDATA 'LaborLens PH'
$currentData = Join-Path $env:LOCALAPPDATA 'ComplyPH'
$machineData = Join-Path $env:ProgramData 'ComplyPH'
foreach ($path in @($legacyData, $currentData, $machineData)) {
    if (Test-Path -LiteralPath $path) { throw 'An existing workspace was found; refusing to use it.' }
}
$registryBase = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\'
foreach ($name in @('LaborLensPH','ComplyPH')) {
    foreach ($view in @([Microsoft.Win32.RegistryView]::Registry32,[Microsoft.Win32.RegistryView]::Registry64)) {
        $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::CurrentUser,$view)
        try {
            $existing = $base.OpenSubKey($registryBase + $name)
            if ($existing) { $existing.Dispose(); throw "Existing installation: $name" }
        } finally { $base.Dispose() }
    }
}
New-Item -ItemType Directory -Force -Path $fixtureRoot, (Join-Path $legacyData 'private'), (Join-Path $legacyData 'library') | Out-Null
$desktop = [Environment]::GetFolderPath('DesktopDirectory')
$programs = [Environment]::GetFolderPath('Programs')
New-Item -ItemType Directory -Force -Path $desktop, $programs | Out-Null
$original = Join-Path $fixtureRoot 'Synthetic original.docx'
Copy-Item -LiteralPath (Join-Path $root 'examples/Synthetic workplace policy.docx') -Destination $original
$originalHash = (Get-FileHash -LiteralPath $original -Algorithm SHA256).Hash
$legacyInstaller = Join-Path $fixtureRoot 'LaborLensPH-Setup-0.1.0.exe'
Invoke-WebRequest -Uri 'https://github.com/tatselkrik/complyph/releases/download/v0.1.0/LaborLensPH-Setup-0.1.0.exe' -OutFile $legacyInstaller
if ((Get-FileHash -LiteralPath $legacyInstaller -Algorithm SHA256).Hash.ToLowerInvariant() -ne 'ba61cac1779ea8f63b712397312a6145d03395f1677030a59b29aaec29c40639') {
    throw 'Legacy installer hash mismatch.'
}
$old = Start-Process -FilePath $legacyInstaller -ArgumentList '/S', "/D=$installDirectory" -Wait -PassThru
if ($old.ExitCode -ne 0) { throw 'Legacy fixture installation failed.' }
$env:COMPLYPH_SMOKE_LEGACY = $legacyData
$env:COMPLYPH_SMOKE_ORIGINAL = $original
@'
import json, os, pathlib, sqlite3
root = pathlib.Path(os.environ["COMPLYPH_SMOKE_LEGACY"])
original = pathlib.Path(os.environ["COMPLYPH_SMOKE_ORIGINAL"])
with sqlite3.connect(root / "private/workspace.sqlite3") as db:
    db.executescript("CREATE TABLE documents(id INTEGER PRIMARY KEY,path TEXT NOT NULL UNIQUE,name TEXT NOT NULL,kind TEXT NOT NULL,bytes INTEGER NOT NULL,added_at INTEGER NOT NULL,modified_at INTEGER);CREATE TABLE preferences(id INTEGER PRIMARY KEY,value TEXT NOT NULL);PRAGMA user_version=1;")
    db.execute("INSERT INTO documents VALUES(1,?,?,?,?,?,NULL)", (str(original), original.name, "DOCX", original.stat().st_size, 1))
    db.execute("INSERT INTO preferences VALUES(1,?)", (json.dumps({"remember_page":True,"last_page":"documents","show_full_paths":True}),))
with sqlite3.connect(root / "library/catalog.sqlite3") as db:
    db.executescript("CREATE TABLE sources(id INTEGER PRIMARY KEY,title TEXT NOT NULL,issuer TEXT NOT NULL,source_url TEXT NOT NULL UNIQUE,verification_status TEXT NOT NULL DEFAULT 'unverified');CREATE TABLE source_versions(id INTEGER PRIMARY KEY,source_id INTEGER NOT NULL REFERENCES sources(id),content_sha256 TEXT NOT NULL,local_path TEXT NOT NULL,retrieved_at INTEGER NOT NULL,published_at INTEGER,effective_from INTEGER,UNIQUE(source_id,content_sha256));PRAGMA user_version=1;")
'@ | python -
if ($LASTEXITCODE -ne 0) { throw 'Legacy database fixtures failed.' }
$privateDatabase = Join-Path $legacyData 'private/workspace.sqlite3'
$privateHash = (Get-FileHash -LiteralPath $privateDatabase -Algorithm SHA256).Hash
$sentinel = Join-Path $legacyData 'private/preserve.fixture'
'Synthetic preservation fixture.' | Set-Content -LiteralPath $sentinel
$installer = Join-Path $root 'dist/windows/ComplyPH-Setup-0.15.0.exe'
$process = Start-Process -FilePath $installer -ArgumentList '/S' -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "ComplyPH upgrade exit code: $($process.ExitCode)" }
$application = Join-Path $installDirectory 'ComplyPH.exe'
foreach ($path in @($application, (Join-Path $desktop 'ComplyPH.lnk'), (Join-Path $programs 'ComplyPH.lnk'), (Join-Path $installDirectory 'starter-manifest.json'))) {
    if (!(Test-Path -LiteralPath $path)) { throw "Missing installed file: $path" }
}
foreach ($path in @((Join-Path $installDirectory 'LaborLensPH.exe'), (Join-Path $desktop 'LaborLens PH.lnk'), (Join-Path $programs 'LaborLens PH.lnk'))) {
    if (Test-Path -LiteralPath $path) { throw "Legacy branding not replaced: $path" }
}
$startup = Start-Process -FilePath $application -ArgumentList '--help' -Wait -PassThru
if ($startup.ExitCode -ne 0) { throw 'Application loader check failed.' }
# The desktop, not the public updater, copies private SQLite databases before opening.
$legacyPrivateHash = $privateHash
$info = [Diagnostics.ProcessStartInfo]::new()
$info.FileName = $application; $info.UseShellExecute = $false
foreach ($argument in @('--read-file', $original, '--output', (Join-Path $fixtureRoot 'migrated-original.json'))) { $info.ArgumentList.Add($argument) }
$migration = [Diagnostics.Process]::Start($info)
if (!$migration.WaitForExit(45000)) { $migration.Kill($true); throw 'Migration timed out.' }
if ($migration.ExitCode -ne 0) { throw 'Desktop workspace migration failed.' }
if ((Get-FileHash -LiteralPath $privateDatabase -Algorithm SHA256).Hash -ne $legacyPrivateHash) { throw 'Migration changed the legacy private database.' }
if (!(Test-Path -LiteralPath (Join-Path $machineData 'migration.json'))) { throw 'Migration marker missing.' }
$privateDatabase = Join-Path $machineData 'private/workspace.sqlite3'
$privateHash = (Get-FileHash -LiteralPath $privateDatabase -Algorithm SHA256).Hash
$env:COMPLYPH_SMOKE_ACTIVE = $machineData
$acl = Get-Acl -LiteralPath $machineData
$ownerSid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
if (!$acl.AreAccessRulesProtected -or $acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -ne $ownerSid) { throw 'Workspace ACL is not protected for its owning account.' }
$allowed = @($ownerSid,'S-1-5-18','S-1-5-32-544')
foreach ($rule in $acl.Access) {
    $sid = $rule.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value
    if ($sid -notin $allowed -or $rule.AccessControlType -ne 'Allow') { throw "Unexpected workspace access: $sid" }
}
$statusJson = & (Join-Path $root 'target/release/complyph-updater.exe') '--status'
if ($LASTEXITCODE -ne 0) { throw 'Default library discovery or import failed.' }
$status = $statusJson | ConvertFrom-Json
if ($status.source_count -ne 19 -or $status.verified_source_count -ne 14 -or $status.identity_checked_upload_count -ne 3 -or $status.provisional_source_count -ne 2 -or $status.domains.Count -ne 7 -or $null -eq $status.bundled_verified_at -or $null -ne $status.last_successful_check -or !$status.automatic_updates_available -or $status.approved_update_count -ne 0 -or $status.pending_update_count -ne 0) {
    throw 'Unexpected starter-library or update status.'
}
if (@($status.domains | Where-Object { $_.source_count -lt 1 }).Count -gt 0) { throw 'A domain has no source snapshots.' }
if (Test-Path -LiteralPath $currentData) { throw 'Upgrade created a second data root.' }
if ((Get-FileHash -LiteralPath $privateDatabase -Algorithm SHA256).Hash -ne $privateHash) { throw 'Public catalog import changed private data.' }
$catalog = Join-Path $machineData 'library/catalog.sqlite3'
function Set-SourceWorkerEnvironment($processInfo) {
    $processInfo.Environment.Clear()
    foreach ($name in @('SystemDrive','SystemRoot','WINDIR','TEMP','TMP','USERPROFILE','LOCALAPPDATA','APPDATA')) {
        $value = [Environment]::GetEnvironmentVariable($name)
        if ($null -ne $value) { $processInfo.Environment[$name] = $value }
    }
}
$installedUpdater = Join-Path $installDirectory 'complyph-updater.exe'
if (!(Test-Path -LiteralPath $installedUpdater)) { throw 'The installed source updater is missing.' }
$info = [Diagnostics.ProcessStartInfo]::new(); $info.FileName = $installedUpdater; $info.UseShellExecute = $false; $info.RedirectStandardOutput = $true; Set-SourceWorkerEnvironment $info
foreach ($argument in @('--self-test-isolation','--probe-file',$privateDatabase,'--probe-file',$catalog,'--probe-file',$original)) { $info.ArgumentList.Add($argument) }
$isolationProcess = [Diagnostics.Process]::Start($info)
$isolationTask = $isolationProcess.StandardOutput.ReadToEndAsync()
if (!$isolationProcess.WaitForExit(45000)) { $isolationProcess.Kill($true); throw 'Downloader isolation verification timed out.' }
if ($isolationProcess.ExitCode -ne 0) { throw 'The installed AppContainer downloader failed its private-file denial test.' }
$isolation = $isolationTask.Result | ConvertFrom-Json
if (!$isolation.app_container -or !$isolation.all_files_denied -or $isolation.files_tested -ne 3) { throw 'The network worker could open a protected synthetic fixture.' }
$info = [Diagnostics.ProcessStartInfo]::new(); $info.FileName = $installedUpdater; $info.UseShellExecute = $false; $info.RedirectStandardOutput = $true; Set-SourceWorkerEnvironment $info
foreach ($argument in @('--check','--library-dir',(Join-Path $machineData 'library'))) { $info.ArgumentList.Add($argument) }
$emptyBatch = [Diagnostics.Process]::Start($info)
$emptyTask = $emptyBatch.StandardOutput.ReadToEndAsync()
if (!$emptyBatch.WaitForExit(45000)) { $emptyBatch.Kill($true); throw 'The no-approval check timed out.' }
if ($emptyBatch.ExitCode -ne 0 -or @((($emptyTask.Result | ConvertFrom-Json).checks)).Count -ne 0) { throw 'An unapproved source was checked.' }
# Observe a real certificate-validated HTTP response in an isolated public-only catalog.
# A server rejection (for example 403) proves transport, not source availability.
$probeLibrary = Join-Path $fixtureRoot 'https-probe-library'
& $installedUpdater '--status' '--library-dir' $probeLibrary | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'The public HTTPS probe catalog could not be initialized.' }
$env:COMPLYPH_SMOKE_PROBE_LIBRARY = $probeLibrary
$liveResponseObserved = $false
$liveProbeResults = @()
foreach ($key in @('ra-10173','ra-11223','ra-11199')) {
    $env:COMPLYPH_SMOKE_PROBE_KEY = $key
    @'
import json,os,pathlib,sqlite3,time
with sqlite3.connect(pathlib.Path(os.environ['COMPLYPH_SMOKE_PROBE_LIBRARY'])/'catalog.sqlite3') as db:
 db.execute('UPDATE source_update_watches SET enabled=0')
 row=db.execute('SELECT s.id,p.metadata_json FROM sources s JOIN source_versions v ON v.source_id=s.id JOIN source_snapshots p ON p.version_id=v.id WHERE s.stable_key=? ORDER BY v.id DESC LIMIT 1',(os.environ['COMPLYPH_SMOKE_PROBE_KEY'],)).fetchone()
 db.execute('INSERT INTO source_update_watches(source_id,url,enabled,approved_at) VALUES(?,?,1,?)',(row[0],json.loads(row[1])['source_url'],int(time.time())))
'@ | python -
    if ($LASTEXITCODE -ne 0) { throw 'The explicit public probe URL fixture could not be approved.' }
    $info = [Diagnostics.ProcessStartInfo]::new(); $info.FileName = $installedUpdater; $info.UseShellExecute = $false; $info.RedirectStandardOutput = $true; Set-SourceWorkerEnvironment $info
    foreach ($argument in @('--check','--library-dir',$probeLibrary)) { $info.ArgumentList.Add($argument) }
    $liveProcess = [Diagnostics.Process]::Start($info)
    $liveTask = $liveProcess.StandardOutput.ReadToEndAsync()
    if (!$liveProcess.WaitForExit(90000)) { $liveProcess.Kill($true); throw 'The isolated HTTPS probe timed out.' }
    if ($liveProcess.ExitCode -ne 0) { throw 'The isolated HTTPS coordinator failed.' }
    $live = $liveTask.Result | ConvertFrom-Json
    $liveProbeResults += $live.checks
    $observed = @($live.checks | Where-Object { $_.outcome -in @('changed','unchanged') -or $_.detail -match 'returned HTTP [0-9]{3}' })
    if ($observed.Count -gt 0) { $liveResponseObserved = $true; break }
}
if (!$liveResponseObserved) { throw ('No secure public HTTP response was observed: ' + ($liveProbeResults | ConvertTo-Json -Compress)) }
if ((Get-FileHash -LiteralPath $privateDatabase -Algorithm SHA256).Hash -ne $privateHash) { throw 'Downloader verification changed private data.' }
$catalogHash = (Get-FileHash -LiteralPath $catalog -Algorithm SHA256).Hash
# Exercise the installed GUI-subsystem executable headlessly, including its child worker.
$expected = Get-Content -LiteralPath (Join-Path $installDirectory 'Samples/expected-results.json') -Raw | ConvertFrom-Json -AsHashtable
$readerResults = @()
foreach ($name in $expected.Keys | Sort-Object) {
    $sample = Join-Path $installDirectory "Samples/$name"
    $hashBefore = (Get-FileHash -LiteralPath $sample -Algorithm SHA256).Hash
    $reportFile = Join-Path $fixtureRoot ($name + '.json')
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $application
    $info.UseShellExecute = $false
    foreach ($argument in @('--read-file', $sample, '--output', $reportFile)) { $info.ArgumentList.Add($argument) }
    $reading = [Diagnostics.Process]::Start($info)
    if (!$reading.WaitForExit(30000)) { $reading.Kill($true); throw "Installed reader timed out: $name" }
    if ($reading.ExitCode -ne 0) { throw "Installed reader failed: $name" }
    $saved = Get-Content -LiteralPath $reportFile -Raw | ConvertFrom-Json
    if ($saved.report.status -ne $expected[$name].status) { throw "Wrong reader state: $name ($($saved.report.status))" }
    $text = ($saved.report.units | ForEach-Object { $_.text }) -join "`n"
    foreach ($needle in $expected[$name]['contains']) { if (!$text.Contains($needle)) { throw "Missing synthetic text: $name" } }
    foreach ($needle in $expected[$name]['excludes']) { if ($text.Contains($needle)) { throw "Unsupported content was read: $name" } }
    if ($expected[$name].ContainsKey('units') -and $saved.report.units.Count -ne $expected[$name].units) { throw "Wrong location count: $name" }
    if ((Get-FileHash -LiteralPath $sample -Algorithm SHA256).Hash -ne $hashBefore) { throw "Reader changed its original: $name" }
    $readerResults += [ordered]@{ name = $name; status = $saved.report.status; locations = $saved.report.units.Count; original_preserved = $true }
}
@'
import json, os, pathlib, sqlite3
root = pathlib.Path(os.environ['COMPLYPH_SMOKE_ACTIVE'])
with sqlite3.connect(root / 'private/workspace.sqlite3') as db:
    assert db.execute('PRAGMA user_version').fetchone()[0] == 6
    original = db.execute('SELECT id,path,added_at FROM documents WHERE id=1').fetchone()
    assert original == (1, os.environ['COMPLYPH_SMOKE_ORIGINAL'], 1)
    prefs = json.loads(db.execute('SELECT value FROM preferences WHERE id=1').fetchone()[0])
    assert prefs == {'remember_page':True, 'last_page':'documents', 'show_full_paths':True}
    assert db.execute('SELECT COUNT(*) FROM document_extractions').fetchone()[0] == 8
'@ | python -
if ($LASTEXITCODE -ne 0) { throw 'Private schema upgrade lost existing records or extraction caches.' }
if (Test-Path -LiteralPath $currentData) { throw 'Reader created a duplicate data root.' }
# Exercise real installed reading, pinned sources, scope gates, comparisons and saved reports.
$checkCases = Get-Content -LiteralPath (Join-Path $installDirectory 'Samples/expected-checks.json') -Raw | ConvertFrom-Json -AsHashtable
$contextFile = Join-Path $fixtureRoot 'check-context.json'
$checkCases.context | ConvertTo-Json | Set-Content -LiteralPath $contextFile -Encoding utf8NoBOM
$checkResults = @()
foreach ($name in $checkCases.expected.Keys | Sort-Object) {
    $sample = Join-Path $installDirectory "Samples/$name"
    $before = (Get-FileHash -LiteralPath $sample -Algorithm SHA256).Hash
    $output = Join-Path $fixtureRoot ($name + '.check.json')
    $info = [Diagnostics.ProcessStartInfo]::new(); $info.FileName = $application; $info.UseShellExecute = $false
    foreach ($argument in @('--check-file',$sample,'--context',$contextFile,'--output',$output)) { $info.ArgumentList.Add($argument) }
    $checking = [Diagnostics.Process]::Start($info)
    if (!$checking.WaitForExit(45000)) { $checking.Kill($true); throw "Installed checking timed out: $name" }
    if ($checking.ExitCode -ne 0) { throw "Installed checking failed: $name" }
    $saved = Get-Content -LiteralPath $output -Raw | ConvertFrom-Json
    $outcomes = @($saved.review.findings | ForEach-Object { $_.outcome })
    if (($outcomes -join ',') -ne ($checkCases.expected[$name] -join ',')) { throw "Incorrect checking outcomes: $name" }
    if (!$saved.review.context.operative_terms_confirmed -or $saved.review.engine -ne 'complyph-rules-1') { throw 'Review audit context missing.' }
    if ((Get-FileHash -LiteralPath $sample -Algorithm SHA256).Hash -ne $before) { throw 'Checking changed an original.' }
    $checkResults += [ordered]@{ name=$name; review_id=$saved.id; outcomes=$outcomes; original_preserved=$true }
}
@'
import os,pathlib,sqlite3
with sqlite3.connect(pathlib.Path(os.environ['COMPLYPH_SMOKE_ACTIVE'])/'private/workspace.sqlite3') as db:
 assert db.execute('SELECT COUNT(*) FROM review_runs').fetchone()[0]==3
 assert db.execute('SELECT COUNT(*) FROM document_extractions').fetchone()[0]==11
'@ | python -
if ($LASTEXITCODE -ne 0) { throw 'Saved report/extraction persistence failed.' }
# Exercise separate reviewer history and local PDF generation in the installed GUI binary.
function Invoke-ReviewDiagnostic([string[]]$Arguments, [bool]$ShouldSucceed = $true) {
    $info = [Diagnostics.ProcessStartInfo]::new(); $info.FileName = $application; $info.UseShellExecute = $false
    foreach ($argument in $Arguments) { $info.ArgumentList.Add($argument) }
    $process = [Diagnostics.Process]::Start($info)
    if (!$process.WaitForExit(45000)) { $process.Kill($true); throw 'Installed review tracking/export timed out.' }
    if (($process.ExitCode -eq 0) -ne $ShouldSucceed) { throw ('Unexpected tracking/export result: ' + ($Arguments -join ' ')) }
}
$reviewId = ($checkResults | Where-Object { $_.name -eq 'Synthetic labour issues.docx' }).review_id
$stateFile = Join-Path $fixtureRoot 'review-state.json'
Invoke-ReviewDiagnostic @('--review-state',"$reviewId",'--output',$stateFile)
$workflow = Get-Content -LiteralPath $stateFile -Raw | ConvertFrom-Json
if ($workflow.events.Count -ne 0 -or $workflow.saved.review.findings.Count -ne 20) { throw 'New review inherited tracking history.' }
$env:COMPLYPH_SMOKE_REVIEW_ID = "$reviewId"
$env:COMPLYPH_SMOKE_REVIEW_ORIGINAL = Join-Path $fixtureRoot 'review-value-before.json'
@'
import os, pathlib, sqlite3
with sqlite3.connect(pathlib.Path(os.environ['COMPLYPH_SMOKE_ACTIVE'])/'private/workspace.sqlite3') as db:
 value=db.execute('SELECT value FROM review_runs WHERE id=?',(int(os.environ['COMPLYPH_SMOKE_REVIEW_ID']),)).fetchone()[0]
 pathlib.Path(os.environ['COMPLYPH_SMOKE_REVIEW_ORIGINAL']).write_text(value,encoding='utf-8')
'@ | python -
if ($LASTEXITCODE -ne 0) { throw 'Could not capture original automated review.' }
$actionFile = Join-Path $fixtureRoot 'review-action.json'
$revision = 0
foreach ($step in @('document','acknowledged','resolved')) {
    $rule = if ($step -eq 'document') { $null } else { 'PH-LAB-083-01' }
    $newStatus = if ($step -eq 'document') { $null } else { $step }
    $action = [ordered]@{ review_fingerprint=$workflow.review_fingerprint; rule_id=$rule; expected_revision=$revision; reviewer='José Example'; note="Fictional ₱500 $step follow-up. Original findings retained."; status=$newStatus }
    $action | ConvertTo-Json | Set-Content -LiteralPath $actionFile -Encoding utf8NoBOM
    $resultFile = Join-Path $fixtureRoot "review-$step.json"
    Invoke-ReviewDiagnostic @('--review-action',"$reviewId",'--metadata',$actionFile,'--output',$resultFile)
    $workflow = Get-Content -LiteralPath $resultFile -Raw | ConvertFrom-Json
    if ($step -ne 'document') { $revision = $workflow.events[-1].id }
}
if ($workflow.events.Count -ne 3 -or $workflow.events[-1].previous_status -ne 'acknowledged' -or $workflow.events[-1].new_status -ne 'resolved' -or $workflow.saved.review.findings[0].outcome -ne 'potential_issue') { throw 'Reviewer history or original result was changed.' }
Invoke-ReviewDiagnostic @('--review-action',"$reviewId",'--metadata',$actionFile,'--output',(Join-Path $fixtureRoot 'review-stale.json')) $false
$pdf = Join-Path $fixtureRoot 'synthetic-review.pdf'
Invoke-ReviewDiagnostic @('--export-review-pdf',"$reviewId",'--output',$pdf)
$pdfHash = (Get-FileHash -LiteralPath $pdf -Algorithm SHA256).Hash
Invoke-ReviewDiagnostic @('--export-review-pdf',"$reviewId",'--output',$pdf) $false
if ((Get-FileHash -LiteralPath $pdf -Algorithm SHA256).Hash -ne $pdfHash) { throw 'PDF overwrite refusal failed.' }
$pdfReading = Join-Path $fixtureRoot 'pdf-reading.json'
Invoke-ReviewDiagnostic @('--read-file',$pdf,'--output',$pdfReading)
$reading = Get-Content -LiteralPath $pdfReading -Raw | ConvertFrom-Json
$text = (($reading.report.units | ForEach-Object { $_.text }) -join ' ') -replace '\s+', ' '
foreach ($needle in @('José','₱500','Acknowledged to Resolved','Human follow-up status: Resolved','Automated result: Potential issue','PH-LAB-083-01','PH-LAB-087-01','PH-LAB-086-01','PH-LAB-085-01','PH-LAB-091-01','PH-LAB-095-01','complyph-pdf-1')) {
    if (!$text.Contains($needle)) { throw "Missing exported PDF content: $needle" }
}
@'
import os,pathlib,sqlite3
with sqlite3.connect(pathlib.Path(os.environ['COMPLYPH_SMOKE_ACTIVE'])/'private/workspace.sqlite3') as db:
 value=db.execute('SELECT value FROM review_runs WHERE id=?',(int(os.environ['COMPLYPH_SMOKE_REVIEW_ID']),)).fetchone()[0]
 assert value==pathlib.Path(os.environ['COMPLYPH_SMOKE_REVIEW_ORIGINAL']).read_text(encoding='utf-8')
 assert db.execute('SELECT COUNT(*) FROM review_actions').fetchone()[0]==3
'@ | python -
if ($LASTEXITCODE -ne 0) { throw 'Tracking changed original findings or stale actions were accepted.' }
# Exercise the actual installed, bundled recognizer; no runtime downloader is available.
$ocrCases = Get-Content -LiteralPath (Join-Path $installDirectory 'Samples/expected-ocr.json') -Raw | ConvertFrom-Json -AsHashtable
$ocrResults = @()
foreach ($name in $ocrCases.Keys | Sort-Object) {
    $sample = Join-Path $installDirectory "Samples/$name"
    $before = (Get-FileHash $sample -Algorithm SHA256).Hash
    $output = Join-Path $fixtureRoot ($name + '.ocr.json')
    Invoke-ReviewDiagnostic @('--ocr-file',$sample,'--output',$output)
    $reading = Get-Content $output -Raw | ConvertFrom-Json
    if ($reading.report.status -ne 'ready' -or $reading.report.engine -ne 'complyph-ocr-2' -or $reading.report.units.Count -ne $ocrCases[$name].pages) { throw "Installed OCR failed: $name" }
    if ($reading.report.ocr.processing.profile -ne 'complyph-raster-1' -or $null -eq $reading.report.ocr.processing.settings) { throw 'Installed OCR processing provenance is missing.' }
    if ($reading.report.ocr.model_sha256 -ne '7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2' -or $reading.report.ocr.renderer -ne 'pdfium-8165' -or $reading.report.ocr.engine_version -ne '5.5.4') { throw 'OCR provenance mismatch.' }
    $text = (($reading.report.units | ForEach-Object { $_.text }) -join ' ') -replace '\s+', ' '
    foreach ($needle in $ocrCases[$name].contains) { if (!$text.Contains($needle)) { throw "OCR missed expected synthetic text: $needle" } }
    if ((Get-FileHash $sample -Algorithm SHA256).Hash -ne $before) { throw 'OCR changed an original.' }
    $ocrResults += [ordered]@{ name=$name; pages=$reading.report.units.Count; original_preserved=$true }
}
$ocrPolicy = Join-Path $installDirectory 'Samples/Synthetic OCR policy.pdf'
foreach ($confirmed in @($false,$true)) {
    $checkCases.context['ocr_text_confirmed'] = $confirmed
    $checkCases.context | ConvertTo-Json | Set-Content $contextFile -Encoding utf8NoBOM
    $output = Join-Path $fixtureRoot "ocr-scope-$confirmed.json"
    Invoke-ReviewDiagnostic @('--check-file',$ocrPolicy,'--context',$contextFile,'--output',$output)
    $checked = Get-Content $output -Raw | ConvertFrom-Json
    $expectedOutcome = if ($confirmed) { 'potential_issue' } else { 'needs_review' }
    if ($null -eq $checked.review.ocr -or @($checked.review.findings[0..4] | Where-Object { $_.outcome -ne $expectedOutcome }).Count -gt 0) { throw 'OCR accuracy confirmation gate failed.' }
    if ($checked.review.findings[5].outcome -ne 'needs_context') { throw 'Earlier scope silently established SIL eligibility.' }
}
$watchFolder = Join-Path $fixtureRoot 'document-intake'
New-Item -ItemType Directory -Force -Path $watchFolder | Out-Null
$watchBytes = [IO.File]::ReadAllBytes((Join-Path $installDirectory 'Samples/Synthetic employment.pdf')) + [Text.Encoding]::ASCII.GetBytes("`n% WATCH-INTAKE-FIXTURE-ONLY`n")
[IO.File]::WriteAllBytes((Join-Path $watchFolder 'Complete.pdf'),$watchBytes)
[IO.File]::WriteAllBytes((Join-Path $watchFolder 'Duplicate.pdf'),$watchBytes)
[IO.File]::WriteAllBytes((Join-Path $watchFolder 'Partial.pdf'),$watchBytes[0..([int]($watchBytes.Length/2))])
$watchResult = Join-Path $fixtureRoot 'folder-intake.json'
Invoke-ReviewDiagnostic @('--watch-folder',$watchFolder,'--output',$watchResult)
$intake = Get-Content $watchResult -Raw | ConvertFrom-Json
if ($intake.scan.added.Count -ne 1 -or $intake.scan.duplicates -ne 1 -or $intake.scan.waiting -ne 1) { throw 'Installed folder intake did not respect completeness/duplicates.' }
@'
import os,pathlib,sqlite3
with sqlite3.connect(pathlib.Path(os.environ['COMPLYPH_SMOKE_ACTIVE'])/'private/workspace.sqlite3') as db:
 assert db.execute('PRAGMA user_version').fetchone()[0]==6
 assert db.execute('SELECT COUNT(*) FROM review_runs').fetchone()[0]==5
 assert db.execute('SELECT COUNT(*) FROM folder_intake').fetchone()[0]==2
 assert db.execute('SELECT COUNT(*) FROM review_actions').fetchone()[0]==3
'@ | python -
if ($LASTEXITCODE -ne 0) { throw 'Folder intake created a review or lost existing history.' }
# Keep the earlier fixed-count persistence checks above independent of these new reviews.
$silCases = Get-Content -LiteralPath (Join-Path $installDirectory 'Samples/expected-sil.json') -Raw | ConvertFrom-Json -AsHashtable
$silResults = @()
foreach ($name in $silCases.Keys | Sort-Object) {
    $sample = Join-Path $installDirectory "Samples/$name"
    $before = (Get-FileHash -LiteralPath $sample -Algorithm SHA256).Hash
    $silContext = Join-Path $fixtureRoot ($name + '.context.json')
    $silCases[$name].context | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $silContext -Encoding utf8NoBOM
    $output = Join-Path $fixtureRoot ($name + '.sil.json')
    Invoke-ReviewDiagnostic @('--check-file',$sample,'--context',$silContext,'--output',$output)
    $saved = Get-Content -LiteralPath $output -Raw | ConvertFrom-Json
    $finding = $saved.review.findings[5]
    if ($saved.review.findings.Count -ne 20 -or $finding.rule_id -ne 'PH-LAB-095-01' -or $finding.outcome -ne $silCases[$name].outcome) { throw "Installed SIL outcome mismatch: $name" }
    if ($saved.review.context.service_incentive_leave.basis -ne $silCases[$name].context.service_incentive_leave.basis -or $finding.article -notlike '*Article 95*') { throw 'SIL eligibility/citation audit data missing.' }
    if ((Get-FileHash -LiteralPath $sample -Algorithm SHA256).Hash -ne $before) { throw 'SIL review changed an original.' }
    $silResults += [ordered]@{ name=$name; outcome=$finding.outcome; original_preserved=$true }
    if ($finding.outcome -eq 'potential_issue') { $silIssueId = $saved.id }
}
# Omitted/unknown facts and an empty basis must not inherit coverage from other rules.
foreach ($dimension in @('employee_coverage','qualifying_service','exceptions','basis')) {
    $gate = $silCases['Synthetic SIL issues.docx'].context | ConvertTo-Json -Depth 6 | ConvertFrom-Json -AsHashtable
    $gate.service_incentive_leave[$dimension] = if ($dimension -eq 'basis') { '' } else { 'unknown' }
    $gateFile = Join-Path $fixtureRoot ("sil-gate-$dimension.context.json")
    $gate | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $gateFile -Encoding utf8NoBOM
    $output = Join-Path $fixtureRoot ("sil-gate-$dimension.json")
    Invoke-ReviewDiagnostic @('--check-file',(Join-Path $installDirectory 'Samples/Synthetic SIL issues.docx'),'--context',$gateFile,'--output',$output)
    $saved = Get-Content -LiteralPath $output -Raw | ConvertFrom-Json
    if ($saved.review.findings[5].outcome -ne 'needs_context') { throw "SIL eligibility gate was bypassed: $dimension" }
}
$silPdf = Join-Path $fixtureRoot 'synthetic-sil-review.pdf'
Invoke-ReviewDiagnostic @('--export-review-pdf',"$silIssueId",'--output',$silPdf)
$silReading = Join-Path $fixtureRoot 'sil-pdf-reading.json'
Invoke-ReviewDiagnostic @('--read-file',$silPdf,'--output',$silReading)
$silRead = Get-Content -LiteralPath $silReading -Raw | ConvertFrom-Json
$text = (($silRead.report.units | ForEach-Object { $_.text }) -join ' ') -replace '\s+', ' '
foreach ($needle in @('PH-LAB-095-01','Article 95','five days with pay','At least one qualifying year confirmed','regularly employs 10 workers','Potential issue')) {
    if (!$text.Contains($needle)) { throw "Missing SIL PDF content: $needle" }
}
# Exercise the initial seven-domain pack through the installed executable.
$initial = Get-Content -LiteralPath (Join-Path $installDirectory 'Samples/expected-initial.json') -Raw | ConvertFrom-Json -AsHashtable
$initialResults = @()
$expectedPackHash = (Get-FileHash -LiteralPath (Join-Path $root 'crates/checks/src/initial_rules.json') -Algorithm SHA256).Hash.ToLowerInvariant()
foreach ($name in $initial.cases.Keys | Sort-Object) {
    $case = $initial.cases[$name]
    $sample = Join-Path $installDirectory "Samples/$name"
    $before = (Get-FileHash -LiteralPath $sample -Algorithm SHA256).Hash
    $context = Join-Path $fixtureRoot ($name + '.initial-context.json')
    $case.context | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $context -Encoding utf8NoBOM
    $output = Join-Path $fixtureRoot ($name + '.initial-review.json')
    Invoke-ReviewDiagnostic @('--check-file',$sample,'--context',$context,'--output',$output)
    $saved = Get-Content -LiteralPath $output -Raw | ConvertFrom-Json
    $outcomes = @($saved.review.findings | ForEach-Object { $_.outcome })
    if (($outcomes -join ',') -ne ($case.outcomes -join ',') -or $saved.review.rule_pack_sha256 -ne $expectedPackHash) { throw "Initial rule-pack mismatch: $name" }
    foreach ($finding in $saved.review.findings[6..19]) {
        if (!$finding.source.key -or $finding.source.sha256.Length -ne 64 -or $finding.source.text_sha256.Length -ne 64 -or !$finding.provision.start -or !$finding.provision.end) { throw 'Finding lost its exact source or provision boundary.' }
    }
    if ((Get-FileHash -LiteralPath $sample -Algorithm SHA256).Hash -ne $before) { throw 'Initial review changed an original.' }
    $initialResults += [ordered]@{ name=$name; outcomes=$outcomes; original_preserved=$true }
    if ($name -eq 'Synthetic seven-domain issues.docx') { $initialIssueId = $saved.id }
}
foreach ($gateName in @('coverage','exceptions','date','basis','period')) {
    $gate = $initial.cases['Synthetic seven-domain baseline.docx'].context | ConvertTo-Json -Depth 8 | ConvertFrom-Json -AsHashtable
    foreach ($id in $gate.additional_checks.Keys) {
        if ($id -eq 'PH-HDMF-EE-LOW-01') { continue }
        $scope = $gate.additional_checks[$id]
        switch ($gateName) {
            'coverage' { $scope.coverage = 'unknown' }
            'exceptions' { $scope.exceptions_reviewed = $false }
            'date' { $scope.review_on = '2026-02-30' }
            'basis' { $scope.basis = '' }
            'period' { $scope.review_on = '2027-01-01' }
        }
    }
    $context = Join-Path $fixtureRoot "initial-gate-$gateName.context.json"
    $gate | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $context -Encoding utf8NoBOM
    $output = Join-Path $fixtureRoot "initial-gate-$gateName.json"
    Invoke-ReviewDiagnostic @('--check-file',(Join-Path $installDirectory 'Samples/Synthetic seven-domain baseline.docx'),'--context',$context,'--output',$output)
    $saved = Get-Content -LiteralPath $output -Raw | ConvertFrom-Json
    $expectedOutcome = if ($gateName -eq 'period') { 'needs_review' } else { 'needs_context' }
    if (@($saved.review.findings[6..19] | Where-Object { $_.rule_id -ne 'PH-HDMF-EE-LOW-01' -and $_.outcome -ne $expectedOutcome }).Count -gt 0) { throw "Initial scope gate bypassed: $gateName" }
}
$initialPdf = Join-Path $fixtureRoot 'synthetic-initial-review.pdf'
Invoke-ReviewDiagnostic @('--export-review-pdf',"$initialIssueId",'--output',$initialPdf)
$initialReadFile = Join-Path $fixtureRoot 'initial-pdf-reading.json'
Invoke-ReviewDiagnostic @('--read-file',$initialPdf,'--output',$initialReadFile)
$initialRead = Get-Content -LiteralPath $initialReadFile -Raw | ConvertFrom-Json
$text = (($initialRead.report.units | ForEach-Object { $_.text }) -join ' ') -replace '\s+', ' '
foreach ($needle in @('PH-OSH-016-01','PH-NPC-038-01','PH-BIR-024-01','PH-SSS-TOTAL-01','PH-PHIC-RATE-01','PH-HDMF-ER-01','2026-11-01','Synthetic assertions only',$expectedPackHash)) {
    if (!$text.Contains($needle)) { throw "Missing initial-pack PDF content: $needle" }
}
$privateHash = (Get-FileHash -LiteralPath $privateDatabase -Algorithm SHA256).Hash
# Import a fictional public reference through the installed bounded reader.
$sourceSample = Join-Path $installDirectory 'Samples/Synthetic employment.pdf'
$sourceMetadata = Join-Path $installDirectory 'Samples/synthetic-source.json'
$sourceHash = (Get-FileHash -LiteralPath $sourceSample -Algorithm SHA256).Hash
$importResults = @()
foreach ($name in @('source-import.json','source-duplicate.json')) {
    $output = Join-Path $fixtureRoot $name
    $info = [Diagnostics.ProcessStartInfo]::new(); $info.FileName = $application; $info.UseShellExecute = $false
    foreach ($argument in @('--import-legal-source',$sourceSample,'--metadata',$sourceMetadata,'--output',$output,'--library-dir',(Join-Path $machineData 'library'))) { $info.ArgumentList.Add($argument) }
    $intake = [Diagnostics.Process]::Start($info)
    if (!$intake.WaitForExit(45000)) { $intake.Kill($true); throw 'Installed source import timed out.' }
    if ($intake.ExitCode -ne 0) { throw 'Installed source import failed.' }
    $importResults += Get-Content -LiteralPath $output -Raw | ConvertFrom-Json
}
if (!$importResults[0].added -or $importResults[1].added -or $importResults[0].source_id -ne $importResults[1].source_id) { throw 'Source duplicate handling failed.' }
if ($importResults[0].metadata.provenance -ne 'manual_pending' -or $importResults[0].metadata.verified_at_unix -ne 0) { throw 'Manual import was incorrectly promoted to verified.' }
$manualStatus = (& (Join-Path $root 'target/release/complyph-updater.exe') '--status') | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or $manualStatus.source_count -ne 20 -or $manualStatus.verified_source_count -ne 14 -or $manualStatus.manually_imported_source_count -ne 1 -or $manualStatus.awaiting_verification_count -ne 1 -or $manualStatus.user_checked_source_count -ne 0 -or $null -ne $manualStatus.last_successful_check) { throw 'Imported source status was not retained.' }
@'
import os,pathlib,sqlite3
with sqlite3.connect(pathlib.Path(os.environ['COMPLYPH_SMOKE_ACTIVE'])/'library/catalog.sqlite3') as db:
 assert db.execute('PRAGMA user_version').fetchone()[0]==5
 assert db.execute('SELECT COUNT(*) FROM source_versions').fetchone()[0]==20
'@ | python -
if ($LASTEXITCODE -ne 0) { throw 'Public source migration or duplicate persistence failed.' }
if ((Get-FileHash -LiteralPath $privateDatabase -Algorithm SHA256).Hash -ne $privateHash) { throw 'Reference import changed the private workspace.' }
if ((Get-FileHash -LiteralPath $sourceSample -Algorithm SHA256).Hash -ne $sourceHash) { throw 'Reference import changed the original.' }
$maintenanceOutput = Join-Path $fixtureRoot 'library-maintenance.json'
& python (Join-Path $root 'scripts/test-library-maintenance.py') '--app' $application '--samples' (Join-Path $installDirectory 'Samples') '--work' (Join-Path $fixtureRoot 'library-maintenance') '--output' $maintenanceOutput
if ($LASTEXITCODE -ne 0) { throw 'Installed library maintenance failed.' }
$maintenance = Get-Content -LiteralPath $maintenanceOutput -Raw | ConvertFrom-Json
if ($maintenance.public_schema -ne 5 -or $maintenance.workflow_cases.Count -ne 6 -or @($maintenance.workflow_cases | Where-Object { !$_.passed }).Count -gt 0) { throw 'Incomplete maintenance workflow evidence.' }
foreach ($check in $maintenance.checks.PSObject.Properties) { if ($check.Value -ne $true) { throw "Failed maintenance check: $($check.Name)" } }
if ((Get-FileHash -LiteralPath $privateDatabase -Algorithm SHA256).Hash -ne $privateHash) { throw 'Library maintenance changed the default private workspace.' }
$revisionsOutput = Join-Path $fixtureRoot 'document-revisions.json'
& python (Join-Path $root 'scripts/test-document-revisions.py') '--app' $application '--samples' (Join-Path $installDirectory 'Samples') '--work' (Join-Path $fixtureRoot 'document-revisions') '--output' $revisionsOutput
if ($LASTEXITCODE -ne 0) { throw 'Installed document revision workflows failed.' }
$revisions = Get-Content -LiteralPath $revisionsOutput -Raw | ConvertFrom-Json
if ($revisions.private_schema -ne 6 -or $revisions.public_schema -ne 5 -or $revisions.workflow_cases.Count -ne 6) { throw 'Incomplete document revision evidence.' }
foreach ($case in $revisions.workflow_cases) { if ($case.passed -ne $true) { throw "Failed revision workflow: $($case.name)" } }
foreach ($check in $revisions.checks.PSObject.Properties) { if ($check.Value -ne $true) { throw "Failed revision check: $($check.Name)" } }
if ((Get-FileHash -LiteralPath $privateDatabase -Algorithm SHA256).Hash -ne $privateHash) { throw 'Revision tests changed the default private workspace.' }
$qualityOutput = Join-Path $fixtureRoot 'reading-quality.json'
& python (Join-Path $root 'scripts/test-reading-quality.py') '--app' $application '--samples' (Join-Path $installDirectory 'Samples') '--work' (Join-Path $fixtureRoot 'reading-quality') '--output' $qualityOutput
if ($LASTEXITCODE -ne 0) { throw 'Installed reading-quality workflows failed.' }
$quality = Get-Content -LiteralPath $qualityOutput -Raw | ConvertFrom-Json
if ($quality.private_schema -ne 6 -or $quality.public_schema -ne 5 -or $quality.workflow_cases.Count -ne 6 -or $quality.reader_results.Count -ne 8 -or $quality.ocr_results.Count -ne 6) { throw 'Incomplete reading-quality evidence.' }
foreach ($case in $quality.workflow_cases) { if ($case.passed -ne $true) { throw "Failed quality workflow: $($case.name)" } }
foreach ($check in $quality.checks.PSObject.Properties) { if ($check.Value -ne $true) { throw "Failed reading-quality check: $($check.Name)" } }
if ((Get-FileHash -LiteralPath $privateDatabase -Algorithm SHA256).Hash -ne $privateHash) { throw 'Reading-quality tests changed the default private workspace.' }
$desktopOutput = Join-Path $fixtureRoot 'desktop-quality.json'
$desktopRoot = Join-Path $fixtureRoot 'desktop-quality'
$desktopStart = [Diagnostics.ProcessStartInfo]::new()
$desktopStart.FileName = $application
$desktopStart.UseShellExecute = $false
$desktopStart.RedirectStandardError = $true
foreach ($argument in @('--desktop-quality-check','--output',$desktopOutput,'--data-dir',$desktopRoot)) { $desktopStart.ArgumentList.Add($argument) }
$desktopProcess = [Diagnostics.Process]::Start($desktopStart)
$desktopErrors = $desktopProcess.StandardError.ReadToEndAsync()
if (!$desktopProcess.WaitForExit(300000)) { $desktopProcess.Kill($true); throw 'Installed desktop-quality diagnostic timed out.' }
if ($desktopProcess.ExitCode -ne 0) { throw ('Installed desktop-quality workflows failed: ' + $desktopErrors.GetAwaiter().GetResult()) }
$desktopQuality = Get-Content -LiteralPath $desktopOutput -Raw | ConvertFrom-Json
if ($desktopQuality.profile -ne 'initial-desktop-quality-v1' -or $desktopQuality.version -ne '0.15.0' -or $desktopQuality.layout_cases.Count -ne 12 -or $desktopQuality.keyboard_cases.Count -ne 5 -or $desktopQuality.recovery_cases.Count -ne 3) { throw 'Incomplete desktop-quality evidence.' }
foreach ($case in $desktopQuality.layout_cases) { if ($case.passed -ne $true -or $case.scenarios -ne 14) { throw 'A desktop layout scenario failed.' } }
foreach ($case in @($desktopQuality.keyboard_cases) + @($desktopQuality.recovery_cases)) { if ($case.passed -ne $true) { throw "Failed desktop workflow: $($case.name)" } }
if ($desktopQuality.interactive_windows_display_tested -ne $false -or $desktopQuality.qualified_legal_review -ne $false) { throw 'Desktop evidence misrepresents its verification scope.' }
if (!(Test-Path -LiteralPath (Join-Path $installDirectory 'Desktop-quality.md'))) { throw 'Desktop controls guide is missing.' }
if ((Get-FileHash -LiteralPath $privateDatabase -Algorithm SHA256).Hash -ne $privateHash) { throw 'Desktop-quality tests changed the default private workspace.' }
$catalogHash = (Get-FileHash -LiteralPath $catalog -Algorithm SHA256).Hash
$reinstall = Start-Process -FilePath $installer -ArgumentList '/S' -Wait -PassThru
if ($reinstall.ExitCode -ne 0 -or !(Test-Path -LiteralPath $application)) { throw 'Reinstallation lost the registered installation location.' }
if (Test-Path -LiteralPath (Join-Path $env:LOCALAPPDATA 'Programs/ComplyPH/ComplyPH.exe')) { throw 'Reinstallation created a duplicate app installation.' }
$uninstall = Start-Process -FilePath (Join-Path $installDirectory 'Uninstall.exe') -ArgumentList '/S' -Wait -PassThru
if ($uninstall.ExitCode -ne 0) { throw 'Uninstallation failed.' }
$deadline = [DateTime]::UtcNow.AddSeconds(15)
while ((Test-Path -LiteralPath $application) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 250 }
if (Test-Path -LiteralPath $application) { throw 'Uninstallation left the executable.' }
$freshDirectory = Join-Path $fixtureRoot 'fresh-app'
$fresh = Start-Process -FilePath $installer -ArgumentList '/S', "/D=$freshDirectory" -Wait -PassThru
if ($fresh.ExitCode -ne 0 -or !(Test-Path -LiteralPath (Join-Path $freshDirectory 'ComplyPH.exe'))) { throw 'Fresh installation failed.' }
$freshRemove = Start-Process -FilePath (Join-Path $freshDirectory 'Uninstall.exe') -ArgumentList '/S' -Wait -PassThru
if ($freshRemove.ExitCode -ne 0) { throw 'Fresh uninstallation failed.' }
$deadline = [DateTime]::UtcNow.AddSeconds(15)
while ((Test-Path -LiteralPath (Join-Path $freshDirectory 'ComplyPH.exe')) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 250 }
if (Test-Path -LiteralPath (Join-Path $freshDirectory 'ComplyPH.exe')) { throw 'Fresh application was not removed.' }
if (!(Test-Path -LiteralPath $sentinel)) { throw 'Workspace fixture removed.' }
if ((Get-FileHash -LiteralPath $privateDatabase -Algorithm SHA256).Hash -ne $privateHash) { throw 'Private workspace changed.' }
if ((Get-FileHash -LiteralPath $catalog -Algorithm SHA256).Hash -ne $catalogHash) { throw 'Uninstall changed the reference library.' }
if ((Get-FileHash -LiteralPath $original -Algorithm SHA256).Hash -ne $originalHash) { throw 'Original document changed.' }
foreach ($path in @((Join-Path $desktop 'ComplyPH.lnk'), (Join-Path $programs 'ComplyPH.lnk'))) {
    if (Test-Path -LiteralPath $path) { throw "Uninstallation left a shortcut: $path" }
}
# Upgrade a real workspace created and annotated by the released ComplyPH 0.12.0 binary.
$currentInstaller = Join-Path $fixtureRoot 'ComplyPH-Setup-0.12.0.exe'
Invoke-WebRequest -Uri 'https://github.com/tatselkrik/complyph/releases/download/v0.12.0/ComplyPH-Setup-0.12.0.exe' -OutFile $currentInstaller
if ((Get-FileHash -LiteralPath $currentInstaller -Algorithm SHA256).Hash.ToLowerInvariant() -ne 'e604c6c9f2b79ffde613ea1e7fed1a753383611b9cc4a58733868d9365436589') { throw 'Previous ComplyPH installer hash mismatch.' }
$previous = Start-Process -FilePath $currentInstaller -ArgumentList '/S', "/D=$freshDirectory" -Wait -PassThru
if ($previous.ExitCode -ne 0) { throw 'Previous ComplyPH fixture installation failed.' }
$upgradeApplication = Join-Path $freshDirectory 'ComplyPH.exe'
function Invoke-UpgradeDiagnostic([string[]]$Arguments) {
    $info = [Diagnostics.ProcessStartInfo]::new(); $info.FileName = $upgradeApplication; $info.UseShellExecute = $false
    foreach ($argument in $Arguments) { $info.ArgumentList.Add($argument) }
    $process = [Diagnostics.Process]::Start($info)
    if (!$process.WaitForExit(60000)) { $process.Kill($true); throw 'Released workspace upgrade diagnostic timed out.' }
    if ($process.ExitCode -ne 0) { throw 'Released workspace upgrade diagnostic failed.' }
}
$releasedData = Join-Path $fixtureRoot 'released-v012-data'
$releasedOriginal = Join-Path $fixtureRoot 'Released earlier.docx'
Copy-Item -LiteralPath (Join-Path $root 'examples/Synthetic revision original.docx') -Destination $releasedOriginal
$releasedScope = Join-Path $fixtureRoot 'released-scope.json'
((Get-Content -LiteralPath (Join-Path $root 'examples/expected-revisions.json') -Raw | ConvertFrom-Json).context | ConvertTo-Json -Depth 6) | Set-Content -LiteralPath $releasedScope -Encoding utf8NoBOM
$releasedReview = Join-Path $fixtureRoot 'released-review.json'
Invoke-UpgradeDiagnostic @('--check-file',$releasedOriginal,'--context',$releasedScope,'--output',$releasedReview,'--data-dir',$releasedData)
$released = Get-Content -LiteralPath $releasedReview -Raw | ConvertFrom-Json
$releasedStateFile = Join-Path $fixtureRoot 'released-state.json'
Invoke-UpgradeDiagnostic @('--review-state',"$($released.id)",'--output',$releasedStateFile,'--data-dir',$releasedData)
$releasedState = Get-Content -LiteralPath $releasedStateFile -Raw | ConvertFrom-Json
$releasedAction = Join-Path $fixtureRoot 'released-action.json'
@{ review_fingerprint=$releasedState.review_fingerprint; rule_id='PH-LAB-083-01'; expected_revision=0; reviewer='Synthetic upgrade reviewer'; note='Preserve this released-version resolution.'; status='resolved' } | ConvertTo-Json | Set-Content -LiteralPath $releasedAction -Encoding utf8NoBOM
Invoke-UpgradeDiagnostic @('--review-action',"$($released.id)",'--metadata',$releasedAction,'--output',(Join-Path $fixtureRoot 'released-noted.json'),'--data-dir',$releasedData)
$env:COMPLYPH_RELEASED_DATA = $releasedData
$env:COMPLYPH_RELEASED_BEFORE = Join-Path $fixtureRoot 'released-before.json'
@'
import os,pathlib,sqlite3,json
with sqlite3.connect(pathlib.Path(os.environ['COMPLYPH_RELEASED_DATA'])/'private/workspace.sqlite3') as db:
 assert db.execute('PRAGMA user_version').fetchone()[0]==5
 before={table:db.execute('SELECT * FROM '+table+' ORDER BY 1').fetchall() for table in ['documents','document_extractions','review_runs','review_actions']}
 pathlib.Path(os.environ['COMPLYPH_RELEASED_BEFORE']).write_text(json.dumps(before),encoding='utf-8')
'@ | python -
if ($LASTEXITCODE -ne 0) { throw 'Could not capture a real released schema-5 workspace.' }
$releasedPublicHash = (Get-FileHash -LiteralPath (Join-Path $releasedData 'library/catalog.sqlite3') -Algorithm SHA256).Hash
$upgrade = Start-Process -FilePath $installer -ArgumentList '/S' -Wait -PassThru
if ($upgrade.ExitCode -ne 0 -or !(Test-Path -LiteralPath (Join-Path $freshDirectory 'Samples/Synthetic revision corrected.docx'))) { throw 'ComplyPH 0.12.0 upgrade failed.' }
$upgradeRequest = Join-Path $fixtureRoot 'released-history-request.json'
@{ action='history'; document_id=$released.document_id } | ConvertTo-Json | Set-Content -LiteralPath $upgradeRequest -Encoding utf8NoBOM
Invoke-UpgradeDiagnostic @('--document-action','--metadata',$upgradeRequest,'--output',(Join-Path $fixtureRoot 'released-migrated-history.json'),'--data-dir',$releasedData)
Invoke-UpgradeDiagnostic @('--review-state',"$($released.id)",'--output',(Join-Path $fixtureRoot 'released-migrated-state.json'),'--data-dir',$releasedData)
@'
import os,pathlib,sqlite3,json
before=json.loads(pathlib.Path(os.environ['COMPLYPH_RELEASED_BEFORE']).read_text(encoding='utf-8'))
with sqlite3.connect(pathlib.Path(os.environ['COMPLYPH_RELEASED_DATA'])/'private/workspace.sqlite3') as db:
 assert db.execute('PRAGMA user_version').fetchone()[0]==6
 for table,expected in before.items(): assert [list(r) for r in db.execute('SELECT * FROM '+table+' ORDER BY 1')]==expected,table
 assert db.execute('SELECT COUNT(*) FROM document_readings').fetchone()[0]==1
 assert db.execute('SELECT COUNT(*) FROM review_version_links WHERE reading_id IS NOT NULL').fetchone()[0]==0
'@ | python -
if ($LASTEXITCODE -ne 0) { throw 'Released schema-5 upgrade lost original readings, reports or reviewer history.' }
if ((Get-FileHash -LiteralPath (Join-Path $releasedData 'library/catalog.sqlite3') -Algorithm SHA256).Hash -ne $releasedPublicHash) { throw 'Private revision migration changed the released legal catalog.' }
if ((Get-FileHash -LiteralPath $releasedOriginal -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath (Join-Path $root 'examples/Synthetic revision original.docx') -Algorithm SHA256).Hash) { throw 'Released workspace migration changed an original.' }
$cleanup = Start-Process -FilePath (Join-Path $freshDirectory 'Uninstall.exe') -ArgumentList '/S' -Wait -PassThru
if ($cleanup.ExitCode -ne 0) { throw 'ComplyPH upgrade fixture cleanup failed.' }
$deadline = [DateTime]::UtcNow.AddSeconds(15)
while ((Test-Path -LiteralPath (Join-Path $freshDirectory 'ComplyPH.exe')) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 250 }
if (Test-Path -LiteralPath (Join-Path $freshDirectory 'ComplyPH.exe')) { throw 'ComplyPH upgrade fixture was not removed.' }
if ((Get-FileHash -LiteralPath $privateDatabase -Algorithm SHA256).Hash -ne $privateHash) { throw 'Current-version upgrade changed private data.' }
if ((Get-FileHash -LiteralPath $catalog -Algorithm SHA256).Hash -ne $catalogHash) { throw 'Current-version upgrade changed public data.' }
$readingUpgradeOutput = Join-Path $fixtureRoot 'released-v013-reading-upgrade.json'
& (Join-Path $root 'scripts/test-reading-upgrade.ps1') -Installer $installer -FixtureRoot $fixtureRoot -InstallDirectory $freshDirectory -OutputPath $readingUpgradeOutput
$readingUpgrade = Get-Content -LiteralPath $readingUpgradeOutput -Raw | ConvertFrom-Json
if ($readingUpgrade.passed -ne $true) { throw 'Released reading-history upgrade failed.' }
if ((Get-FileHash -LiteralPath $privateDatabase -Algorithm SHA256).Hash -ne $privateHash -or (Get-FileHash -LiteralPath $catalog -Algorithm SHA256).Hash -ne $catalogHash) { throw 'Reading-history upgrade changed the default workspace.' }
$result = [ordered]@{
    source_commit = (git -C $root rev-parse HEAD).Trim()
    installed = $true; fresh_install_passed = $true; legacy_upgrade_passed = $true
    reinstall_location_preserved = $true
    legacy_branding_replaced = $true; legacy_data_migrated = $true; programdata_acl_passed = $true
    shortcuts_verified = $true; cli_startup_passed = $true
    catalog_status_passed = $true; starter_source_count = $status.source_count; starter_domain_count = $status.domains.Count
    uninstalled = $true; private_fixture_preserved = $true; catalog_preserved = $true; original_preserved = $true
    installed_reader_passed = $true; private_schema_upgrade_passed = $true; current_version_upgrade_passed = $true
    private_schema = 6; validated_check_count = 20; installed_checks_passed = $true; synthetic_check_fixture_count = $checkResults.Count; check_results = $checkResults
    installed_document_revisions_passed = $true; released_v012_history_upgrade_passed = $true; document_revisions = $revisions
    installed_reading_quality_passed = $true; reading_quality = $quality; released_v013_reading_upgrade_passed = $true; reading_upgrade = $readingUpgrade
    installed_desktop_quality_passed = $true; desktop_quality = $desktopQuality
    installed_initial_checks_passed = $true; initial_period_scope_gates_passed = $true; installed_initial_source_citations_passed = $true; initial_pdf_sources_passed = $true; synthetic_initial_fixture_count = $initialResults.Count; initial_results = $initialResults; rule_pack_sha256 = $expectedPackHash
    installed_sil_checks_passed = $true; sil_eligibility_gates_passed = $true; sil_pdf_basis_passed = $true; synthetic_sil_fixture_count = $silResults.Count; sil_results = $silResults
    installed_ocr_passed = $true; ocr_confirmation_gate_passed = $true; ocr_originals_preserved = $true; synthetic_ocr_fixture_count = $ocrResults.Count; ocr_results = $ocrResults
    watched_folder_intake_passed = $true; no_auto_review_from_watch_passed = $true
    installed_review_notes_passed = $true; installed_issue_tracking_passed = $true; installed_pdf_export_passed = $true; review_audit_history_preserved = $true
    public_schema = 5; installed_source_import_passed = $true; manual_source_count = $manualStatus.manually_imported_source_count; awaiting_verification_count = $manualStatus.awaiting_verification_count
    installed_library_maintenance_passed = $true; library_maintenance = $maintenance
    downloader_isolation_passed = $true; updater_no_approval_no_network_passed = $true
    public_https_probe_passed = $liveResponseObserved; public_https_probe_results = $liveProbeResults
    synthetic_fixture_count = $readerResults.Count; reader_results = $readerResults
    interactive_windows_ui_tested = $false
}
$result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $root 'dist/windows/windows-install-check.json') -Encoding utf8
Write-Output 'Install/upgrades, ProgramData copy and ACL, reader/check cases, public source import, reviewer history, offline PDF export, and preservation passed.'
