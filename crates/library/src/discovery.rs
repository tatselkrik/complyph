//! Bounded link discovery from explicitly approved official publication pages.
//! Links are public citations only. Discovering a link never approves a download.
use super::*;
use chrono::Utc;
use scraper::{Html, Selector};
use std::collections::BTreeMap;

pub(super) fn migrate(tx: &rusqlite::Transaction<'_>) -> Result<()> {
    tx.execute_batch("
      CREATE TABLE publication_pages(id INTEGER PRIMARY KEY,url TEXT NOT NULL UNIQUE,domain_id TEXT NOT NULL REFERENCES domains(id),enabled INTEGER NOT NULL,approved_at INTEGER NOT NULL,last_attempt INTEGER,last_success INTEGER,last_status TEXT NOT NULL DEFAULT 'Never scanned',last_error TEXT,lease_token TEXT,lease_until INTEGER);
      CREATE TABLE publication_versions(id INTEGER PRIMARY KEY,page_id INTEGER NOT NULL REFERENCES publication_pages(id),sha256 TEXT NOT NULL,raw_bytes BLOB NOT NULL,retrieved_at INTEGER NOT NULL,UNIQUE(page_id,sha256));
      CREATE TABLE publication_links(id INTEGER PRIMARY KEY,page_id INTEGER NOT NULL REFERENCES publication_pages(id),url TEXT NOT NULL,title TEXT NOT NULL,first_seen INTEGER NOT NULL,last_seen INTEGER NOT NULL,initial_scan INTEGER NOT NULL,UNIQUE(page_id,url));
      CREATE TABLE publication_link_reviews(id INTEGER PRIMARY KEY,link_id INTEGER NOT NULL REFERENCES publication_links(id),decision TEXT NOT NULL CHECK(decision IN('dismissed','reopened')),actor TEXT NOT NULL,basis TEXT NOT NULL,review_on TEXT NOT NULL,recorded_at INTEGER NOT NULL);
    ")?;
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicationPage {
    pub id: i64,
    pub url: String,
    pub domain_id: String,
    pub enabled: bool,
    pub last_attempt: Option<i64>,
    pub last_success: Option<i64>,
    pub last_status: String,
    pub last_error: Option<String>,
}
#[derive(Clone, Debug)]
pub struct PublicationTarget {
    pub page_id: i64,
    pub url: String,
    pub lease_token: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveredIssuance {
    pub id: i64,
    pub page_id: i64,
    pub url: String,
    pub title: String,
    pub domain_id: String,
    pub first_seen: i64,
    pub last_seen: i64,
    pub initial_scan: bool,
    pub dismissed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveryReceipt {
    pub page_id: i64,
    pub checked_at: i64,
    pub outcome: String,
    pub links_seen: usize,
    pub new_links: usize,
    pub detail: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct PublicationLinks {
    pub links: Vec<(String, String)>,
    pub truncated: bool,
}
pub fn publication_links(base: &str, bytes: &[u8]) -> Result<PublicationLinks> {
    approved_update_url(base)?;
    if bytes.len() > MAX_PUBLIC_HTML_BYTES {
        return Err(LibraryError::InvalidImport(
            "Publication-page discovery is limited to 1 MiB per approved page.".into(),
        ));
    }
    let html = std::str::from_utf8(bytes).map_err(|_| {
        LibraryError::InvalidImport(
            "Publication pages must use UTF-8; scripts and browser automation are not used.".into(),
        )
    })?;
    let document = Html::parse_document(html);
    let selector = Selector::parse("a[href]").expect("fixed selector");
    let base = url::Url::parse(base)
        .map_err(|_| LibraryError::InvalidImport("Invalid publication URL.".into()))?;
    let mut links = BTreeMap::new();
    let mut truncated = false;
    for anchor in document.select(&selector) {
        if anchor.ancestors().any(|n|matches!(n.value(),scraper::Node::Element(e) if matches!(e.name(),"script"|"style"|"template"|"noscript") || e.attr("hidden").is_some())) {continue;}
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        let Ok(mut url) = base.join(href) else {
            continue;
        };
        url.set_fragment(None);
        let Ok(url) = approved_update_url(url.as_str()) else {
            continue;
        };
        let words = anchor
            .text()
            .flat_map(str::split_whitespace)
            .collect::<Vec<_>>()
            .join(" ");
        let title = if words.is_empty() { url.clone() } else { words };
        if title.len() > 1000 {
            continue;
        }
        let lower = title.to_lowercase();
        let path = url::Url::parse(&url)
            .expect("validated URL")
            .path()
            .to_ascii_lowercase();
        if !path.ends_with(".pdf")
            && !path.ends_with(".docx")
            && ![
                "circular",
                "advisory",
                "department order",
                "revenue regulation",
                "revenue memorandum",
                "republic act",
                "issuance",
                "board resolution",
                "implementing rules",
            ]
            .iter()
            .any(|term| lower.contains(term))
        {
            continue;
        }
        links.entry(url).or_insert(title);
        if links.len() >= 500 {
            truncated = true;
            break;
        }
    }
    Ok(PublicationLinks {
        links: links.into_iter().collect(),
        truncated,
    })
}

impl LibraryCatalog {
    pub fn approve_publication_page(&self, url: &str, domain: &str) -> Result<()> {
        let url = approved_update_url(url)?;
        if !starter_manifest()?.domains.iter().any(|d| d.id == domain) {
            return Err(LibraryError::InvalidImport(
                "Choose a supported compliance domain.".into(),
            ));
        }
        self.connection.execute("INSERT INTO publication_pages(url,domain_id,enabled,approved_at) VALUES(?1,?2,1,?3) ON CONFLICT(url) DO UPDATE SET domain_id=excluded.domain_id,enabled=1,approved_at=excluded.approved_at,lease_token=NULL,lease_until=NULL",params![url,domain,Utc::now().timestamp()])?;
        Ok(())
    }
    pub fn disable_publication_page(&self, id: i64) -> Result<()> {
        self.connection.execute(
            "UPDATE publication_pages SET enabled=0,lease_token=NULL,lease_until=NULL WHERE id=?1",
            [id],
        )?;
        Ok(())
    }
    pub fn publication_pages(&self) -> Result<Vec<PublicationPage>> {
        let mut stmt=self.connection.prepare("SELECT id,url,domain_id,enabled,last_attempt,last_success,last_status,last_error FROM publication_pages ORDER BY id")?;
        Ok(stmt
            .query_map([], |r| {
                Ok(PublicationPage {
                    id: r.get(0)?,
                    url: r.get(1)?,
                    domain_id: r.get(2)?,
                    enabled: r.get(3)?,
                    last_attempt: r.get(4)?,
                    last_success: r.get(5)?,
                    last_status: r.get(6)?,
                    last_error: r.get(7)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?)
    }
    pub fn due_publication_page_ids(&self, due_only: bool) -> Result<Vec<i64>> {
        let now = Utc::now().timestamp();
        let mut stmt=self.connection.prepare("SELECT id FROM publication_pages WHERE enabled=1 AND (lease_until IS NULL OR lease_until<?1) AND (?2=0 OR last_attempt IS NULL OR last_attempt<=?1-86400 OR last_status='checking') ORDER BY COALESCE(last_attempt,0),id LIMIT 10")?;
        Ok(stmt
            .query_map(params![now, due_only], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?)
    }
    pub fn claim_publication_page(
        &self,
        id: i64,
        due_only: bool,
    ) -> Result<Option<PublicationTarget>> {
        let tx =
            rusqlite::Transaction::new_unchecked(&self.connection, TransactionBehavior::Immediate)?;
        let now = Utc::now().timestamp();
        let url:Option<String>=tx.query_row("SELECT url FROM publication_pages WHERE id=?1 AND enabled=1 AND (lease_until IS NULL OR lease_until<?2) AND (?3=0 OR last_attempt IS NULL OR last_attempt<=?2-86400 OR last_status='checking')",params![id,now,due_only],|r|r.get(0)).optional()?;
        let Some(url) = url else { return Ok(None) };
        approved_update_url(&url)?;
        let lease_token = sha256(
            format!(
                "{}-{id}-{}",
                std::process::id(),
                Utc::now().timestamp_nanos_opt().unwrap_or_default()
            )
            .as_bytes(),
        );
        tx.execute("UPDATE publication_pages SET last_attempt=?1,last_status='checking',lease_token=?2,lease_until=?1+90,last_error=NULL WHERE id=?3",params![now,lease_token,id])?;
        tx.commit()?;
        Ok(Some(PublicationTarget {
            page_id: id,
            url,
            lease_token,
        }))
    }
    pub fn finish_publication_page(
        &self,
        target: &PublicationTarget,
        reply: UpdateReply,
    ) -> Result<DiscoveryReceipt> {
        let now = Utc::now().timestamp();
        let parsed = (|| -> Result<(Vec<u8>, PublicationLinks)> {
            let UpdateReply::Download {
                resolved_url,
                mime,
                bytes,
                ..
            } = reply
            else {
                let message = if let UpdateReply::Failed { message } = reply {
                    message
                } else {
                    "An unconditional discovery scan needs a complete HTML response.".into()
                };
                return Err(LibraryError::InvalidImport(message));
            };
            approved_update_url(&resolved_url)?;
            if !same_origin(&target.url, &resolved_url)
                || download_mime(&bytes, &mime)? != "text/html"
            {
                return Err(LibraryError::InvalidImport(
                    "Publication discovery requires HTML from the approved page's origin.".into(),
                ));
            }
            let links = publication_links(&resolved_url, &bytes)?;
            Ok((bytes, links))
        })();
        let tx =
            rusqlite::Transaction::new_unchecked(&self.connection, TransactionBehavior::Immediate)?;
        let lease: Option<String> = tx
            .query_row(
                "SELECT lease_token FROM publication_pages WHERE id=?1 AND url=?2 AND enabled=1",
                params![target.page_id, target.url],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        if lease.as_deref() != Some(&target.lease_token) {
            return Err(LibraryError::InvalidImport(
                "Publication-page approval changed; the scan was discarded.".into(),
            ));
        }
        let mut receipt = DiscoveryReceipt {
            page_id: target.page_id,
            checked_at: now,
            outcome: "failed".into(),
            links_seen: 0,
            new_links: 0,
            detail: String::new(),
        };
        let success = match parsed {
            Err(error) => {
                receipt.detail = error
                    .to_string()
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(600)
                    .collect();
                false
            }
            Ok((bytes, parsed)) => {
                let first: bool = tx.query_row(
                    "SELECT NOT EXISTS(SELECT 1 FROM publication_versions WHERE page_id=?1)",
                    [target.page_id],
                    |r| r.get(0),
                )?;
                tx.execute("INSERT OR IGNORE INTO publication_versions(page_id,sha256,raw_bytes,retrieved_at) VALUES(?1,?2,?3,?4)",params![target.page_id,sha256(&bytes),bytes,now])?;
                receipt.links_seen = parsed.links.len();
                for (url, title) in parsed.links {
                    receipt.new_links+=tx.execute("INSERT OR IGNORE INTO publication_links(page_id,url,title,first_seen,last_seen,initial_scan) VALUES(?1,?2,?3,?4,?4,?5)",params![target.page_id,url,title,now,first])?;
                    tx.execute(
                        "UPDATE publication_links SET last_seen=?1 WHERE page_id=?2 AND url=?3",
                        params![now, target.page_id, url],
                    )?;
                }
                receipt.outcome = if parsed.truncated {
                    "partial"
                } else {
                    "scanned"
                }
                .into();
                receipt.detail=if parsed.truncated{"The 500-link scan limit was reached. Review the preserved page; discovery is incomplete."}else if receipt.links_seen==0{"No qualifying publication links were found. Dynamic pages and unlinked issuances require manual intake."}else if first{"Initial page inventory saved. These are newly discovered citations, not confirmed new or effective laws."}else{"Official publication links inventoried. Discovered links are not fetched or adopted automatically."}.into();
                true
            }
        };
        tx.execute("UPDATE publication_pages SET last_success=CASE WHEN ?1 THEN ?2 ELSE last_success END,last_status=?3,last_error=CASE WHEN ?1 THEN NULL ELSE ?4 END,lease_token=NULL,lease_until=NULL WHERE id=?5",params![success,now,receipt.outcome,receipt.detail,target.page_id])?;
        tx.commit()?;
        Ok(receipt)
    }
    pub fn discovered_issuances(&self) -> Result<Vec<DiscoveredIssuance>> {
        let mut stmt=self.connection.prepare("SELECT l.id,l.page_id,l.url,l.title,p.domain_id,l.first_seen,l.last_seen,l.initial_scan,COALESCE((SELECT decision='dismissed' FROM publication_link_reviews r WHERE r.link_id=l.id ORDER BY r.id DESC LIMIT 1),0) FROM publication_links l JOIN publication_pages p ON p.id=l.page_id ORDER BY l.initial_scan,l.first_seen DESC,l.id DESC")?;
        Ok(stmt
            .query_map([], |r| {
                Ok(DiscoveredIssuance {
                    id: r.get(0)?,
                    page_id: r.get(1)?,
                    url: r.get(2)?,
                    title: r.get(3)?,
                    domain_id: r.get(4)?,
                    first_seen: r.get(5)?,
                    last_seen: r.get(6)?,
                    initial_scan: r.get(7)?,
                    dismissed: r.get(8)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?)
    }
    pub fn review_discovered_issuance(
        &self,
        id: i64,
        dismiss: bool,
        reviewer: &LibraryReviewer,
    ) -> Result<()> {
        let reviewer = reviewer.validated()?;
        self.connection.execute("INSERT INTO publication_link_reviews(link_id,decision,actor,basis,review_on,recorded_at) VALUES(?1,?2,?3,?4,?5,?6)",params![id,if dismiss{"dismissed"}else{"reopened"},reviewer.actor,reviewer.basis,reviewer.review_on,Utc::now().timestamp()])?;
        Ok(())
    }
    pub fn publication_page_original(&self, page_id: i64) -> Result<Vec<u8>> {
        let (hash,raw):(String,Vec<u8>)=self.connection.query_row("SELECT sha256,raw_bytes FROM publication_versions WHERE page_id=?1 ORDER BY id DESC LIMIT 1",[page_id],|r|Ok((r.get(0)?,r.get(1)?)))?;
        if sha256(&raw) != hash {
            return Err(LibraryError::Integrity("preserved publication page".into()));
        }
        Ok(raw)
    }
}
