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
Invoke-WebRequest -Uri 'https://github.com/tatselkrik/laborlens-ph/releases/download/v0.1.0/LaborLensPH-Setup-0.1.0.exe' -OutFile $legacyInstaller
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
$installer = Join-Path $root 'dist/windows/ComplyPH-Setup-0.2.0.exe'
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
$result = [ordered]@{
    source_commit = (git -C $root rev-parse HEAD).Trim()
    installed = $true; fresh_install_passed = $true; legacy_upgrade_passed = $true
    reinstall_location_preserved = $true
    legacy_branding_replaced = $true; legacy_data_root_reused = $true
    shortcuts_verified = $true; cli_startup_passed = $true
    catalog_status_passed = $true; starter_source_count = $status.source_count; starter_domain_count = $status.domains.Count
    uninstalled = $true; private_fixture_preserved = $true; catalog_preserved = $true; original_preserved = $true
    interactive_windows_ui_tested = $false
}
$result | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'dist/windows/windows-install-check.json') -Encoding utf8
Write-Output 'Fresh installation, legacy upgrade, seven-domain library import, shortcuts, and data preservation passed.'
