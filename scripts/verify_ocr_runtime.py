"""Verify the packaged x64 OCR dependency closure without running any library."""
import json
import struct
import sys
from pathlib import Path

SYSTEM_DLLS = {
    'KERNEL32.DLL', 'KERNELBASE.DLL', 'NTDLL.DLL', 'USER32.DLL', 'GDI32.DLL',
    'ADVAPI32.DLL', 'WS2_32.DLL', 'OLE32.DLL', 'OLEAUT32.DLL', 'SHELL32.DLL',
    'SHLWAPI.DLL', 'VERSION.DLL', 'BCRYPT.DLL', 'CRYPT32.DLL', 'UCRTBASE.DLL',
    'COMDLG32.DLL', 'COMCTL32.DLL', 'RPCRT4.DLL', 'USERENV.DLL', 'NORMALIZ.DLL',
    'IMM32.DLL', 'DWRITE.DLL', 'D2D1.DLL', 'DXGI.DLL', 'D3D11.DLL',
}


def imports(data):
    pe = struct.unpack_from('<I', data, 0x3c)[0]
    assert data[pe:pe + 4] == b'PE\0\0', 'Invalid native library'
    assert struct.unpack_from('<H', data, pe + 4)[0] == 0x8664, 'OCR requires x64 libraries'
    opt = pe + 24
    assert struct.unpack_from('<H', data, opt)[0] == 0x20b, 'Expected PE32+'
    sections = opt + struct.unpack_from('<H', data, pe + 20)[0]

    def offset(rva):
        for i in range(struct.unpack_from('<H', data, pe + 6)[0]):
            size, address, raw_size, raw_offset = struct.unpack_from('<IIII', data, sections + i * 40 + 8)
            if address <= rva < address + max(size, raw_size):
                return raw_offset + rva - address
        raise AssertionError('Unmapped native-library address')

    table = struct.unpack_from('<I', data, opt + 120)[0]
    if not table:
        return []
    cursor = offset(table)
    result = []
    for _ in range(1024):
        if not any(data[cursor:cursor + 20]):
            return result
        start = offset(struct.unpack_from('<I', data, cursor + 12)[0])
        end = data.index(b'\0', start)
        result.append(data[start:end].decode('ascii').upper())
        cursor += 20
    raise AssertionError('Native import table exceeds its limit')


def verify(files):
    names = {name.upper() for name in files}
    assert {'PDFIUM.DLL', 'TESSERACT.DLL', 'ENG.TRAINEDDATA'} <= names
    for name, data in files.items():
        if not name.lower().endswith('.dll'):
            continue
        for dependency in imports(data):
            if dependency.startswith(('API-MS-WIN-', 'EXT-MS-WIN-')) or dependency in SYSTEM_DLLS:
                continue
            assert dependency in names, f'{name} requires unbundled {dependency}'


if __name__ == '__main__':
    root = Path(sys.argv[1])
    manifest = json.loads((root / 'runtime-manifest.json').read_text(encoding='utf-8-sig'))
    files = {item['name']: (root / item['name']).read_bytes() for item in manifest['files']}
    verify(files)
    print('Verified self-contained x64 OCR libraries; only Windows system DLLs remain external.')
