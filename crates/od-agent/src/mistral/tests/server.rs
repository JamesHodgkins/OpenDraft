//! A tiny HTTP server on the loopback address, answering from a script, so
//! the real client code (headers, body, status handling, timeouts) runs
//! without the network or a key.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Seen {
    pub request_line: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Seen {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

pub struct Server {
    pub url: String,
    pub seen: Arc<Mutex<Vec<Seen>>>,
}

/// What the server does with one connection.
pub enum Reply {
    Answer(u16, String),
    /// Reads the request and says nothing for this long.
    Silence(Duration),
}

pub fn answer(status: u16, body: &str) -> Reply {
    Reply::Answer(status, body.to_owned())
}

/// A server that handles one connection per reply, in order.
pub fn serve(replies: Vec<Reply>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let url = format!("http://{}", listener.local_addr().expect("an address"));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    thread::spawn(move || {
        for reply in replies {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let request = read_request(&mut stream);
            if let Ok(mut seen) = log.lock() {
                seen.push(request);
            }
            match reply {
                Reply::Answer(status, body) => {
                    let text = format!(
                        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(text.as_bytes());
                }
                Reply::Silence(wait) => thread::sleep(wait),
            }
        }
    });
    Server { url, seen }
}

/// A port nothing listens on.
pub fn closed_port() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let url = format!("http://{}", listener.local_addr().expect("an address"));
    drop(listener);
    url
}

fn read_request(stream: &mut std::net::TcpStream) -> Seen {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 4096];
    let (head_end, length) = loop {
        let n = stream.read(&mut buffer).unwrap_or(0);
        if n == 0 {
            break (bytes.len(), 0);
        }
        bytes.extend_from_slice(&buffer[..n]);
        if let Some(end) = find(&bytes, b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&bytes[..end]).to_string();
            let length = head
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(str::to_owned)
                })
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            break (end + 4, length);
        }
    };
    while bytes.len() < head_end + length {
        let n = stream.read(&mut buffer).unwrap_or(0);
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..n]);
    }
    let head =
        String::from_utf8_lossy(&bytes[..head_end.saturating_sub(4).min(bytes.len())]).to_string();
    let mut lines = head.lines();
    let request_line = lines.next().unwrap_or_default().to_owned();
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect();
    let body = String::from_utf8_lossy(&bytes[head_end.min(bytes.len())..]).to_string();
    Seen {
        request_line,
        headers,
        body,
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}
