//! 서버 쪽: 센서와 감독관 뷰의 연결을 받는다.
//!
//! 서버는 센서 보고서를 신뢰하지 않는다(AGENTS.md §2 규칙 10). 페어링되지 않은 연결은 거부하고, 실패가
//! 쌓인 출처는 잠시 잠그고, 보고서의 센서 ID는 연결의 센서 ID로 덮어쓰고, 형식이 틀린 줄과 서버 시각보다
//! 너무 앞선 보고서는 버리고 알린다. 센서와 감독관 뷰는 서로 다른 페어링 코드를 쓴다.

use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tacet_core::ids::SensorId;
use tacet_core::obs::SensorInfo;
use tacet_core::view::BuildingView;
use tokio::io::{AsyncWrite, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use tokio_rustls::TlsAcceptor;

use crate::NetError;
use crate::frame::{read_frame, write_frame};
use crate::pairing::{Fingerprint, verify};
use crate::protocol::{
    ClientHello, Downlink, Rejection, Role, ServerHello, SessionInfo, Uplink, VERSION,
};
use crate::tls::ServerIdentity;

#[cfg(test)]
mod tests;

/// 핸드셰이크와 첫 메시지 시간 제한.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
/// 센서가 이만큼 아무것도 안 보내면(하트비트 포함) 연결을 끝낸다. 반쯤 열린 옛 연결이 남지 않게.
pub const SENSOR_IDLE_TIMEOUT: Duration = Duration::from_secs(90);
/// 기본 페어링 실패 잠금 시간. 잠금은 출처 IP별이고 이 시간이 지나면 풀린다.
pub const DEFAULT_LOCKOUT: Duration = Duration::from_secs(300);
/// 센서 시각이 서버 시각보다 이만큼 넘게 앞서면 그 보고서를 버린다(NTP 어긋남·조작 방어).
pub const MAX_CLOCK_AHEAD_MS: u64 = 10_000;

/// 서버 설정. `Debug`는 페어링 코드를 가린다 — 로그에 찍혀도 새지 않게.
#[derive(Clone)]
pub struct ServerSettings {
    /// 들을 주소(내부망 인터페이스).
    pub bind: SocketAddr,
    /// 센서 페어링 코드.
    pub sensor_code: String,
    /// 감독관 뷰 페어링 코드. 센서 코드와 따로 둔다.
    pub viewer_code: String,
    /// 세션 시작(유닉스 밀리초).
    pub session_start_epoch_ms: u64,
    /// 한 출처가 이만큼 페어링에 실패하면 그 출처를 잠시 잠근다.
    pub max_failures: u32,
    /// 잠금 시간. 마지막 실패에서 이만큼 지나면 풀린다.
    pub lockout: Duration,
}

impl std::fmt::Debug for ServerSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerSettings")
            .field("bind", &self.bind)
            .field("sensor_code", &"<가림>")
            .field("viewer_code", &"<가림>")
            .field("session_start_epoch_ms", &self.session_start_epoch_ms)
            .field("max_failures", &self.max_failures)
            .field("lockout", &self.lockout)
            .finish()
    }
}

/// 서버가 앱에 알리는 사건.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerEvent {
    /// 센서가 연결됐다.
    SensorJoined(SensorInfo),
    /// 센서가 보낸 것(보고서의 센서 ID는 이미 연결의 센서 ID다).
    Uplink(SensorId, Uplink),
    /// 센서 연결이 끊겼다.
    SensorLeft(SensorId),
    /// 형식이 틀린 메시지를 버렸다.
    MalformedDropped(SensorId),
    /// 센서 시각이 서버보다 너무 앞서 보고서를 버렸다(앞선 밀리초).
    ClockAhead(SensorId, u64),
    /// 연결을 거부했다.
    Rejected(Rejection),
}

/// 실행 중인 서버.
#[derive(Debug, Clone)]
pub struct ServerHandle {
    addr: SocketAddr,
    fingerprint: Fingerprint,
    views: Arc<watch::Sender<BuildingView>>,
}

impl ServerHandle {
    /// 실제로 들은 주소.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// 서버 인증서 지문.
    pub fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    /// 감독관 뷰들에 새 화면을 보낸다.
    pub fn publish(&self, view: BuildingView) {
        self.views.send_replace(view);
    }
}

#[derive(Debug)]
struct Shared {
    settings: ServerSettings,
    fingerprint: Fingerprint,
    failures: Mutex<BTreeMap<IpAddr, (u32, Instant)>>,
    /// 센서별 현재 연결 세대. 같은 센서가 다시 붙으면 세대가 올라가 옛 연결이 스스로 끝난다.
    sensors: Mutex<BTreeMap<SensorId, u64>>,
    events: mpsc::Sender<ServerEvent>,
    views: Arc<watch::Sender<BuildingView>>,
}

impl Shared {
    fn locked(&self, ip: IpAddr) -> bool {
        let Ok(mut failures) = self.failures.lock() else {
            return true;
        };
        match failures.get(&ip).copied() {
            Some((_, since)) if since.elapsed() >= self.settings.lockout => {
                failures.remove(&ip);
                false
            }
            Some((count, _)) => count >= self.settings.max_failures,
            None => false,
        }
    }

    fn record_failure(&self, ip: IpAddr) {
        if let Ok(mut failures) = self.failures.lock() {
            // 잠금이 끝난 출처는 지운다 — 실패 기록이 출처 수만큼 계속 쌓이지 않게.
            let lockout = self.settings.lockout;
            failures.retain(|_, (_, since)| since.elapsed() < lockout);
            let entry = failures.entry(ip).or_insert((0, Instant::now()));
            entry.0 += 1;
            entry.1 = Instant::now();
        }
    }

    fn generation(&self, id: &SensorId) -> Option<u64> {
        self.sensors.lock().ok().and_then(|s| s.get(id).copied())
    }
}

/// 서버를 띄운다. 사건은 돌려준 수신기로 받는다.
pub async fn serve(
    settings: ServerSettings,
    identity: &ServerIdentity,
) -> Result<(ServerHandle, mpsc::Receiver<ServerEvent>), NetError> {
    let acceptor = TlsAcceptor::from(identity.server_config()?);
    let listener = TcpListener::bind(settings.bind).await?;
    let addr = listener.local_addr()?;
    let (events, rx) = mpsc::channel(1024);
    let views = Arc::new(watch::channel(BuildingView::default()).0);
    let shared = Arc::new(Shared {
        settings,
        fingerprint: identity.fingerprint(),
        failures: Mutex::new(BTreeMap::new()),
        sensors: Mutex::new(BTreeMap::new()),
        events,
        views: views.clone(),
    });
    tokio::spawn(async move {
        while let Ok((stream, peer)) = listener.accept().await {
            let shared = shared.clone();
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                // 연결 하나의 실패는 그 연결만 끝낸다.
                let _ = handle(stream, peer.ip(), acceptor, shared).await;
            });
        }
    });
    Ok((
        ServerHandle {
            addr,
            fingerprint: identity.fingerprint(),
            views,
        },
        rx,
    ))
}

async fn reply<W: AsyncWrite + Unpin>(
    writer: &mut W,
    shared: &Shared,
    hello: ServerHello,
) -> Result<(), NetError> {
    if let ServerHello::Rejected(reason) = hello {
        let _ = shared.events.send(ServerEvent::Rejected(reason)).await;
    }
    write_frame(writer, &hello).await
}

async fn handle(
    stream: TcpStream,
    peer: IpAddr,
    acceptor: TlsAcceptor,
    shared: Arc<Shared>,
) -> Result<(), NetError> {
    let tls = tokio::time::timeout(HELLO_TIMEOUT, acceptor.accept(stream))
        .await
        .map_err(|_| NetError::Timeout)??;
    let (read_half, mut writer) = tokio::io::split(tls);
    let mut reader = BufReader::new(read_half);
    let hello: ClientHello = tokio::time::timeout(HELLO_TIMEOUT, read_frame(&mut reader))
        .await
        .map_err(|_| NetError::Timeout)??
        .ok_or(NetError::Closed)?;

    if hello.version != VERSION {
        return reply(
            &mut writer,
            &shared,
            ServerHello::Rejected(Rejection::BadVersion),
        )
        .await;
    }
    if shared.locked(peer) {
        return reply(
            &mut writer,
            &shared,
            ServerHello::Rejected(Rejection::LockedOut),
        )
        .await;
    }
    let code = match hello.role {
        Role::Sensor(_) => &shared.settings.sensor_code,
        Role::Viewer => &shared.settings.viewer_code,
    };
    if !verify(code, &shared.fingerprint, &hello.role.tag(), &hello.proof) {
        shared.record_failure(peer);
        return reply(
            &mut writer,
            &shared,
            ServerHello::Rejected(Rejection::BadPairing),
        )
        .await;
    }
    let session = SessionInfo {
        start_epoch_ms: shared.settings.session_start_epoch_ms,
    };
    match hello.role {
        Role::Viewer => {
            reply(&mut writer, &shared, ServerHello::Accepted(session)).await?;
            let mut views = shared.views.subscribe();
            loop {
                let view = views.borrow_and_update().clone();
                write_frame(&mut writer, &Downlink::View(view)).await?;
                if views.changed().await.is_err() {
                    return Ok(());
                }
            }
        }
        Role::Sensor(info) => {
            // 같은 센서가 다시 붙으면(네트워크 끊김 뒤 재접속) 새 연결을 받고 옛 연결은 다음 메시지에서 스스로 끝난다.
            let generation = shared
                .sensors
                .lock()
                .map(|mut s| {
                    let next = s.get(&info.id).map_or(1, |g| g + 1);
                    s.insert(info.id.clone(), next);
                    next
                })
                .unwrap_or(0);
            let id = info.id.clone();
            let result =
                sensor_loop(&mut reader, &mut writer, &shared, info, session, generation).await;
            let still_current = shared.generation(&id) == Some(generation);
            if still_current {
                if let Ok(mut sensors) = shared.sensors.lock() {
                    sensors.remove(&id);
                }
                let _ = shared.events.send(ServerEvent::SensorLeft(id)).await;
            }
            result
        }
    }
}

/// 세션 시작 기준 지금 시각(ms).
fn session_now_ms(start_epoch_ms: u64) -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
    now.saturating_sub(start_epoch_ms)
}

async fn sensor_loop<R, W>(
    reader: &mut R,
    writer: &mut W,
    shared: &Shared,
    info: SensorInfo,
    session: SessionInfo,
    generation: u64,
) -> Result<(), NetError>
where
    R: tokio::io::AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    reply(writer, shared, ServerHello::Accepted(session)).await?;
    let id = info.id.clone();
    let _ = shared.events.send(ServerEvent::SensorJoined(info)).await;
    loop {
        let frame = tokio::time::timeout(SENSOR_IDLE_TIMEOUT, read_frame::<_, Uplink>(reader))
            .await
            .map_err(|_| NetError::Timeout)?;
        if shared.generation(&id) != Some(generation) {
            return Ok(());
        }
        match frame {
            Ok(Some(Uplink::Report(mut report))) => {
                let now = session_now_ms(session.start_epoch_ms);
                let at = report.obs.at().0;
                if at > now.saturating_add(MAX_CLOCK_AHEAD_MS) {
                    let _ = shared
                        .events
                        .send(ServerEvent::ClockAhead(id.clone(), at - now))
                        .await;
                    continue;
                }
                report.sensor = id.clone();
                let _ = shared
                    .events
                    .send(ServerEvent::Uplink(id.clone(), Uplink::Report(report)))
                    .await;
            }
            Ok(Some(Uplink::Heartbeat)) => {}
            Ok(Some(status @ Uplink::Status(..))) => {
                let _ = shared
                    .events
                    .send(ServerEvent::Uplink(id.clone(), status))
                    .await;
            }
            Ok(None) => return Ok(()),
            Err(NetError::Malformed(_)) => {
                let _ = shared
                    .events
                    .send(ServerEvent::MalformedDropped(id.clone()))
                    .await;
            }
            Err(other) => return Err(other),
        }
    }
}
