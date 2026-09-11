use super::{providers, types::Source, Cancel};
use scraper::{Html, Selector};
use std::{
    io::Read,
    net::{IpAddr, ToSocketAddrs},
    time::Duration,
};

pub trait SearchProvider {
    fn search(&self, query: &str, cancel: &Cancel) -> Result<Vec<Source>, String>;
}
pub struct Brave;
impl SearchProvider for Brave {
    fn search(&self, query: &str, cancel: &Cancel) -> Result<Vec<Source>, String> {
        cancel.check()?;
        let v = providers::response_json(
            providers::client()?
                .get("https://api.search.brave.com/res/v1/web/search")
                .header("X-Subscription-Token", providers::secret("brave")?)
                .query(&[("q", query), ("count", "5")])
                .send()
                .map_err(|_| "Search could not connect.".to_string())?,
        )?;
        cancel.check()?;
        Ok(v["web"]["results"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| {
                let url = r["url"].as_str()?;
                if !url.starts_with("https://") {
                    return None;
                }
                Some(Source {
                    title: r["title"].as_str().unwrap_or("Source").into(),
                    url: url.into(),
                    excerpt: r["description"].as_str().unwrap_or("").into(),
                })
            })
            .collect())
    }
}
fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_unspecified()
                || ip.is_multicast()
                || ip.octets()[0] == 0
                || ip.octets()[0] >= 240
                || (ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1])))
        }
        IpAddr::V6(ip) => {
            let s = ip.segments();
            ip.to_ipv4_mapped().is_none()
                && !ip.is_loopback()
                && !ip.is_unspecified()
                && (s[0] & 0xe000) == 0x2000
                && !(s[0] == 0x2001 && s[1] == 0x0db8)
        }
    }
}
/// Resolve once and pin the address. Do not follow redirects to private hosts.
pub fn fetch(url: &str, cancel: &Cancel) -> Result<Source, String> {
    cancel.check()?;
    let u = reqwest::Url::parse(url).map_err(|_| "Invalid page URL".to_string())?;
    if u.scheme() != "https"
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port_or_known_default() != Some(443)
    {
        return Err("Page reading supports public HTTPS URLs on port 443 only.".into());
    }
    let host = u.host_str().ok_or("URL has no host")?;
    let addresses: Vec<_> = (host, 443)
        .to_socket_addrs()
        .map_err(|_| "Could not resolve page host".to_string())?
        .collect();
    if addresses.is_empty() || addresses.iter().any(|a| !public_ip(a.ip())) {
        return Err("Private and reserved network addresses cannot be fetched.".into());
    }
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .resolve_to_addrs(host, &addresses)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| "Page client failed".to_string())?;
    let response = client
        .get(u.clone())
        .send()
        .map_err(|_| "Page could not be retrieved".to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "Page returned HTTP {}; redirects are not followed.",
            response.status().as_u16()
        ));
    }
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !(content_type.starts_with("text/html") || content_type.starts_with("text/plain")) {
        return Err("Only HTML and text web pages are supported.".into());
    }
    let plain = content_type.starts_with("text/plain");
    let mut body = String::new();
    response
        .take(1_000_001)
        .read_to_string(&mut body)
        .map_err(|_| "Page is not valid text".to_string())?;
    if body.len() > 1_000_000 {
        return Err("Page exceeds the 1 MB limit.".into());
    }
    cancel.check()?;
    if plain {
        return Ok(Source {
            title: host.into(),
            url: u.to_string(),
            excerpt: body.chars().take(24_000).collect(),
        });
    }
    let html = Html::parse_document(&body);
    let title = html
        .select(&Selector::parse("title").unwrap())
        .next()
        .map(|e| e.text().collect::<String>())
        .unwrap_or_else(|| host.into());
    let excerpt = html
        .select(&Selector::parse("h1,h2,h3,p,li,pre,td").unwrap())
        .map(|e| e.text().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(24_000)
        .collect();
    Ok(Source {
        title,
        url: u.to_string(),
        excerpt,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_private_and_mapped_addresses() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.169.254",
            "100.64.0.1",
            "::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(!public_ip(ip.parse().unwrap()));
        }
        assert!(public_ip("8.8.8.8".parse().unwrap()));
    }
}
