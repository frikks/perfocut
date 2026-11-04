//! PerfoCut для Windows: локальный сервер с встроенным интерфейсом.
//! Открывает интерфейс в Microsoft Edge в режиме приложения (без адресной строки)
//! или в браузере по умолчанию.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::thread;

static INDEX: &[u8] = include_bytes!("../../dist/PerfoCut.html");

fn respond(mut s: TcpStream) {
    let mut buf = [0u8; 4096];
    let n = match s.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return,
    };
    let req = String::from_utf8_lossy(&buf[..n]);
    let path = req.split_whitespace().nth(1).unwrap_or("/");
    let (status, ctype, body): (&str, &str, &[u8]) = if path == "/" || path.starts_with("/index") || path.starts_with("/?") {
        ("200 OK", "text/html; charset=utf-8", INDEX)
    } else {
        ("404 Not Found", "text/plain; charset=utf-8", b"not found")
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = s.write_all(head.as_bytes());
    let _ = s.write_all(body);
    let _ = s.flush();
}

fn open_ui(url: &str) {
    #[cfg(windows)]
    {
        let mut candidates = Vec::new();
        for var in ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"] {
            if let Ok(base) = std::env::var(var) {
                candidates.push(format!("{base}\\Microsoft\\Edge\\Application\\msedge.exe"));
            }
        }
        for c in candidates {
            if std::path::Path::new(&c).exists() {
                if Command::new(&c).arg(format!("--app={url}")).arg("--window-size=1400,900").spawn().is_ok() {
                    return;
                }
            }
        }
        let _ = Command::new("cmd").args(["/C", "start", "", url]).spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("xdg-open").arg(url).spawn();
    }
}

fn main() {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Не удалось запустить: {e}");
            return;
        }
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    let url = format!("http://127.0.0.1:{port}/");
    println!("PerfoCut {}", env!("CARGO_PKG_VERSION"));
    println!("Интерфейс: {url}");
    println!("Закройте это окно, чтобы завершить работу.");
    open_ui(&url);
    for s in listener.incoming().flatten() {
        thread::spawn(move || respond(s));
    }
}
