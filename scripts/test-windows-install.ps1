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
$installer = Join-Path $root 'dist/windows/ComplyPH-Setup-0.7.0.exe'
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
    assert db.execute('PRAGMA user_version').fetchone()[0] == 3
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
    if (!$saved.review.context.operative_terms_confirmed -or $saved.review.engine -ne 'complyph-labor-2') { throw 'Review audit context missing.' }
    if ((Get-FileHash -LiteralPath $sample -Algorithm SHA256).Hash -ne $before) { throw 'Checking changed an original.' }
    $checkResults += [ordered]@{ name=$name; outcomes=$outcomes; original_preserved=$true }
}
@'
import os,pathlib,sqlite3
with sqlite3.connect(pathlib.Path(os.environ['COMPLYPH_SMOKE_ACTIVE'])/'private/workspace.sqlite3') as db:
 assert db.execute('SELECT COUNT(*) FROM review_runs').fetchone()[0]==3
 assert db.execute('SELECT COUNT(*) FROM document_extractions').fetchone()[0]==11
'@ | python -
if ($LASTEXITCODE -ne 0) { throw 'Saved report/extraction persistence failed.' }
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
 assert db.execute('PRAGMA user_version').fetchone()[0]==4
 assert db.execute('SELECT COUNT(*) FROM source_versions').fetchone()[0]==20
'@ | python -
if ($LASTEXITCODE -ne 0) { throw 'Public source migration or duplicate persistence failed.' }
if ((Get-FileHash -LiteralPath $privateDatabase -Algorithm SHA256).Hash -ne $privateHash) { throw 'Reference import changed the private workspace.' }
if ((Get-FileHash -LiteralPath $sourceSample -Algorithm SHA256).Hash -ne $sourceHash) { throw 'Reference import changed the original.' }
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
# Upgrade the already released ComplyPH 0.6.0 installer in its registered directory.
$currentInstaller = Join-Path $fixtureRoot 'ComplyPH-Setup-0.6.0.exe'
Invoke-WebRequest -Uri 'https://github.com/tatselkrik/complyph/releases/download/v0.6.0/ComplyPH-Setup-0.6.0.exe' -OutFile $currentInstaller
if ((Get-FileHash -LiteralPath $currentInstaller -Algorithm SHA256).Hash.ToLowerInvariant() -ne '1984ca3ea4720363a8a8759a79b47b5633e20d71149ed467d809e9301a7794cb') { throw 'Previous ComplyPH installer hash mismatch.' }
$previous = Start-Process -FilePath $currentInstaller -ArgumentList '/S', "/D=$freshDirectory" -Wait -PassThru
if ($previous.ExitCode -ne 0) { throw 'Previous ComplyPH fixture installation failed.' }
$upgrade = Start-Process -FilePath $installer -ArgumentList '/S' -Wait -PassThru
if ($upgrade.ExitCode -ne 0 -or !(Test-Path -LiteralPath (Join-Path $freshDirectory 'Samples/synthetic-source.json'))) { throw 'ComplyPH 0.6.0 upgrade failed.' }
$cleanup = Start-Process -FilePath (Join-Path $freshDirectory 'Uninstall.exe') -ArgumentList '/S' -Wait -PassThru
if ($cleanup.ExitCode -ne 0) { throw 'ComplyPH upgrade fixture cleanup failed.' }
$deadline = [DateTime]::UtcNow.AddSeconds(15)
while ((Test-Path -LiteralPath (Join-Path $freshDirectory 'ComplyPH.exe')) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 250 }
if (Test-Path -LiteralPath (Join-Path $freshDirectory 'ComplyPH.exe')) { throw 'ComplyPH upgrade fixture was not removed.' }
if ((Get-FileHash -LiteralPath $privateDatabase -Algorithm SHA256).Hash -ne $privateHash) { throw 'Current-version upgrade changed private data.' }
if ((Get-FileHash -LiteralPath $catalog -Algorithm SHA256).Hash -ne $catalogHash) { throw 'Current-version upgrade changed public data.' }
$result = [ordered]@{
    source_commit = (git -C $root rev-parse HEAD).Trim()
    installed = $true; fresh_install_passed = $true; legacy_upgrade_passed = $true
    reinstall_location_preserved = $true
    legacy_branding_replaced = $true; legacy_data_migrated = $true; programdata_acl_passed = $true
    shortcuts_verified = $true; cli_startup_passed = $true
    catalog_status_passed = $true; starter_source_count = $status.source_count; starter_domain_count = $status.domains.Count
    uninstalled = $true; private_fixture_preserved = $true; catalog_preserved = $true; original_preserved = $true
    installed_reader_passed = $true; private_schema_upgrade_passed = $true; current_version_upgrade_passed = $true
    private_schema = 3; validated_check_count = 5; installed_checks_passed = $true; synthetic_check_fixture_count = $checkResults.Count; check_results = $checkResults
    public_schema = 4; installed_source_import_passed = $true; manual_source_count = $manualStatus.manually_imported_source_count; awaiting_verification_count = $manualStatus.awaiting_verification_count
    downloader_isolation_passed = $true; updater_no_approval_no_network_passed = $true
    public_https_probe_passed = $liveResponseObserved; public_https_probe_results = $liveProbeResults
    synthetic_fixture_count = $readerResults.Count; reader_results = $readerResults
    interactive_windows_ui_tested = $false
}
$result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $root 'dist/windows/windows-install-check.json') -Encoding utf8
Write-Output 'Install/upgrades, ProgramData copy and ACL, reader/check cases, public source import, saved reports, and preservation passed.'
