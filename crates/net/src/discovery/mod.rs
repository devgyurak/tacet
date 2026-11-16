//! 내부망 mDNS로 서버를 알리고 찾는다(설계 §5.1). TXT에 인증서 지문 앞부분을 실어 사람이 대조할 수 있게 한다.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::NetError;
use crate::pairing::Fingerprint;

#[cfg(test)]
mod tests;

/// mDNS 서비스 형식.
pub const SERVICE_TYPE: &str = "_tacet._tcp.local.";

fn mdns_err(e: mdns_sd::Error) -> NetError {
    NetError::Io(std::io::Error::other(e.to_string()))
}

/// 서버 알림. 버리면 알림을 멈춘다.
pub struct Advertiser {
    daemon: ServiceDaemon,
}

impl std::fmt::Debug for Advertiser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Advertiser")
    }
}

impl Drop for Advertiser {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown();
    }
}

/// 서버를 알린다.
pub fn advertise(
    instance: &str,
    port: u16,
    fingerprint: &Fingerprint,
) -> Result<Advertiser, NetError> {
    let daemon = ServiceDaemon::new().map_err(mdns_err)?;
    let props = [("fp", fingerprint.short())];
    let info = ServiceInfo::new(
        SERVICE_TYPE,
        instance,
        "tacet-server.local.",
        "",
        port,
        &props[..],
    )
    .map_err(mdns_err)?
    .enable_addr_auto();
    daemon.register(info).map_err(mdns_err)?;
    Ok(Advertiser { daemon })
}

/// 찾은 서버.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundServer {
    /// 인스턴스 이름.
    pub name: String,
    /// 주소.
    pub addr: SocketAddr,
    /// 지문 앞 16자리(사람이 서버 화면과 대조).
    pub fingerprint_short: Option<String>,
}

/// `wait` 동안 서버를 찾는다.
pub fn discover(wait: Duration) -> Result<Vec<FoundServer>, NetError> {
    let daemon = ServiceDaemon::new().map_err(mdns_err)?;
    let rx = daemon.browse(SERVICE_TYPE).map_err(mdns_err)?;
    let deadline = Instant::now() + wait;
    let mut found = Vec::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let Ok(event) = rx.recv_timeout(left) else {
            break;
        };
        if let ServiceEvent::ServiceResolved(service) = event {
            let mut addrs: Vec<_> = service.get_addresses_v4().into_iter().collect();
            addrs.sort();
            if let Some(ip) = addrs.first() {
                found.push(FoundServer {
                    name: service.fullname.clone(),
                    addr: SocketAddr::new((*ip).into(), service.get_port()),
                    fingerprint_short: service.get_property_val_str("fp").map(str::to_owned),
                });
            }
        }
    }
    let _ = daemon.shutdown();
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found.dedup();
    Ok(found)
}
