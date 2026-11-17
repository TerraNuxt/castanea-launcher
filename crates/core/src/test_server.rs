//! A local HTTP server for tests: fixed files by path, an ETag per file, and a log of request headers.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::download::{Client, UrlPolicy};

/// Each request: its path and its headers (names in lower case).
pub type Seen = Arc<Mutex<Vec<(String, Vec<(String, String)>)>>>;

pub struct TestServer {
    pub base: String,
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    pub seen: Seen,
    server: Arc<tiny_http::Server>,
    thread: Option<thread::JoinHandle<()>>,
}

impl TestServer {
    pub fn start() -> Self {
        let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        let port = server.server_addr().to_ip().unwrap().port();
        let files: Arc<Mutex<HashMap<String, Vec<u8>>>> = Arc::default();
        let seen: Seen = Arc::default();
        let thread = {
            let (server, files, seen) = (server.clone(), files.clone(), seen.clone());
            thread::spawn(move || {
                for request in server.incoming_requests() {
                    let path = request.url().to_owned();
                    let headers: Vec<(String, String)> = request
                        .headers()
                        .iter()
                        .map(|h| (h.field.to_string().to_ascii_lowercase(), h.value.to_string()))
                        .collect();
                    seen.lock().unwrap().push((path.clone(), headers.clone()));
                    let body = files.lock().unwrap().get(&path).cloned();
                    let response = match body {
                        None => tiny_http::Response::from_data(b"missing".to_vec()).with_status_code(404),
                        Some(bytes) => {
                            let etag = format!("\"{}\"", &crate::download::sha256_hex(&bytes)[..16]);
                            let matches = headers.iter().any(|(k, v)| k == "if-none-match" && *v == etag);
                            let header = tiny_http::Header::from_bytes("ETag", etag.as_bytes()).unwrap();
                            if matches {
                                tiny_http::Response::from_data(Vec::new()).with_status_code(304).with_header(header)
                            } else {
                                tiny_http::Response::from_data(bytes).with_header(header)
                            }
                        }
                    };
                    let _ = request.respond(response);
                }
            })
        };
        Self { base: format!("http://127.0.0.1:{port}"), files, seen, server, thread: Some(thread) }
    }

    pub fn put(&self, path: &str, bytes: impl Into<Vec<u8>>) {
        self.files.lock().unwrap().insert(path.to_owned(), bytes.into());
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    pub fn client() -> Client {
        Client::new(UrlPolicy { allow_loopback_http: true })
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.server.unblock();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
