//! A local line-based chat partner, so the example runs without a server.
use std::{
    io::{self, BufRead, BufReader, Write},
    net::{SocketAddr, TcpListener},
    thread,
};

/// Listen on a free local port and answer one client on a background thread.
pub fn start() -> io::Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    thread::Builder::new().name("bot".into()).spawn(move || {
        let Ok((stream, _)) = listener.accept() else {
            return;
        };
        let _ = serve(stream);
    })?;
    Ok(addr)
}

fn serve(stream: std::net::TcpStream) -> io::Result<()> {
    let mut out = io::BufWriter::new(stream.try_clone()?);
    writeln!(out, "bot: hi! try /flood 10000 or /help")?;
    out.flush()?;
    for line in BufReader::new(stream).lines() {
        let line = line?;
        match line.split_once(' ').unwrap_or((&line, "")) {
            ("/help", _) => {
                writeln!(out, "bot: /flood N sends N lines as fast as it can")?;
            }
            ("/flood", count) => {
                let count: usize = count.trim().parse().unwrap_or(1000);
                for i in 1..=count {
                    writeln!(out, "bot: flood {i}/{count}")?;
                }
            }
            _ => writeln!(out, "bot: you said {:?}", line)?,
        }
        out.flush()?;
    }
    Ok(())
}
