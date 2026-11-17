use super::*;
use crate::test_server::TestServer;

fn request<'a>(url: &'a str, sha256: &'a str) -> Request<'a> {
    Request { url, sha256, size: None, max_bytes: 1 << 20, headers: Vec::new() }
}

#[test]
fn only_https_and_test_loopback_pass_the_policy() {
    let strict = UrlPolicy::default();
    assert!(strict.check("https://example.org/a.zip").is_ok());
    assert!(strict.check("http://example.org/a.zip").is_err());
    assert!(strict.check("http://127.0.0.1:8000/a.zip").is_err());
    assert!(strict.check("file:///etc/passwd").is_err());
    let test = UrlPolicy { allow_loopback_http: true };
    assert!(test.check("http://127.0.0.1:8000/a.zip").is_ok());
    assert!(test.check("http://localhost/a").is_ok());
    assert!(test.check("http://127.0.0.1.evil.org/a").is_err());
    assert!(test.check("http://evil.org/127.0.0.1").is_err());
}

#[test]
fn checksums_are_normalised_or_refused() {
    let hash = "AB".repeat(32);
    assert_eq!(normalise_sha256(&format!("sha256:{hash}")).unwrap(), "ab".repeat(32));
    assert!(normalise_sha256("abc").is_err());
    assert!(normalise_sha256(&"zz".repeat(32)).is_err());
}

#[test]
fn a_download_with_the_right_hash_lands_and_reports_progress() {
    let server = TestServer::start();
    let body = vec![7u8; 200_000];
    server.put("/a.zip", body.clone());
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("a.zip");
    let hash = sha256_hex(&body);
    let url = server.url("/a.zip");
    let mut last = (0, None);
    let mut req = request(&url, &hash);
    req.size = Some(body.len() as u64);
    TestServer::client().download(&req, &dest, &mut |done, total| last = (done, total), &AtomicBool::new(false)).unwrap();
    assert_eq!(fs::read(&dest).unwrap(), body);
    assert_eq!(last, (200_000, Some(200_000)));
    assert!(!dest.with_extension("part").exists());
}

#[test]
fn a_wrong_hash_leaves_no_file() {
    let server = TestServer::start();
    server.put("/a.zip", b"evil bytes".to_vec());
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("a.zip");
    let url = server.url("/a.zip");
    let wrong = sha256_hex(b"good bytes");
    let error = TestServer::client().download(&request(&url, &wrong), &dest, &mut |_, _| {}, &AtomicBool::new(false)).unwrap_err();
    assert!(matches!(error, Error::Hash { .. }), "{error}");
    assert!(!dest.exists());
    assert!(!dest.with_extension("part").exists());
}

#[test]
fn size_limits_and_missing_files_fail() {
    let server = TestServer::start();
    server.put("/big", vec![0u8; 5000]);
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("big");
    let url = server.url("/big");
    let hash = sha256_hex(&[0u8; 5000]);
    let mut small = request(&url, &hash);
    small.max_bytes = 1000;
    let error = TestServer::client().download(&small, &dest, &mut |_, _| {}, &AtomicBool::new(false)).unwrap_err();
    assert!(matches!(error, Error::Refused(_)), "{error}");
    let mut sized = request(&url, &hash);
    sized.size = Some(4000);
    assert!(TestServer::client().download(&sized, &dest, &mut |_, _| {}, &AtomicBool::new(false)).is_err());
    let gone = server.url("/gone");
    let error = TestServer::client().download(&request(&gone, &hash), &dest, &mut |_, _| {}, &AtomicBool::new(false)).unwrap_err();
    assert!(error.to_string().contains("404"), "{error}");
    assert!(!dest.exists());
}

#[test]
fn a_cancelled_download_stops_and_cleans_up() {
    let server = TestServer::start();
    server.put("/a", vec![1u8; 100_000]);
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("a");
    let url = server.url("/a");
    let hash = sha256_hex(&[1u8; 100_000]);
    let error = TestServer::client().download(&request(&url, &hash), &dest, &mut |_, _| {}, &AtomicBool::new(true)).unwrap_err();
    assert!(error.to_string().contains("cancelled"));
    assert!(!dest.with_extension("part").exists());
}

#[test]
fn plain_http_is_refused_before_any_request() {
    let server = TestServer::start();
    server.put("/a", b"x".to_vec());
    let strict = Client::new(UrlPolicy::default());
    assert!(matches!(strict.get_bytes(&server.url("/a"), &[]), Err(Error::Refused(_))));
    assert!(server.seen.lock().unwrap().is_empty());
}
