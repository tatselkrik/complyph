#!/usr/bin/env bash
set -euo pipefail
project_dir="$(cd "$(dirname "$0")/.." && pwd)"
runtime_dir="$project_dir/target/ocr-runtime"
mkdir -p "$runtime_dir"
curl --fail --location --silent --show-error https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8165/pdfium-linux-x64.tgz -o "$runtime_dir/pdfium.tgz"
printf '%s  %s\n' ba7865e16c0271daab75256790be06c26272190afe6e2a70eae0bb39bb99089c "$runtime_dir/pdfium.tgz" | sha256sum -c -
tar -xzf "$runtime_dir/pdfium.tgz" -C "$runtime_dir"
ln -sfn lib/libpdfium.so "$runtime_dir/libpdfium.so"
ocr_library="$(ldconfig -p | awk '/libtesseract.so.5 / && !found {print $NF; found=1}')"
test -n "$ocr_library" || { echo 'Install the system Tesseract 5 library first (for example libtesseract5 on Debian/Ubuntu).'; exit 1; }
ln -sfn "$ocr_library" "$runtime_dir/libtesseract.so.5"
curl --fail --location --silent --show-error https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/87416418657359cb625c412a48b6e1d6d41c29bd/eng.traineddata -o "$runtime_dir/eng.traineddata"
printf '%s  %s\n' 7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2 "$runtime_dir/eng.traineddata" | sha256sum -c -
printf 'For development: export COMPLYPH_OCR_DIR=%q\n' "$runtime_dir"
