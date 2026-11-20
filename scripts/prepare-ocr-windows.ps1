$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$output = Join-Path $root 'target/ocr-runtime'
$vcpkgRoot = Join-Path $root 'target/ocr-vcpkg'
$commit = '74f5ea45ca78988784b810ad9bbb148ea7ff76d8'
if (Test-Path -LiteralPath $output) { Remove-Item -LiteralPath $output -Recurse -Force }
New-Item -ItemType Directory -Force -Path $output, (Join-Path $output 'licenses') | Out-Null
if (!(Test-Path (Join-Path $vcpkgRoot '.git'))) {
    git clone --filter=blob:none --no-checkout https://github.com/microsoft/vcpkg.git $vcpkgRoot
    if ($LASTEXITCODE -ne 0) { throw 'Could not fetch pinned OCR build tools.' }
}
git -C $vcpkgRoot checkout --detach $commit
if ($LASTEXITCODE -ne 0) { throw 'Could not select pinned OCR build tools.' }
& (Join-Path $vcpkgRoot 'bootstrap-vcpkg.bat') -disableMetrics
if ($LASTEXITCODE -ne 0) { throw 'OCR build tool bootstrap failed.' }
# core excludes Tesseract's default curl/libarchive features. No URL image input.
$triplets = Join-Path $root 'target/ocr-triplets'
New-Item -ItemType Directory -Force -Path $triplets | Out-Null
@'
set(VCPKG_TARGET_ARCHITECTURE x64)
set(VCPKG_CRT_LINKAGE static)
set(VCPKG_LIBRARY_LINKAGE dynamic)
set(VCPKG_BUILD_TYPE release)
# The shared JPEG codec forces a dynamic VC runtime upstream. Link this codec
# statically into its consumers so a fresh PC needs no redistributable installer.
if(PORT STREQUAL "libjpeg-turbo")
    set(VCPKG_LIBRARY_LINKAGE static)
endif()
'@ | Set-Content (Join-Path $triplets 'x64-complyph-ocr.cmake') -Encoding ascii
& (Join-Path $vcpkgRoot 'vcpkg.exe') install 'tesseract[core]:x64-complyph-ocr' "--overlay-triplets=$triplets" --disable-metrics
if ($LASTEXITCODE -ne 0) { throw 'The pinned offline OCR engine could not be built.' }
$installed = Join-Path $vcpkgRoot 'installed/x64-complyph-ocr'
Get-ChildItem (Join-Path $installed 'bin') -Filter '*.dll' | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $output -Force }
$tesseract = @(Get-ChildItem $output -Filter '*tesseract*.dll')
if ($tesseract.Count -ne 1) { throw 'Expected one Tesseract C-API library.' }
if ($tesseract[0].Name -ne 'tesseract.dll') { Move-Item -LiteralPath $tesseract[0].FullName -Destination (Join-Path $output 'tesseract.dll') -Force }
if (@(Get-ChildItem $output -File | Where-Object { $_.Name -match 'curl|archive|ssl|crypto' }).Count -gt 0) { throw 'Unexpected network/archive dependency in OCR bundle.' }
Get-ChildItem (Join-Path $installed 'share') -Filter copyright -Recurse | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $output ('licenses/' + $_.Directory.Name + '-copyright.txt')) -Force
}
$archive = Join-Path $root 'target/pdfium-win-x64.tgz'
Invoke-WebRequest 'https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8165/pdfium-win-x64.tgz' -OutFile $archive
if ((Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne 'ca7338ae7779b8b1b82f4580305c4d762d214f8df1d54b47647fe3cc75989ded') { throw 'PDF renderer package hash mismatch.' }
$pdfium = Join-Path $root 'target/pdfium-windows'
New-Item -ItemType Directory -Force -Path $pdfium | Out-Null
tar -xzf $archive -C $pdfium
if ($LASTEXITCODE -ne 0) { throw 'PDF renderer extraction failed.' }
Copy-Item -LiteralPath (Join-Path $pdfium 'bin/pdfium.dll') -Destination $output -Force
Copy-Item -LiteralPath (Join-Path $pdfium 'LICENSE') -Destination (Join-Path $output 'licenses/PDFium-LICENSE.txt') -Force
Copy-Item -LiteralPath (Join-Path $pdfium 'licenses') -Destination (Join-Path $output 'licenses/PDFium-third-party') -Recurse -Force
Invoke-WebRequest 'https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/87416418657359cb625c412a48b6e1d6d41c29bd/eng.traineddata' -OutFile (Join-Path $output 'eng.traineddata')
if ((Get-FileHash (Join-Path $output 'eng.traineddata') -Algorithm SHA256).Hash.ToLowerInvariant() -ne '7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2') { throw 'OCR model hash mismatch.' }
Invoke-WebRequest 'https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/87416418657359cb625c412a48b6e1d6d41c29bd/LICENSE' -OutFile (Join-Path $output 'licenses/tessdata-fast-LICENSE.txt')
# Runtime files can be reused without any downloader or setup command on the user's PC.
$files = @(Get-ChildItem $output -File | ForEach-Object { [ordered]@{ name=$_.Name; sha256=(Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() } })
[ordered]@{ tesseract='5.5.4'; vcpkg_commit=$commit; pdfium='8165'; language='eng'; model_sha256='7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2'; network_features=$false; files=$files } | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $output 'runtime-manifest.json') -Encoding utf8
if ($env:GITHUB_ENV) { "COMPLYPH_OCR_DIR=$output" | Out-File $env:GITHUB_ENV -Encoding utf8 -Append }
python (Join-Path $PSScriptRoot 'verify_ocr_runtime.py') $output
if ($LASTEXITCODE -ne 0) { throw 'The OCR runtime has an unbundled dependency.' }
Write-Output 'Prepared pinned local OCR libraries and licenses; URL support is disabled.'
