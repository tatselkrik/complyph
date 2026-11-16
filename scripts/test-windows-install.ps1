$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or !$env:RUNNER_TEMP) {
    throw 'This installer check requires an isolated GitHub Actions runner.'
}
$root = Split-Path -Parent $PSScriptRoot
$fixtureRoot = Join-Path $env:RUNNER_TEMP 'complyph-install-smoke'
$installDirectory = Join-Path $fixtureRoot 'upgraded-app'
$legacyData = Join-Path $env:LOCALAPPDATA 'LaborLens PH'
$currentData = Join-Path $env:LOCALAPPDATA 'ComplyPH'
foreach ($path in @($legacyData, $currentData)) {
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
Copy-Item -LiteralPath (Join-Path $root 'examples/Synthetic sample.docx') -Destination $original
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
$installer = Join-Path $root 'dist/windows/ComplyPH-Setup-0.3.0.exe'
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
$statusJson = & (Join-Path $root 'target/release/complyph-updater.exe') '--status'
if ($LASTEXITCODE -ne 0) { throw 'Default library discovery or import failed.' }
$status = $statusJson | ConvertFrom-Json
if ($status.source_count -ne 14 -or $status.verified_source_count -ne 14 -or $status.domains.Count -ne 7 -or $null -eq $status.bundled_verified_at -or $null -ne $status.last_successful_check -or $status.automatic_updates_available) {
    throw 'Unexpected starter-library or update status.'
}
if (@($status.domains | Where-Object { $_.source_count -lt 1 }).Count -gt 0) { throw 'A domain has no source snapshots.' }
if (Test-Path -LiteralPath $currentData) { throw 'Upgrade created a second data root.' }
if ((Get-FileHash -LiteralPath $privateDatabase -Algorithm SHA256).Hash -ne $privateHash) { throw 'Public catalog import changed private data.' }
$catalog = Join-Path $legacyData 'library/catalog.sqlite3'
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
root = pathlib.Path(os.environ['COMPLYPH_SMOKE_LEGACY'])
with sqlite3.connect(root / 'private/workspace.sqlite3') as db:
    assert db.execute('PRAGMA user_version').fetchone()[0] == 2
    original = db.execute('SELECT id,path,added_at FROM documents WHERE id=1').fetchone()
    assert original == (1, os.environ['COMPLYPH_SMOKE_ORIGINAL'], 1)
    prefs = json.loads(db.execute('SELECT value FROM preferences WHERE id=1').fetchone()[0])
    assert prefs == {'remember_page':True, 'last_page':'documents', 'show_full_paths':True}
    assert db.execute('SELECT COUNT(*) FROM document_extractions').fetchone()[0] == 7
'@ | python -
if ($LASTEXITCODE -ne 0) { throw 'Private schema upgrade lost existing records or extraction caches.' }
if (Test-Path -LiteralPath $currentData) { throw 'Reader created a duplicate data root.' }
$privateHash = (Get-FileHash -LiteralPath $privateDatabase -Algorithm SHA256).Hash
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
# Upgrade the already released ComplyPH 0.2.0 installer in its registered directory.
$currentInstaller = Join-Path $fixtureRoot 'ComplyPH-Setup-0.2.0.exe'
Invoke-WebRequest -Uri 'https://github.com/tatselkrik/complyph/releases/download/v0.2.0/ComplyPH-Setup-0.2.0.exe' -OutFile $currentInstaller
if ((Get-FileHash -LiteralPath $currentInstaller -Algorithm SHA256).Hash.ToLowerInvariant() -ne 'd61a4a93f2f15562b8e1dabbffe5dafc2b4e59c0c515ba521ce000879e0fed56') { throw 'Previous ComplyPH installer hash mismatch.' }
$previous = Start-Process -FilePath $currentInstaller -ArgumentList '/S', "/D=$freshDirectory" -Wait -PassThru
if ($previous.ExitCode -ne 0) { throw 'Previous ComplyPH fixture installation failed.' }
$upgrade = Start-Process -FilePath $installer -ArgumentList '/S' -Wait -PassThru
if ($upgrade.ExitCode -ne 0 -or !(Test-Path -LiteralPath (Join-Path $freshDirectory 'Samples/Synthetic employment.pdf'))) { throw 'ComplyPH 0.2.0 upgrade failed.' }
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
    legacy_branding_replaced = $true; legacy_data_root_reused = $true
    shortcuts_verified = $true; cli_startup_passed = $true
    catalog_status_passed = $true; starter_source_count = $status.source_count; starter_domain_count = $status.domains.Count
    uninstalled = $true; private_fixture_preserved = $true; catalog_preserved = $true; original_preserved = $true
    installed_reader_passed = $true; private_schema_upgrade_passed = $true; current_version_upgrade_passed = $true
    private_schema = 2; synthetic_fixture_count = $readerResults.Count; reader_results = $readerResults
    interactive_windows_ui_tested = $false
}
$result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $root 'dist/windows/windows-install-check.json') -Encoding utf8
Write-Output 'Fresh installation, legacy and ComplyPH upgrades, seven synthetic reader cases, schema migration, shortcuts, and preservation passed.'
