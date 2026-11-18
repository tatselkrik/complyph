use complyph_library::MAX_DOWNLOAD_BYTES;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

#[cfg(any(windows, test))]
pub const MAX_WIRE_BYTES: usize = MAX_DOWNLOAD_BYTES + 8196;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Fetch {
        url: String,
        etag: Option<String>,
        last_modified: Option<String>,
    },
    /// Explicit diagnostic only; no source URL or file contents are sent in this mode.
    Probe { files: Vec<std::path::PathBuf> },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reply {
    Download {
        resolved_url: String,
        mime: String,
        etag: Option<String>,
        last_modified: Option<String>,
    },
    NotModified {
        resolved_url: String,
    },
    Failed {
        message: String,
    },
    Probe {
        app_container: bool,
        all_files_denied: bool,
        files_tested: usize,
    },
}

pub fn encode(reply: &Reply, body: &[u8], mut output: impl Write) -> Result<(), String> {
    let header =
        serde_json::to_vec(reply).map_err(|_| "The downloader result could not be encoded.")?;
    if header.len() > 8192 || body.len() > MAX_DOWNLOAD_BYTES {
        return Err("The downloader exceeded its output limit.".into());
    }
    output
        .write_all(&(header.len() as u32).to_le_bytes())
        .and_then(|_| output.write_all(&header))
        .and_then(|_| output.write_all(body))
        .map_err(|_| "The downloader result could not be written.".into())
}

#[cfg(any(windows, test))]
pub fn decode(bytes: &[u8]) -> Result<(Reply, Vec<u8>), String> {
    if bytes.len() < 4 || bytes.len() > MAX_WIRE_BYTES {
        return Err("The downloader returned an invalid response size.".into());
    }
    let length = u32::from_le_bytes(bytes[..4].try_into().expect("four bytes")) as usize;
    if length > 8192 || length + 4 > bytes.len() {
        return Err("The downloader returned an invalid header.".into());
    }
    let reply: Reply = serde_json::from_slice(&bytes[4..4 + length])
        .map_err(|_| "The downloader returned an invalid result.")?;
    let body = bytes[4 + length..].to_vec();
    if !matches!(reply, Reply::Download { .. }) && !body.is_empty() {
        return Err("Unexpected bytes in the downloader result.".into());
    }
    Ok((reply, body))
}

pub fn read_request() -> Result<Request, String> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .take(8193)
        .read_to_end(&mut bytes)
        .map_err(|_| "The downloader input could not be read.")?;
    if bytes.len() > 8192 {
        return Err("The downloader input exceeded its limit.".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "The downloader input was invalid.".into())
}
