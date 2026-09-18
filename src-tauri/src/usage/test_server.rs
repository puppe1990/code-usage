//! Minimal HTTP/1.1 server for tests: serves one canned response per request, in order, so the
//! limit fetchers can be exercised without hitting real endpoints.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

/// Binds a loopback server that replies with `responses` in order, returning its base URL.
pub(crate) fn spawn(responses: Vec<(u16, String)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let address = listener
        .local_addr()
        .expect("test server address")
        .to_string();

    thread::spawn(move || {
        for (status, body) in responses {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            read_request(&mut stream);
            let _ = stream.write_all(&response(status, &body));
            let _ = stream.flush();
        }
    });

    format!("http://{address}")
}

fn read_request(stream: &mut std::net::TcpStream) {
    let mut buffer = [0u8; 4096];
    let _ = stream.read(&mut buffer);
}

fn response(status: u16, body: &str) -> Vec<u8> {
    let reason = if status == 200 { "OK" } else { "Error" };
    format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}
