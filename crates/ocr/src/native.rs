//! Fixed C-ABI bindings for the packaged x64 PDFium and Tesseract libraries.
//! All pointers are owned by their libraries, bounded and consumed in this worker.
use libloading::Library;
use std::{
    ffi::{CStr, c_char, c_int, c_void},
    path::Path,
    ptr,
};
type Handle = *mut c_void;

struct Pdf {
    init: unsafe extern "C" fn(),
    destroy: unsafe extern "C" fn(),
    load: unsafe extern "C" fn(*const c_void, usize, *const c_char) -> Handle,
    close: unsafe extern "C" fn(Handle),
    count: unsafe extern "C" fn(Handle) -> c_int,
    page: unsafe extern "C" fn(Handle, c_int) -> Handle,
    close_page: unsafe extern "C" fn(Handle),
    width: unsafe extern "C" fn(Handle) -> f32,
    height: unsafe extern "C" fn(Handle) -> f32,
    bitmap: unsafe extern "C" fn(c_int, c_int, c_int) -> Handle,
    close_bitmap: unsafe extern "C" fn(Handle),
    fill: unsafe extern "C" fn(Handle, c_int, c_int, c_int, c_int, u32) -> c_int,
    render: unsafe extern "C" fn(Handle, Handle, c_int, c_int, c_int, c_int, c_int, c_int),
    buffer: unsafe extern "C" fn(Handle) -> *mut c_void,
    stride: unsafe extern "C" fn(Handle) -> c_int,
}
struct Tess {
    version: unsafe extern "C" fn() -> *const c_char,
    create: unsafe extern "C" fn() -> Handle,
    delete: unsafe extern "C" fn(Handle),
    init: unsafe extern "C" fn(
        Handle,
        *const c_char,
        c_int,
        *const c_char,
        c_int,
        *mut *mut c_char,
        c_int,
        *mut *mut c_char,
        *mut *mut c_char,
        usize,
        c_int,
    ) -> c_int,
    image: unsafe extern "C" fn(Handle, *const u8, c_int, c_int, c_int, c_int),
    recognize: unsafe extern "C" fn(Handle, Handle) -> c_int,
    tsv: unsafe extern "C" fn(Handle, c_int) -> *mut c_char,
    free_text: unsafe extern "C" fn(*mut c_char),
    clear: unsafe extern "C" fn(Handle),
    resolution: unsafe extern "C" fn(Handle, c_int),
}
fn library(path: &Path) -> Result<Library, String> {
    // SAFETY: fixed packaged libraries, loaded only inside a resource-limited worker.
    // Windows resolves dependencies in the DLL's directory/system32, never the CWD.
    #[cfg(windows)]
    let result = unsafe {
        libloading::os::windows::Library::load_with_flags(path, 0x100 | 0x800).map(Into::into)
    };
    #[cfg(not(windows))]
    let result = unsafe { Library::new(path) };
    result.map_err(|_| "A bundled local OCR library could not be loaded. Reinstall the complete app and OCR folder.".into())
}
macro_rules! symbol {
    ($lib:expr,$name:literal) => {{
        // SAFETY: signatures match the pinned libraries' public C API headers.
        unsafe {
            *$lib
                .get(concat!($name, "\0").as_bytes())
                .map_err(|_| "The local OCR libraries have an unsupported version.")?
        }
    }};
}

pub(super) struct Engine<'a> {
    pdf: Pdf,
    tess: Tess,
    document: Handle,
    recognition: Handle,
    _pdf_library: Library,
    _tess_library: Library,
    _input: &'a [u8],
    _model: &'a [u8],
}
impl<'a> Engine<'a> {
    pub fn new(directory: &Path, input: &'a [u8], model: &'a [u8]) -> Result<Self, String> {
        #[cfg(windows)]
        let (pdf_name, tess_name) = ("pdfium.dll", "tesseract.dll");
        #[cfg(not(windows))]
        let (pdf_name, tess_name) = ("libpdfium.so", "libtesseract.so.5");
        let pdf_library = library(&directory.join(pdf_name))?;
        let tess_library = library(&directory.join(tess_name))?;
        let pdf = Pdf {
            init: symbol!(pdf_library, "FPDF_InitLibrary"),
            destroy: symbol!(pdf_library, "FPDF_DestroyLibrary"),
            load: symbol!(pdf_library, "FPDF_LoadMemDocument64"),
            close: symbol!(pdf_library, "FPDF_CloseDocument"),
            count: symbol!(pdf_library, "FPDF_GetPageCount"),
            page: symbol!(pdf_library, "FPDF_LoadPage"),
            close_page: symbol!(pdf_library, "FPDF_ClosePage"),
            width: symbol!(pdf_library, "FPDF_GetPageWidthF"),
            height: symbol!(pdf_library, "FPDF_GetPageHeightF"),
            bitmap: symbol!(pdf_library, "FPDFBitmap_Create"),
            close_bitmap: symbol!(pdf_library, "FPDFBitmap_Destroy"),
            fill: symbol!(pdf_library, "FPDFBitmap_FillRect"),
            render: symbol!(pdf_library, "FPDF_RenderPageBitmap"),
            buffer: symbol!(pdf_library, "FPDFBitmap_GetBuffer"),
            stride: symbol!(pdf_library, "FPDFBitmap_GetStride"),
        };
        let tess = Tess {
            version: symbol!(tess_library, "TessVersion"),
            create: symbol!(tess_library, "TessBaseAPICreate"),
            delete: symbol!(tess_library, "TessBaseAPIDelete"),
            init: symbol!(tess_library, "TessBaseAPIInit5"),
            image: symbol!(tess_library, "TessBaseAPISetImage"),
            recognize: symbol!(tess_library, "TessBaseAPIRecognize"),
            tsv: symbol!(tess_library, "TessBaseAPIGetTsvText"),
            free_text: symbol!(tess_library, "TessDeleteText"),
            clear: symbol!(tess_library, "TessBaseAPIClear"),
            resolution: symbol!(tess_library, "TessBaseAPISetSourceResolution"),
        };
        // SAFETY: input/model buffers outlive Engine; null password/configs disable file/URL input.
        let (document, recognition) = unsafe {
            (pdf.init)();
            (
                (pdf.load)(input.as_ptr().cast(), input.len(), ptr::null()),
                (tess.create)(),
            )
        };
        let engine = Self {
            pdf,
            tess,
            document,
            recognition,
            _pdf_library: pdf_library,
            _tess_library: tess_library,
            _input: input,
            _model: model,
        };
        if document.is_null() || recognition.is_null() {
            return Err("The local OCR engine could not open this PDF.".into());
        }
        // SAFETY: bounded known model; fixed English and LSTM mode; no configs or variables from a document.
        let initialized = unsafe {
            (engine.tess.init)(
                recognition,
                model.as_ptr().cast(),
                model.len() as c_int,
                c"eng".as_ptr(),
                1,
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                0,
            )
        };
        if initialized != 0 {
            return Err("The bundled English OCR model could not be initialized.".into());
        }
        Ok(engine)
    }
    pub fn pages(&self) -> usize {
        // SAFETY: live document handle.
        unsafe { (self.pdf.count)(self.document).max(0) as usize }
    }
    pub fn version(&self) -> String {
        // SAFETY: Tesseract returns an immutable static NUL-terminated version string.
        unsafe {
            CStr::from_ptr((self.tess.version)())
                .to_string_lossy()
                .into_owned()
        }
    }
    pub fn page(&mut self, index: usize) -> Result<String, String> {
        // SAFETY: caller bounds index by GetPageCount; handle is closed on every path.
        let page = unsafe { (self.pdf.page)(self.document, index as c_int) };
        if page.is_null() {
            return Err("A PDF page could not be rendered locally.".into());
        }
        let result = self.render_page(page);
        unsafe {
            (self.pdf.close_page)(page);
        }
        result
    }
    fn render_page(&mut self, page: Handle) -> Result<String, String> {
        // SAFETY: live page handle, dimensions validated before any allocation.
        let (w, h) = unsafe { ((self.pdf.width)(page), (self.pdf.height)(page)) };
        if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
            return Err("This PDF has unsupported page dimensions.".into());
        }
        let scale = (200.0_f32 / 72.0).min(super::MAX_SIDE as f32 / w.max(h));
        let width = (w * scale).ceil().clamp(1.0, super::MAX_SIDE as f32) as c_int;
        let height = (h * scale).ceil().clamp(1.0, super::MAX_SIDE as f32) as c_int;
        let bitmap = unsafe { (self.pdf.bitmap)(width, height, 1) };
        if bitmap.is_null() {
            return Err("The local PDF renderer could not allocate a bounded page.".into());
        }
        let result = (|| {
            // SAFETY: bounded BGRA bitmap, rendered without JavaScript/form execution.
            unsafe {
                (self.pdf.fill)(bitmap, 0, 0, width, height, 0xffff_ffff);
                (self.pdf.render)(bitmap, page, 0, 0, width, height, 0, 0);
            }
            let stride = unsafe { (self.pdf.stride)(bitmap) };
            let buffer = unsafe { (self.pdf.buffer)(bitmap) };
            if buffer.is_null() || stride != width * 4 {
                return Err("The local renderer returned an unsupported bitmap.".into());
            }
            // SAFETY: PDFium owns height rows of stride bytes until close_bitmap below.
            let bgra = unsafe {
                std::slice::from_raw_parts(buffer.cast::<u8>(), stride as usize * height as usize)
            };
            let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
            for p in bgra.as_chunks::<4>().0 {
                rgb.extend_from_slice(&[p[2], p[1], p[0]]);
            }
            unsafe {
                (self.tess.image)(self.recognition, rgb.as_ptr(), width, height, 3, width * 3);
                (self.tess.resolution)(self.recognition, (72.0 * scale).round() as c_int);
                if (self.tess.recognize)(self.recognition, ptr::null_mut()) != 0 {
                    return Err("A page could not be recognized locally.".into());
                }
                let text = (self.tess.tsv)(self.recognition, 0);
                if text.is_null() {
                    return Err("Local OCR returned no page result.".into());
                }
                // Tesseract's own allocated UTF-8 text; take a bounded copy before freeing.
                let raw = CStr::from_ptr(text).to_bytes();
                let result = if raw.len() > 12 * 1024 * 1024 {
                    Err("The OCR page result exceeds its limit.".into())
                } else {
                    std::str::from_utf8(raw)
                        .map(str::to_owned)
                        .map_err(|_| "OCR returned invalid text.".into())
                };
                (self.tess.free_text)(text);
                (self.tess.clear)(self.recognition);
                result
            }
        })();
        unsafe {
            (self.pdf.close_bitmap)(bitmap);
        }
        result
    }
}
impl Drop for Engine<'_> {
    fn drop(&mut self) {
        // SAFETY: owned handles, no outstanding page/bitmap/text handles at this point.
        unsafe {
            if !self.recognition.is_null() {
                (self.tess.delete)(self.recognition);
            }
            if !self.document.is_null() {
                (self.pdf.close)(self.document);
            }
            (self.pdf.destroy)();
        }
    }
}
