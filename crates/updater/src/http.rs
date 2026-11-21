//! Only reachable after the worker verifies its AppContainer token.
use crate::protocol::{Reply, Request};
use complyph_library::{MAX_DOWNLOAD_BYTES, approved_update_url, download_mime, same_origin};
use std::{io::Read, net::IpAddr, time::Duration};
use ureq::{
    http::Uri,
    unversioned::{
        resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver},
        transport::{DefaultConnector, NextTimeout},
    },
};

#[derive(Debug)]
struct PublicResolver;

fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && (b == 168 || b == 0 || (b == 2)))
                || (a == 198 && ((18..=19).contains(&b) || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            let s = ip.segments();
            (s[0] & 0xe000) == 0x2000
                && !(s[0] == 0x2001 && (s[1] < 0x0200 || s[1] == 0x0db8))
                && s[0] != 0x2002
        }
    }
}

impl Resolver for PublicResolver {
    fn resolve(
        &self,
        uri: &Uri,
        config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        approved_update_url(&uri.to_string()).map_err(|_| ureq::Error::HostNotFound)?;
        let addresses = DefaultResolver::default().resolve(uri, config, timeout)?;
        // Reject the entire DNS answer if any address is private/reserved. The
        // returned addresses are used by the connector, without a second lookup.
        if addresses
            .iter()
            .any(|address| address.port() != 443 || !public_ip(address.ip()))
        {
            return Err(ureq::Error::HostNotFound);
        }
        Ok(addresses)
    }
}

fn validator(value: &Option<String>, limit: usize) -> Option<&str> {
    value
        .as_deref()
        .filter(|v| v.len() <= limit && v.is_ascii() && !v.chars().any(char::is_control))
}

fn run(
    url: &str,
    etag: &Option<String>,
    modified: &Option<String>,
) -> Result<(Reply, Vec<u8>), String> {
    let original = approved_update_url(url).map_err(|e| e.to_string())?;
    let config = ureq::Agent::config_builder()
        .https_only(true)
        .proxy(None)
        .max_redirects(0)
        .http_status_as_error(false)
        .max_response_header_size(16 * 1024)
        .timeout_global(Some(Duration::from_secs(20)))
        .timeout_connect(Some(Duration::from_secs(5)))
        .timeout_resolve(Some(Duration::from_secs(5)))
        .timeout_per_read(Some(Duration::from_secs(5)))
        .user_agent(concat!("ComplyPH-source-check/", env!("CARGO_PKG_VERSION")))
        .build();
    let agent = ureq::Agent::with_parts(config, DefaultConnector::default(), PublicResolver);
    let mut current = original.clone();
    for hop in 0..=3 {
        let mut request=agent.get(&current).header("Accept-Encoding","identity").header("Accept","application/pdf,text/html,text/plain,application/vnd.openxmlformats-officedocument.wordprocessingml.document");
        if let Some(value) = validator(etag, 512) {
            request = request.header("If-None-Match", value);
        }
        if let Some(value) = validator(modified, 128) {
            request = request.header("If-Modified-Since", value);
        }
        let mut response = request.call().map_err(|error| {
            format!("The approved URL could not be reached securely within the connection/time limits: {error}")
        })?;
        let code = response.status().as_u16();
        if matches!(code, 301 | 302 | 303 | 307 | 308) {
            if hop == 3 {
                return Err("The source exceeded the three-redirect limit.".into());
            }
            let location = response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .ok_or("The redirect did not provide a valid location.")?;
            let next = url::Url::parse(&current)
                .and_then(|u| u.join(location))
                .map_err(|_| "The redirect URL was invalid.")?
                .to_string();
            approved_update_url(&next).map_err(|e| e.to_string())?;
            if !same_origin(&original, &next) {
                return Err("The source redirected outside its approved origin. Approve the new official URL explicitly.".into());
            }
            current = next;
            continue;
        }
        if code == 304 {
            return Ok((
                Reply::NotModified {
                    resolved_url: current,
                },
                Vec::new(),
            ));
        }
        if code != 200 {
            return Err(format!(
                "The agency server returned HTTP {code}. The saved references were retained."
            ));
        }
        if response
            .headers()
            .get("content-encoding")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| !v.eq_ignore_ascii_case("identity"))
        {
            return Err("The server returned an unsupported compressed response.".into());
        }
        if response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .is_some_and(|n| n > MAX_DOWNLOAD_BYTES as u64)
        {
            return Err("The download exceeds 32 MiB.".into());
        }
        let header = |key: &str| {
            response
                .headers()
                .get(key)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let mime = header("content-type").unwrap_or_default();
        let etag = header("etag");
        let last_modified = header("last-modified");
        let mut body = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(MAX_DOWNLOAD_BYTES as u64 + 1)
            .read_to_end(&mut body)
            .map_err(|_| "The download ended early or exceeded its time limit.")?;
        let mime = download_mime(&body, &mime)
            .map_err(|e| e.to_string())?
            .to_owned();
        return Ok((
            Reply::Download {
                resolved_url: current,
                mime,
                etag,
                last_modified,
            },
            body,
        ));
    }
    Err("The source redirect could not be followed.".into())
}

pub fn fetch(request: &Request) -> (Reply, Vec<u8>) {
    if let Err(message) = crate::sandbox::require_isolation() {
        return (Reply::Failed { message }, Vec::new());
    }
    let Request::Fetch {
        url,
        etag,
        last_modified,
    } = request
    else {
        return (
            Reply::Failed {
                message: "Invalid fetch request.".into(),
            },
            Vec::new(),
        );
    };
    run(url, etag, last_modified).unwrap_or_else(|message| (Reply::Failed { message }, Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolver_policy_blocks_private_special_and_mapped_addresses() {
        for ip in [
            "0.0.0.0",
            "127.0.0.1",
            "10.0.0.1",
            "169.254.169.254",
            "172.16.0.1",
            "192.168.1.1",
            "100.64.0.1",
            "198.18.0.1",
            "192.0.2.1",
            "203.0.113.1",
            "224.0.0.1",
            "::1",
            "::ffff:127.0.0.1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "2002:7f00:1::",
        ] {
            assert!(!public_ip(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111"] {
            assert!(public_ip(ip.parse().unwrap()), "{ip}");
        }
    }
}
