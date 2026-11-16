param([string]$OutputDirectory = 'dist/windows')
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$output = [IO.Path]::GetFullPath((Join-Path $root $OutputDirectory))
$application = Join-Path $root 'target/release/complyph.exe'
if (!(Test-Path -LiteralPath $application)) { throw 'Build the Windows release with cargo first.' }
$makensis = 'C:\Program Files (x86)\NSIS\makensis.exe'
if (!(Test-Path -LiteralPath $makensis)) {
    $command = Get-Command makensis -ErrorAction SilentlyContinue
    if (!$command) { throw 'Install the free NSIS 3 packaging tools.' }
    $makensis = $command.Source
}
$version = '0.4.0'
$stage = Join-Path $root 'target/windows-package/ComplyPH'
if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
New-Item -ItemType Directory -Force -Path $output, $stage, (Join-Path $stage 'Samples') | Out-Null
Copy-Item -LiteralPath $application -Destination (Join-Path $stage 'ComplyPH.exe')
Copy-Item -LiteralPath (Join-Path $root 'assets/icon.ico') -Destination (Join-Path $stage 'ComplyPH.ico')
Copy-Item -LiteralPath (Join-Path $root 'LICENSE') -Destination (Join-Path $stage 'LICENSE.txt')
Copy-Item -LiteralPath (Join-Path $root 'assets/fonts/OFL.txt') -Destination (Join-Path $stage 'Lato-OFL.txt')
Copy-Item -LiteralPath (Join-Path $root 'packaging/windows/THIRD_PARTY_NOTICES.txt') -Destination $stage
Copy-Item -LiteralPath (Join-Path $root 'packaging/windows/Read-me.txt') -Destination $stage
Get-ChildItem -LiteralPath (Join-Path $root 'examples') -File | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $stage 'Samples') }
$library = Get-Content -LiteralPath (Join-Path $root 'library/starter/manifest.json') -Raw | ConvertFrom-Json
Copy-Item -LiteralPath (Join-Path $root 'library/starter/manifest.json') -Destination (Join-Path $stage 'starter-manifest.json')
Copy-Item -LiteralPath (Join-Path $root 'library/README.md') -Destination (Join-Path $stage 'Library-notes.md')
Copy-Item -LiteralPath (Join-Path $root 'library/starter/manifest.json') -Destination (Join-Path $output 'starter-library-manifest.json')
$installer = Join-Path $output "ComplyPH-Setup-$version.exe"
$notices = Join-Path $root 'packaging/windows/THIRD_PARTY_NOTICES.txt'
& $makensis '/V2' "/DAPP_EXE=$application" "/DOUTPUT_FILE=$installer" "/DNOTICES_FILE=$notices" (Join-Path $root 'packaging/windows/installer.nsi')
if ($LASTEXITCODE -ne 0) { throw 'Installer packaging failed.' }
$zip = Join-Path $output "ComplyPH-Windows-x64-$version.zip"
Compress-Archive -LiteralPath $stage -DestinationPath $zip -Force
$sourceCommit = (git -C $root rev-parse HEAD).Trim()
$manifest = [ordered]@{
    project = 'ComplyPH'; version = $version; milestone = 'Focused labour checks'
    source_commit = $sourceCommit; target = 'x86_64-pc-windows-msvc'; signing = 'unsigned'
    document_text_reading = 'offline PDF and DOCX'; private_schema = 3
    synthetic_fixture_count = 7; reader_engine = 'complyph-text-1'
    legal_checking = 'three focused labour numeric comparisons'; rule_engine = 'complyph-labor-1'; validated_check_count = 3; synthetic_check_fixture_count = 3; workspace_default = 'C:\ProgramData\ComplyPH'; automatic_downloads = 'not implemented'
    starter_source_count = $library.sources.Count; starter_domain_count = $library.domains.Count
    starter_bundle_id = $library.bundle_id; verification_scope = $library.verification_scope
    artifacts = @($installer, $zip) | ForEach-Object {
        [ordered]@{ name = (Split-Path $_ -Leaf); bytes = (Get-Item -LiteralPath $_).Length; sha256 = (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant() }
    }
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $output 'release-manifest.json') -Encoding utf8
$checksums = @($installer, $zip, (Join-Path $output 'release-manifest.json'), (Join-Path $output 'starter-library-manifest.json')) | ForEach-Object {
    '{0}  {1}' -f (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant(), (Split-Path $_ -Leaf)
}
$checksums | Set-Content -LiteralPath (Join-Path $output 'SHA256SUMS.txt') -Encoding ascii
Write-Output "Packaged ComplyPH $version from $sourceCommit"
