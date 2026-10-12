//! UDP LAN transport on port 47474 (lolPing-compatible).

use anyhow::{Context, Result};
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub const LAN_PORT: u16 = 47474;

pub struct LanTransport {
    running: Arc<AtomicBool>,
    socket: Arc<UdpSocket>,
}

impl LanTransport {
    pub fn start<F>(port: u16, on_packet: F) -> Result<Self>
    where
        F: Fn(Vec<u8>, String) + Send + 'static,
    {
        let socket = UdpSocket::bind(("0.0.0.0", port))
            .with_context(|| format!("bind UDP {port}"))?;
        socket.set_broadcast(true)?;
        socket.set_read_timeout(Some(Duration::from_millis(200)))?;
        let socket = Arc::new(socket);
        let running = Arc::new(AtomicBool::new(true));
        let sock_r = socket.clone();
        let run_r = running.clone();
        thread::Builder::new()
            .name("lolping-lan".into())
            .spawn(move || {
                let mut buf = [0u8; 2048];
                while run_r.load(Ordering::SeqCst) {
                    match sock_r.recv_from(&mut buf) {
                        Ok((n, addr)) => {
                            on_packet(buf[..n].to_vec(), addr.to_string());
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut => {}
                        Err(e) => {
                            log::debug!("lan recv: {e}");
                        }
                    }
                }
            })?;
        Ok(Self { running, socket })
    }

    pub fn broadcast(&self, packet: &[u8]) {
        let targets = ["255.255.255.255", "127.0.0.1"];
        for host in targets {
            let addr: SocketAddr = format!("{host}:{LAN_PORT}").parse().unwrap();
            let _ = self.socket.send_to(packet, addr);
        }
    }

    pub fn stop(self) {
        self.running.store(false, Ordering::SeqCst);
        // socket dropped after thread times out
    }
}
