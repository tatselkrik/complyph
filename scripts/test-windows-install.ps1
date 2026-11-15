$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or !$env:RUNNER_TEMP) {
    throw 'This installer smoke test is restricted to an isolated GitHub Actions runner.'
}
$root = Split-Path -Parent $PSScriptRoot
$fixtureRoot = Join-Path $env:RUNNER_TEMP 'laborlens-install-smoke'
$installDirectory = Join-Path $fixtureRoot 'app'
$dataDirectory = Join-Path $env:LOCALAPPDATA 'LaborLens PH'
if (Test-Path -LiteralPath $dataDirectory) { throw 'The runner already has a LaborLens PH workspace; refusing to use it.' }
$registryPath = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\LaborLensPH'
foreach ($view in @([Microsoft.Win32.RegistryView]::Registry32, [Microsoft.Win32.RegistryView]::Registry64)) {
    $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::CurrentUser, $view)
    try {
        $existing = $base.OpenSubKey($registryPath)
        if ($existing) { $existing.Dispose(); throw 'The runner already has a LaborLens PH installation.' }
    } finally { $base.Dispose() }
}
New-Item -ItemType Directory -Force -Path $fixtureRoot, (Join-Path $dataDirectory 'private') | Out-Null
$sentinel = Join-Path $dataDirectory 'private/preserve.fixture'
'Synthetic preservation fixture only.' | Set-Content -LiteralPath $sentinel
$original = Join-Path $fixtureRoot 'Synthetic original.docx'
Copy-Item -LiteralPath (Join-Path $root 'examples/Synthetic sample.docx') -Destination $original
$originalHash = (Get-FileHash -LiteralPath $original -Algorithm SHA256).Hash
$desktop = [Environment]::GetFolderPath('DesktopDirectory')
$programs = [Environment]::GetFolderPath('Programs')
New-Item -ItemType Directory -Force -Path $desktop, $programs | Out-Null
$installer = Join-Path $root 'dist/windows/LaborLensPH-Setup-0.1.0.exe'
$process = Start-Process -FilePath $installer -ArgumentList '/S', "/D=$installDirectory" -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "Installer exit code: $($process.ExitCode)" }
$application = Join-Path $installDirectory 'LaborLensPH.exe'
foreach ($path in @($application, (Join-Path $desktop 'LaborLens PH.lnk'), (Join-Path $programs 'LaborLens PH.lnk'))) {
    if (!(Test-Path -LiteralPath $path)) { throw "Missing installed file or shortcut: $path" }
}
$startup = Start-Process -FilePath $application -ArgumentList '--help' -Wait -PassThru
if ($startup.ExitCode -ne 0) { throw "Application loader/CLI startup failed: $($startup.ExitCode)" }
$libraryDirectory = Join-Path $dataDirectory 'library'
$statusJson = & (Join-Path $root 'target/release/laborlens-updater.exe') '--status' '--library-dir' $libraryDirectory
if ($LASTEXITCODE -ne 0) { throw 'Public catalog status failed.' }
$status = $statusJson | ConvertFrom-Json
if ($status.source_count -ne 0 -or $status.verified_source_count -ne 0 -or $null -ne $status.last_successful_check -or $status.automatic_updates_available) {
    throw 'A fresh catalog claimed sources, a successful check, or enabled updates.'
}
$catalog = Join-Path $libraryDirectory 'catalog.sqlite3'
$catalogHash = (Get-FileHash -LiteralPath $catalog -Algorithm SHA256).Hash
$uninstaller = Join-Path $installDirectory 'Uninstall.exe'
$uninstall = Start-Process -FilePath $uninstaller -ArgumentList '/S' -Wait -PassThru
if ($uninstall.ExitCode -ne 0) { throw "Uninstaller exit code: $($uninstall.ExitCode)" }
$deadline = [DateTime]::UtcNow.AddSeconds(15)
while ((Test-Path -LiteralPath $application) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 250 }
if (Test-Path -LiteralPath $application) { throw 'The installed application was not removed.' }
if (!(Test-Path -LiteralPath $sentinel)) { throw 'Uninstallation removed workspace data.' }
if ((Get-FileHash -LiteralPath $catalog -Algorithm SHA256).Hash -ne $catalogHash) { throw 'Uninstallation changed the source catalog.' }
if ((Get-FileHash -LiteralPath $original -Algorithm SHA256).Hash -ne $originalHash) { throw 'Uninstallation changed an original document.' }
foreach ($path in @((Join-Path $desktop 'LaborLens PH.lnk'), (Join-Path $programs 'LaborLens PH.lnk'))) {
    if (Test-Path -LiteralPath $path) { throw "Uninstallation left a shortcut: $path" }
}
$result = [ordered]@{
    source_commit = (git -C $root rev-parse HEAD).Trim()
    installed = $true; shortcuts_verified = $true; cli_startup_passed = $true
    catalog_status_passed = $true; uninstalled = $true
    private_fixture_preserved = $true; catalog_preserved = $true; original_preserved = $true
    interactive_windows_ui_tested = $false
}
$result | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'dist/windows/windows-install-check.json') -Encoding utf8
Write-Output 'Windows installation, startup, shortcuts, catalog status, and preservation checks passed.'
