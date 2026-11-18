use serde_json::json;

use super::text::{self, Span};
use super::*;
use crate::test_server::TestServer;

const PATH: &str = "/v1/news?lang=en&limit=10";

fn feed(n: usize) -> String {
    let items: Vec<_> = (0..n)
        .map(|i| json!({"id": format!("n{i}"), "title": format!("Title {i}"), "body": "Hello **world**", "date": "2026-11-01", "tag": "release"}))
        .collect();
    json!({ "items": items }).to_string()
}

#[test]
fn good_data_reads_with_every_field() {
    let body = json!({"items": [
        {"id": "a", "title": "Copper age", "body": "Smelt **more**.\n\nSee [the notes](https://castanea.xenoo.dev/n).",
         "date": "2026-11-01T10:00:00Z", "tag": "release", "url": "https://castanea.xenoo.dev/news/a"},
        {"id": "b", "title": "No extras"}
    ]});
    let items = parse(body.to_string().as_bytes()).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!((items[0].id.as_str(), items[0].tag.as_str(), items[0].day()), ("a", "release", "2026-11-01"));
    assert_eq!(items[0].url.as_deref(), Some("https://castanea.xenoo.dev/news/a"));
    assert_eq!((items[1].body.as_str(), items[1].url.clone()), ("", None));
    let lines = text::lines(&items[0].body);
    assert_eq!(lines.len(), 3, "a gap line between the paragraphs: {lines:?}");
    assert_eq!(lines[0], [Span::Text("Smelt ".into()), Span::Bold("more".into()), Span::Text(".".into())]);
    assert!(matches!(&lines[2][1], Span::Link { text, url } if text == "the notes" && url.starts_with("https://")));
}

#[test]
fn huge_data_is_cut_to_the_limits() {
    let long = "x".repeat(100_000);
    let many: Vec<_> = (0..500)
        .map(|i| json!({"id": format!("i{i}"), "title": long, "body": long, "tag": long, "date": long}))
        .collect();
    let items = parse(json!({ "items": many }).to_string().as_bytes()).unwrap();
    assert_eq!(items.len(), MAX_ITEMS);
    let item = &items[0];
    assert_eq!(item.title.chars().count(), MAX_TITLE);
    assert_eq!(item.body.chars().count(), MAX_BODY);
    assert_eq!((item.tag.chars().count(), item.date.chars().count()), (MAX_TAG, MAX_DATE));
    let id = parse(json!({"items": [{"id": long, "title": "t"}]}).to_string().as_bytes()).unwrap();
    assert_eq!(id[0].id.chars().count(), MAX_ID);
}

#[test]
fn an_answer_over_the_size_limit_is_refused_and_the_cache_stays() {
    let server = TestServer::start();
    let dir = tempfile::tempdir().unwrap();
    server.put(PATH, feed(3));
    assert_eq!(sync(dir.path(), &server.base, "en").unwrap().len(), 3);
    let huge = json!({"items": [{"id": "a", "title": "t", "body": "y".repeat(MAX_BYTES as usize)}]}).to_string();
    server.put(PATH, huge);
    let error = sync(dir.path(), &server.base, "en").unwrap_err().to_string();
    assert!(error.contains("larger than"), "{error}");
    assert_eq!(load(dir.path(), "en").items.len(), 3);
}

#[test]
fn bad_json_and_wrong_shapes_are_errors_not_panics() {
    let deep = "[".repeat(10_000);
    for bad in ["", "{", "[]", "null", "42", r#"{"items": 3}"#, r#"{"items": {"a": 1}}"#, "\u{0}\u{1}", deep.as_str()] {
        assert!(parse(bad.as_bytes()).is_err(), "{bad:.20}");
    }
    let odd = json!({"items": [null, 1, "x", [], {"title": 5}, {"title": ""}, {"title": "  "}, {"title": "ok", "id": 7, "body": ["a"]}]});
    let items = parse(odd.to_string().as_bytes()).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!((items[0].title.as_str(), items[0].id.as_str(), items[0].body.as_str()), ("ok", "#7", ""));
    let twice = json!({"items": [{"id": "a", "title": "one"}, {"id": "a", "title": "two"}]});
    assert_eq!(parse(twice.to_string().as_bytes()).unwrap().len(), 1, "an id is kept once");
}

#[test]
fn text_is_cleaned_and_only_https_links_survive() {
    let nasty = json!({"items": [{"id": "x", "title": "Hi\u{202e}there\u{0}\n!", "body": "a\u{7}b\tc\r\nd",
        "url": "http://example.com/", "tag": "t\u{1b}[31m"}, {"id": "y", "title": "y", "url": "javascript:alert(1)"},
        {"id": "z", "title": "z", "url": "https://user@evil.example/"}, {"id": "w", "title": "w", "url": "https:// spaced"}]});
    let items = parse(nasty.to_string().as_bytes()).unwrap();
    assert_eq!(items[0].title, "Hithere!");
    assert_eq!(items[0].body, "ab c\nd");
    assert_eq!(items[0].tag, "t[31m");
    assert!(items.iter().all(|i| i.url.is_none()), "{items:?}");
    for url in [
        "http://a.b/",
        "ftp://a.b",
        "file:///etc/passwd",
        "//a.b",
        "https://",
        "https:///x",
        "https://a b/",
        "data:text/html,x",
    ] {
        assert_eq!(text::safe_url(url), None, "{url}");
    }
    assert!(text::safe_url("https://castanea.xenoo.dev/a?b=1#c").is_some());
    assert!(text::safe_url(&format!("https://a.b/{}", "x".repeat(600))).is_none());
    let lines = text::lines("[x](javascript:alert(1)) and [y](http://a.b) and <b>raw</b> **open [z](https://a.b/z)");
    assert_eq!(lines.len(), 1);
    assert!(lines[0].iter().all(|s| !matches!(s, Span::Link { url, .. } if !url.starts_with("https://"))));
    assert!(lines[0].iter().any(|s| s.text().contains("<b>raw</b>")), "markup stays literal text");
    assert!(!lines[0].iter().any(|s| matches!(s, Span::Bold(_))), "an unclosed ** is literal");
}

#[test]
fn the_text_forms_edge_cases() {
    assert_eq!(text::lines("****"), [vec![Span::Text("****".into())]]);
    assert_eq!(text::lines("[](https://a.b)"), [vec![Span::Text("[](https://a.b)".into())]]);
    assert_eq!(
        text::lines("\n\n a \n\n\n b \n\n"),
        [vec![Span::Text("a".into())], vec![], vec![Span::Text("b".into())]]
    );
    let (clipped, cut) = text::clip(&text::lines("one **two** three four"), 8);
    assert!(cut);
    assert_eq!(clipped, [vec![Span::Text("one ".into()), Span::Bold("two".into()), Span::Text("…".into())]]);
    let (whole, cut) = text::clip(&text::lines("short"), 100);
    assert!(!cut && whole.len() == 1);
    let many = text::lines(&"[a](https://a.b) ".repeat(500));
    assert!(!many.is_empty());
}

#[test]
fn the_server_sends_the_etag_back_and_a_304_keeps_the_cache() {
    let server = TestServer::start();
    let dir = tempfile::tempdir().unwrap();
    server.put(PATH, feed(2));
    assert_eq!(sync(dir.path(), &server.base, "en").unwrap().len(), 2);
    assert!(load(dir.path(), "en").etag.is_some());
    assert_eq!(sync(dir.path(), &server.base, "en").unwrap().len(), 2);
    let seen = server.seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert!(!seen[0].1.iter().any(|(k, _)| k == "if-none-match"));
    assert!(seen[1].1.iter().any(|(k, _)| k == "if-none-match"), "{:?}", seen[1].1);
    assert!(seen[0].0.contains("lang=en") && seen[0].0.contains("limit=10"));
}

#[test]
fn the_languages_have_their_own_cache_and_an_unknown_one_is_english() {
    let server = TestServer::start();
    let dir = tempfile::tempdir().unwrap();
    server.put("/v1/news?lang=de&limit=10", feed(1));
    server.put(PATH, feed(2));
    assert_eq!(sync(dir.path(), &server.base, "de").unwrap().len(), 1);
    assert_eq!(sync(dir.path(), &server.base, "../../x").unwrap().len(), 2);
    assert_eq!(load(dir.path(), "de").items.len(), 1);
    assert_eq!(load(dir.path(), "en").items.len(), 2);
    let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names.len(), 2, "{names:?}");
}

#[test]
fn offline_the_saved_feed_is_still_there_and_errors_name_the_problem() {
    let server = TestServer::start();
    let dir = tempfile::tempdir().unwrap();
    server.put(PATH, feed(4));
    sync(dir.path(), &server.base, "en").unwrap();
    let gone = server.base.clone();
    drop(server);
    assert!(sync(dir.path(), &gone, "en").is_err(), "nobody listens any more");
    assert_eq!(load(dir.path(), "en").items.len(), 4, "the saved feed survives a failed request");
    let other = TestServer::start();
    let error = sync(dir.path(), &other.base, "en").unwrap_err().to_string();
    assert!(error.contains("404"), "{error}");
    assert_eq!(load(dir.path(), "en").items.len(), 4);
}

#[test]
fn a_damaged_or_oversized_cache_file_reads_as_empty() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("en.json");
    for bad in ["", "{", "[1]", r#"{"items": "x"}"#] {
        std::fs::write(&file, bad).unwrap();
        assert_eq!(load(dir.path(), "en"), Cache::default(), "{bad}");
    }
    std::fs::write(&file, vec![b'a'; (2 * MAX_BYTES + 1) as usize]).unwrap();
    assert_eq!(load(dir.path(), "en"), Cache::default());
    std::fs::write(&file, r#"{"etag": "a\nb", "items": [{"title": "kept", "id": "k"}]}"#).unwrap();
    let cache = load(dir.path(), "en");
    assert_eq!((cache.etag, cache.items.len()), (None, 1), "a header-unsafe etag is dropped");
}

#[test]
fn a_slow_server_gives_up_after_the_deadline() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let start = std::time::Instant::now();
    let error = request(&base, "en", &Cache::default());
    assert!(error.is_err());
    assert!(start.elapsed() < TIMEOUT + Duration::from_secs(3), "{:?}", start.elapsed());
    drop(listener);
}

#[test]
fn only_https_servers_are_asked() {
    assert!(request("http://news.example.com", "en", &Cache::default()).is_err());
}
