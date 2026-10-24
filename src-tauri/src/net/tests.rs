use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

/// A throwaway local server. `respond` writes the whole reply for each request.
fn serve(
    respond: impl Fn(&mut std::net::TcpStream) + Send + 'static,
) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = hits.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            counter.fetch_add(1, Ordering::SeqCst);
            let mut head = [0_u8; 2048];
            let _ = stream.read(&mut head);
            respond(&mut stream);
        }
    });
    (base, hits)
}

fn reply(stream: &mut std::net::TcpStream, status: &str, extra: &str, body: &[u8]) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n",
        body.len()
    );
    let _ = stream.write_all(body);
}

fn open(scope: Scope) -> Net {
    let net = Net::new(Consent::ephemeral());
    net.allow_for_test(scope, "127.0.0.1");
    net
}

#[test]
fn everything_is_off_until_the_user_switches_it_on_and_nothing_is_sent() {
    let (base, hits) = serve(|s| reply(s, "200 OK", "", b"hello"));
    let net = open(Scope::Marketplace);
    for scope in Scope::ALL {
        assert!(!net.allows(scope));
    }
    let error = net
        .get(Scope::Marketplace, &format!("{base}/"), 1024)
        .unwrap_err();
    assert_eq!(error.code, "offline");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "an offline request must not connect"
    );
}

#[test]
fn a_switched_on_scope_reaches_only_its_own_hosts() {
    let (base, _) = serve(|s| reply(s, "200 OK", "", b"hello"));
    let net = open(Scope::Marketplace);
    net.set(Scope::Marketplace, Mode::Session).unwrap();
    let response = net
        .get(Scope::Marketplace, &format!("{base}/"), 1024)
        .unwrap();
    assert!(response.ok());
    assert_eq!(response.body, b"hello");
    // Another scope is still off, even to the same host.
    assert_eq!(
        net.get(Scope::Analytics, &format!("{base}/"), 1024)
            .unwrap_err()
            .code,
        "offline"
    );
    // A host that is not on the list is refused before any connection.
    let other = base.replace("127.0.0.1", "127.0.0.2");
    assert_eq!(
        net.get(Scope::Marketplace, &format!("{other}/"), 1024)
            .unwrap_err()
            .code,
        "refused"
    );
    assert_eq!(
        net.get(Scope::Marketplace, "not a url", 1024)
            .unwrap_err()
            .code,
        "refused"
    );
    assert_eq!(
        net.get(
            Scope::Marketplace,
            "https://user:pw@marketplace.ledgeling.invalid/",
            1024
        )
        .unwrap_err()
        .code,
        "refused"
    );
}

#[test]
fn a_redirect_off_the_list_is_not_followed() {
    let (base, _) = serve(|s| {
        reply(
            s,
            "302 Found",
            "Location: http://127.0.0.2:9/elsewhere\r\n",
            b"",
        );
    });
    let net = open(Scope::Marketplace);
    net.set(Scope::Marketplace, Mode::Session).unwrap();
    let response = net
        .get(Scope::Marketplace, &format!("{base}/"), 1024)
        .unwrap();
    assert_eq!(response.status, 302);
}

#[test]
fn a_response_over_the_cap_fails_instead_of_filling_memory() {
    let (base, _) = serve(|s| reply(s, "200 OK", "", &[b'x'; 4096]));
    let net = open(Scope::Marketplace);
    net.set(Scope::Marketplace, Mode::Session).unwrap();
    assert_eq!(
        net.get(Scope::Marketplace, &format!("{base}/"), 1024)
            .unwrap_err()
            .code,
        "too_large"
    );
}

#[test]
fn switching_a_scope_off_stops_a_download_in_flight() {
    let (base, _) = serve(|s| {
        let _ = write!(
            s,
            "HTTP/1.1 200 OK\r\nContent-Length: 400000\r\nConnection: close\r\n\r\n"
        );
        for _ in 0..40 {
            if s.write_all(&[b'x'; 10_000]).is_err() {
                return;
            }
            let _ = s.flush();
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    let net = Arc::new(open(Scope::Marketplace));
    net.set(Scope::Marketplace, Mode::Session).unwrap();
    let worker = net.clone();
    let url = format!("{base}/");
    let handle = std::thread::spawn(move || worker.get(Scope::Marketplace, &url, 1 << 20));
    std::thread::sleep(Duration::from_millis(400));
    net.set(Scope::Marketplace, Mode::Off).unwrap();
    let started = std::time::Instant::now();
    let error = handle.join().unwrap().unwrap_err();
    assert_eq!(error.code, "cancelled");
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn analytics_cannot_be_granted_for_one_session() {
    let net = Net::new(Consent::ephemeral());
    assert!(net.set(Scope::Analytics, Mode::Session).is_err());
    assert!(!net.allows(Scope::Analytics));
    net.set(Scope::Analytics, Mode::Remembered).unwrap();
    assert!(net.allows(Scope::Analytics));
}

#[test]
fn only_remembered_choices_survive_a_restart_and_damage_means_off() {
    let dir = std::env::temp_dir().join(format!("perch-net-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut consent = Consent::load(&dir);
    consent.set(Scope::Marketplace, Mode::Session).unwrap();
    consent.set(Scope::Analytics, Mode::Remembered).unwrap();
    let again = Consent::load(&dir);
    assert_eq!(
        again.mode(Scope::Marketplace),
        Mode::Off,
        "a session choice is not saved"
    );
    assert_eq!(again.mode(Scope::Analytics), Mode::Remembered);
    std::fs::write(dir.join(consent::CONSENT_FILE), "{ broken").unwrap();
    for scope in Scope::ALL {
        assert_eq!(Consent::load(&dir).mode(scope), Mode::Off);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_scope_names_its_hosts_and_says_what_it_is_for() {
    for scope in Scope::ALL {
        assert!(!scope.hosts().is_empty());
        assert!(!scope.label().is_empty() && !scope.description().is_empty());
        assert_eq!(Scope::parse(scope.as_str()), Some(scope));
    }
}

mod marketplace_install {
    use super::*;
    use crate::packs::{
        gallery::{self, Source},
        loader::{self, InstallMode},
    };
    use std::path::{Path, PathBuf};

    fn scratch(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("perch-gal-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    /// The gecko pack as the marketplace would serve it, and its hash.
    fn published(root: &Path) -> (Vec<u8>, String) {
        let packs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../packs/gecko-glass");
        let exported = loader::export(&packs, &root.join("gecko.petpack")).unwrap();
        (
            std::fs::read(root.join("gecko.petpack")).unwrap(),
            exported.sha256,
        )
    }

    fn marketplace(body: Vec<u8>) -> (Net, String) {
        let (base, _) = serve(move |s| reply(s, "200 OK", "", &body));
        let net = open(Scope::Marketplace);
        net.set(Scope::Marketplace, Mode::Session).unwrap();
        (net, format!("{base}/gecko.petpack"))
    }

    #[test]
    fn a_download_with_the_published_hash_installs_and_records_its_source() {
        let root = scratch("ok");
        let (body, hash) = published(&root);
        let (net, url) = marketplace(body);
        let packs = root.join("packs");
        let summary =
            gallery::download_and_install(&net, &packs, &url, &hash, "gal-42", InstallMode::New)
                .unwrap();
        assert_eq!(summary.id, "com.perch.gecko-glass");
        assert_eq!(
            gallery::sources(&packs)["com.perch.gecko-glass"],
            Source {
                gallery_id: "gal-42".into(),
                version: "1.0.0".into(),
                sha256: hash.clone()
            }
        );
        assert!(!packs.join(".download.petpack").exists());
        gallery::forget_source(&packs, "com.perch.gecko-glass");
        assert!(gallery::sources(&packs).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_download_that_does_not_match_the_hash_installs_nothing() {
        let root = scratch("bad");
        let (body, _) = published(&root);
        let (net, url) = marketplace(body);
        let packs = root.join("packs");
        let wrong = "0".repeat(64);
        let error =
            gallery::download_and_install(&net, &packs, &url, &wrong, "g", InstallMode::New)
                .unwrap_err();
        assert_eq!(error.code, "hash_mismatch");
        assert!(loader::scan(&packs).is_empty());
        assert!(gallery::sources(&packs).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn offline_means_no_download_and_a_clear_code() {
        let root = scratch("offline");
        let net = open(Scope::Marketplace);
        let error = gallery::download_and_install(
            &net,
            &root.join("packs"),
            "http://127.0.0.1:9/x",
            "00",
            "g",
            InstallMode::New,
        )
        .unwrap_err();
        assert_eq!(error.code, "offline");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_error_status_is_reported_not_installed() {
        let root = scratch("status");
        let (base, _) = serve(|s| reply(s, "404 Not Found", "", b"gone"));
        let net = open(Scope::Marketplace);
        net.set(Scope::Marketplace, Mode::Session).unwrap();
        let error = gallery::download_and_install(
            &net,
            &root.join("packs"),
            &format!("{base}/x"),
            "00",
            "g",
            InstallMode::New,
        )
        .unwrap_err();
        assert_eq!(error.code, "http_status");
        let _ = std::fs::remove_dir_all(&root);
    }
}
