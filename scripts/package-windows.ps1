param([string]$OutputDirectory = 'dist/windows')
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$output = [IO.Path]::GetFullPath((Join-Path $root $OutputDirectory))
$application = Join-Path $root 'target/release/laborlens-ph.exe'
if (!(Test-Path -LiteralPath $application)) { throw 'Build the Windows release with cargo first.' }
$makensis = 'C:\Program Files (x86)\NSIS\makensis.exe'
if (!(Test-Path -LiteralPath $makensis)) {
    $command = Get-Command makensis -ErrorAction SilentlyContinue
    if (!$command) { throw 'Install the free NSIS 3 packaging tools.' }
    $makensis = $command.Source
}
$version = '0.1.0'
$stage = Join-Path $root 'target/windows-package/LaborLens PH'
if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
New-Item -ItemType Directory -Force -Path $output, $stage, (Join-Path $stage 'Samples') | Out-Null
Copy-Item -LiteralPath $application -Destination (Join-Path $stage 'LaborLensPH.exe')
Copy-Item -LiteralPath (Join-Path $root 'assets/icon.ico') -Destination (Join-Path $stage 'LaborLensPH.ico')
Copy-Item -LiteralPath (Join-Path $root 'LICENSE') -Destination (Join-Path $stage 'LICENSE.txt')
Copy-Item -LiteralPath (Join-Path $root 'assets/fonts/OFL.txt') -Destination (Join-Path $stage 'Lato-OFL.txt')
Copy-Item -LiteralPath (Join-Path $root 'packaging/windows/THIRD_PARTY_NOTICES.txt') -Destination $stage
Copy-Item -LiteralPath (Join-Path $root 'packaging/windows/Read-me.txt') -Destination $stage
Copy-Item -LiteralPath (Join-Path $root 'examples/Synthetic sample.pdf') -Destination (Join-Path $stage 'Samples')
Copy-Item -LiteralPath (Join-Path $root 'examples/Synthetic sample.docx') -Destination (Join-Path $stage 'Samples')
$installer = Join-Path $output "LaborLensPH-Setup-$version.exe"
$notices = Join-Path $root 'packaging/windows/THIRD_PARTY_NOTICES.txt'
& $makensis '/V2' "/DAPP_EXE=$application" "/DOUTPUT_FILE=$installer" "/DNOTICES_FILE=$notices" (Join-Path $root 'packaging/windows/installer.nsi')
if ($LASTEXITCODE -ne 0) { throw 'Installer packaging failed.' }
$zip = Join-Path $output "LaborLensPH-Windows-x64-$version.zip"
Compress-Archive -LiteralPath $stage -DestinationPath $zip -Force
$sourceCommit = (git -C $root rev-parse HEAD).Trim()
$manifest = [ordered]@{
    project = 'LaborLens PH'; version = $version; milestone = 'Desktop foundation'
    source_commit = $sourceCommit; target = 'x86_64-pc-windows-msvc'; signing = 'unsigned'
    legal_checking = 'not implemented'; automatic_downloads = 'not implemented'
    artifacts = @($installer, $zip) | ForEach-Object {
        [ordered]@{ name = (Split-Path $_ -Leaf); bytes = (Get-Item -LiteralPath $_).Length; sha256 = (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant() }
    }
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $output 'release-manifest.json') -Encoding utf8
$checksums = @($installer, $zip, (Join-Path $output 'release-manifest.json')) | ForEach-Object {
    '{0}  {1}' -f (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant(), (Split-Path $_ -Leaf)
}
$checksums | Set-Content -LiteralPath (Join-Path $output 'SHA256SUMS.txt') -Encoding ascii
Write-Output "Packaged LaborLens PH $version from $sourceCommit"
