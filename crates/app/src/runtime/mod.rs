//! 모드별 배선. 판정은 [`Hub`](crate::hub::Hub)(core 엔진)가 하고, 여기는 수집기·통신·화면을 잇는다.
//!
//! 비동기 작업은 백그라운드 tokio 런타임에서 돌고, 화면(Electron, `desktop/`)은 [`crate::ipc`]로 [`UiModel`]을 받아 그린다.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Context as _;
use rand::RngCore;
use tacet_core::config::RoomLayout;
use tacet_core::explain::Severity;
use tacet_core::ids::{Platform, SensorId};
use tacet_core::obs::{CollectorEvent, SensorReport};
use tacet_core::report::SessionSalt;
use tacet_core::time::{ClockFn, Timestamp};
use tacet_core::view::{BuildingView, sensor_only_view};
use tacet_dashboard::DashboardState;
use tacet_net::{Fingerprint, Rejection, ServerEvent, ServerIdentity, ServerSettings, Uplink};
use tokio::sync::{mpsc, watch};

use crate::config::{AppConfig, Mode};
use crate::hub::Hub;

#[cfg(test)]
mod tests;

/// 상태 한 줄과 그 무게.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StatusLine {
    /// 무게(화면 색).
    pub severity: Severity,
    /// 쉬운 말 문구.
    pub text: String,
}

impl StatusLine {
    fn new(severity: Severity, text: impl Into<String>) -> Self {
        Self {
            severity,
            text: text.into(),
        }
    }
}

/// 화면이 읽는 모델. JSON으로 화면(Electron)에 보낸다.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct UiModel {
    /// 모드.
    pub mode: Mode,
    /// 모드 이름(화면 표시용).
    pub mode_label: String,
    /// 건물(또는 이 센서) 화면.
    pub view: BuildingView,
    /// 개발자 정보 줄(페어링 코드, 서버 지문, 대시보드 주소 등). 개발자 화면에만 보인다.
    pub header: Vec<String>,
    /// 연결 상태.
    pub connection: Option<StatusLine>,
    /// 최근 알림 기록(기술 용어).
    pub log: Vec<String>,
    /// 마지막 알림(보고서 저장 결과 등).
    pub notice: Option<String>,
    /// 보고서를 저장할 수 있는지(판정을 하는 단독·서버 모드).
    pub can_save_report: bool,
}

impl UiModel {
    /// 모드의 빈 모델.
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            mode_label: mode.label().to_owned(),
            view: BuildingView::default(),
            header: Vec::new(),
            connection: None,
            log: Vec::new(),
            notice: None,
            can_save_report: matches!(mode, Mode::Standalone | Mode::Server),
        }
    }
}

/// 화면과 공유하는 모델.
pub type Shared = Arc<Mutex<UiModel>>;

/// 실행 중인 앱의 손잡이.
#[derive(Debug)]
pub struct Handles {
    /// 화면 모델.
    pub model: Shared,
    /// 판정 허브(단독·서버 모드).
    pub hub: Option<Arc<Mutex<Hub>>>,
    runtime: Option<tokio::runtime::Runtime>,
}

impl Drop for Handles {
    fn drop(&mut self) {
        // 수집기의 블로킹 스캔(수 초 잠듦)을 기다리지 않고 끝낸다.
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

/// 수집기를 어떻게 띄울지. 테스트는 하드웨어를 쓰지 않는다(AGENTS.md §5.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Collectors {
    /// 이 컴퓨터의 BLE·Wi-Fi·SDR 수집기를 띄운다.
    Real,
    /// 띄우지 않는다.
    None,
    /// 정해 둔 사건 열을 수집기 대신 1초마다 되풀이해 흘려보낸다(장비 없는 시연·교육·테스트).
    Replay(Vec<CollectorEvent>),
}

fn update(model: &Shared, f: impl FnOnce(&mut UiModel)) {
    if let Ok(mut guard) = model.lock() {
        f(&mut guard);
    }
}

/// 지금(유닉스 밀리초).
pub fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn this_platform() -> Platform {
    if cfg!(windows) {
        Platform::Windows
    } else if cfg!(target_os = "macos") {
        Platform::MacOs
    } else {
        Platform::Linux
    }
}

/// 세션 솔트. 세션마다 새로 만들고 저장하지 않는다(설계 §8).
pub fn new_salt() -> SessionSalt {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    SessionSalt(bytes)
}

/// 수집기 사건을 허브에 넣는다(단독 모드).
pub fn apply_collector_event(hub: &mut Hub, sensor: &SensorId, event: CollectorEvent) {
    match event {
        CollectorEvent::Obs(obs) => {
            hub.ingest_local(sensor, obs);
        }
        CollectorEvent::Status(kind, status) => hub.set_status(sensor, kind, status),
    }
}

/// 서버 사건을 허브에 넣고, 기록할 줄이 있으면 돌려준다(서버 모드).
pub fn apply_server_event(hub: &mut Hub, event: ServerEvent) -> Option<String> {
    match event {
        ServerEvent::SensorJoined(info) => {
            hub.set_connected(&info.id, true);
            Some(format!("{} 센서가 연결됐다", info.id))
        }
        ServerEvent::Uplink(_, Uplink::Report(report)) => {
            hub.receive(report);
            None
        }
        ServerEvent::Uplink(_, Uplink::Heartbeat) => None,
        ServerEvent::Uplink(sensor, Uplink::Status(kind, status)) => {
            hub.set_status(&sensor, kind, status);
            None
        }
        ServerEvent::SensorLeft(sensor) => {
            hub.set_connected(&sensor, false);
            Some(format!(
                "{sensor} 센서 연결이 끊겼다 — 그 교실은 탐지 공백이다"
            ))
        }
        ServerEvent::MalformedDropped(sensor) => {
            Some(format!("{sensor} 센서가 보낸 형식이 틀린 메시지를 버렸다"))
        }
        ServerEvent::ClockAhead(sensor, ahead_ms) => Some(format!(
            "{sensor} 센서 시각이 서버보다 {}초 앞서 보고서를 버렸다 — 센서의 시간 동기(NTP)를 확인한다",
            ahead_ms / 1_000
        )),
        ServerEvent::Rejected(reason) => {
            Some(format!("연결을 거부했다: {}", rejection_text(reason)))
        }
    }
}

fn rejection_text(reason: Rejection) -> &'static str {
    match reason {
        Rejection::BadVersion => "프로토콜 버전이 다르다 — 같은 버전의 앱을 쓴다",
        Rejection::BadPairing => "페어링 코드가 맞지 않는다",
        Rejection::LockedOut => "실패가 많아 그 출처를 잠시 잠갔다",
    }
}

fn spawn_collectors(
    rt: &tokio::runtime::Runtime,
    config: &AppConfig,
    clock: &ClockFn,
    tx: &mpsc::Sender<CollectorEvent>,
    which: Collectors,
) {
    match which {
        Collectors::Real => {}
        Collectors::None => return,
        Collectors::Replay(events) => {
            let tx = tx.clone();
            // 실제 수집기처럼 같은 사건 열을 1초마다 되풀이한다. 센서 모드는 서버에 붙기 전 관측을 버리므로
            // 한 번만 흘리면 연결 뒤에 아무것도 남지 않는다.
            rt.spawn(async move {
                loop {
                    for event in &events {
                        if tx.send(event.clone()).await.is_err() {
                            return;
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            });
            return;
        }
    }
    rt.spawn(tacet_collector_ble::run(clock.clone(), tx.clone()));
    rt.spawn(tacet_collector_wifi::run(clock.clone(), tx.clone()));
    if let Some(args) = config.sdr.device_args.clone() {
        let settings = tacet_collector_sdr::SdrSettings {
            device_args: args,
            ..Default::default()
        };
        rt.spawn(tacet_collector_sdr::run(
            settings,
            clock.clone(),
            tx.clone(),
        ));
    }
}

/// 모드에 맞게 백그라운드 작업을 띄운다.
pub fn start(config: AppConfig) -> anyhow::Result<Handles> {
    start_with(config, Collectors::Real)
}

/// [`start`]와 같되 수집기를 띄울지 고른다.
pub fn start_with(config: AppConfig, collectors: Collectors) -> anyhow::Result<Handles> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("비동기 런타임을 만들지 못했다")?;
    let model = Arc::new(Mutex::new(UiModel::new(config.mode)));
    let hub = match config.mode {
        Mode::Standalone => Some(start_standalone(&runtime, &config, &model, collectors)?),
        Mode::Server => Some(start_server(&runtime, &config, &model)?),
        Mode::Sensor => {
            start_sensor(&runtime, &config, &model, collectors)?;
            None
        }
        Mode::Viewer => {
            start_viewer(&runtime, &config, &model);
            None
        }
    };
    Ok(Handles {
        model,
        hub,
        runtime: Some(runtime),
    })
}

fn start_standalone(
    rt: &tokio::runtime::Runtime,
    config: &AppConfig,
    model: &Shared,
    collectors: Collectors,
) -> anyhow::Result<Arc<Mutex<Hub>>> {
    let info = config
        .sensor_info(this_platform())
        .context("[sensor] 절이 필요하다")?;
    let hub = Arc::new(Mutex::new(Hub::new(
        config.detector.clone(),
        config.layout.clone(),
        config.whitelist.clone(),
        new_salt(),
    )));
    let started = Instant::now();
    let clock: ClockFn = Arc::new(move || {
        Timestamp(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX))
    });
    if let Ok(mut h) = hub.lock() {
        // 단독 모드의 센서는 이 컴퓨터 자신이라 처음부터 연결돼 있다.
        h.set_connected(&info.id, true);
    }
    let (tx, mut rx) = mpsc::channel(4096);
    spawn_collectors(rt, config, &clock, &tx, collectors);
    let (hub_task, model_task) = (hub.clone(), model.clone());
    rt.spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        // 수집기가 모두 끝나도(모두 "지원 안 함" 등) 시계와 화면은 계속 돈다.
        let mut collectors_open = true;
        loop {
            tokio::select! {
                event = rx.recv(), if collectors_open => {
                    let Some(event) = event else {
                        collectors_open = false;
                        continue;
                    };
                    if let Ok(mut h) = hub_task.lock() {
                        apply_collector_event(&mut h, &info.id, event);
                    }
                }
                _ = tick.tick() => {
                    let snapshot = hub_task.lock().ok().map(|mut h| {
                        h.tick(clock());
                        (h.view(), h.log().to_vec())
                    });
                    if let Some((view, log)) = snapshot {
                        update(&model_task, |m| {
                            m.view = view;
                            m.log = log;
                        });
                    }
                }
            }
        }
    });
    Ok(hub)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StoredIdentity {
    cert: String,
    key: String,
}

/// 서버 인증서를 저장해 두고 다시 쓴다 — 센서가 고정한 지문이 재시작해도 바뀌지 않게. 저장소 밖(앱 상태 폴더)이다.
pub fn load_or_create_identity(path: &Path) -> anyhow::Result<ServerIdentity> {
    if let Ok(text) = std::fs::read_to_string(path) {
        let stored: StoredIdentity =
            serde_json::from_str(&text).context("서버 인증서 파일이 깨졌다")?;
        let cert = hex::decode(&stored.cert).context("서버 인증서가 16진수가 아니다")?;
        let key = hex::decode(&stored.key).context("서버 키가 16진수가 아니다")?;
        return Ok(ServerIdentity::from_der(cert, key));
    }
    let identity = ServerIdentity::generate()?;
    let stored = StoredIdentity {
        cert: hex::encode(identity.cert_der()),
        key: hex::encode(identity.key_der()),
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // TODO(decision): Windows에서는 파일 권한 대신 DPAPI 등으로 키를 보호할지 정한다(docs/STATUS.md 열린 결정).
    write_private(path, &serde_json::to_vec(&stored)?)?;
    Ok(identity)
}

fn start_server(
    rt: &tokio::runtime::Runtime,
    config: &AppConfig,
    model: &Shared,
) -> anyhow::Result<Arc<Mutex<Hub>>> {
    let section = config.server.clone().context("[server] 절이 필요하다")?;
    let bind_warnings = config.bind_warnings();
    let identity = load_or_create_identity(&config.state_dir().join("server-identity.json"))?;
    let sensor_code = tacet_net::pairing::new_code();
    let viewer_code = tacet_net::pairing::new_code();
    let dashboard_code = tacet_dashboard::new_access_code();
    let start_epoch = now_epoch_ms();
    let hub = Arc::new(Mutex::new(Hub::new(
        config.detector.clone(),
        config.layout.clone(),
        config.whitelist.clone(),
        new_salt(),
    )));
    let settings = ServerSettings {
        bind: section.bind,
        sensor_code: sensor_code.clone(),
        viewer_code: viewer_code.clone(),
        session_start_epoch_ms: start_epoch,
        max_failures: 10,
        lockout: tacet_net::server::DEFAULT_LOCKOUT,
    };
    let (server, mut events) = rt
        .block_on(tacet_net::serve(settings, &identity))
        .context("서버를 띄우지 못했다")?;
    let advertiser = tacet_net::discovery::advertise(
        &section.building,
        server.local_addr().port(),
        &server.fingerprint(),
    )
    .ok();
    let (view_tx, view_rx) = watch::channel(BuildingView::default());
    let dashboard = DashboardState::new(view_rx, dashboard_code.clone());
    let (dash_addr, _) = rt
        .block_on(tacet_dashboard::serve(section.dashboard_bind, dashboard))
        .context("대시보드를 띄우지 못했다")?;
    update(model, |m| {
        m.header = vec![
            format!("센서 페어링 코드: {sensor_code}"),
            format!("감독관 뷰 페어링 코드: {viewer_code}"),
            format!("서버 주소: {}", server.local_addr()),
            format!(
                "서버 지문: {} (센서 화면과 대조하거나 센서 설정 [remote] fingerprint에 적는다)",
                server.fingerprint()
            ),
            format!("웹 대시보드: http://{dash_addr}/ · 접속 코드 {dashboard_code}"),
            "대시보드는 시험장 내부망에서만 연다. 인터넷에 노출하지 않는다.".to_owned(),
        ];
        m.header.extend(bind_warnings.iter().cloned());
        m.connection = Some(if advertiser.is_some() {
            StatusLine::new(
                Severity::Ok,
                "서버 작동 중 — 시험장 네트워크에 서버를 알리는 중",
            )
        } else {
            StatusLine::new(
                Severity::Warning,
                "서버 작동 중 — 네트워크 자동 찾기(mDNS)를 못 켰습니다. 교실 노트북에 서버 주소를 직접 넣으세요.",
            )
        });
    });
    let (hub_task, model_task) = (hub.clone(), model.clone());
    rt.spawn(async move {
        let _keep_advertising = advertiser;
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                event = events.recv() => {
                    let Some(event) = event else { return };
                    let line = hub_task.lock().ok().and_then(|mut h| apply_server_event(&mut h, event));
                    if let Some(line) = line {
                        update(&model_task, |m| m.notice = Some(line));
                    }
                }
                _ = tick.tick() => {
                    let now = Timestamp(now_epoch_ms().saturating_sub(start_epoch));
                    let snapshot = hub_task.lock().ok().map(|mut h| {
                        let _ = h.tick(now);
                        (h.view(), h.log().to_vec())
                    });
                    if let Some((view, log)) = snapshot {
                        server.publish(view.clone());
                        view_tx.send_replace(view.clone());
                        update(&model_task, |m| {
                            m.view = view;
                            m.log = log;
                        });
                    }
                }
            }
        }
    });
    Ok(hub)
}

fn pin_path(state_dir: &Path) -> PathBuf {
    state_dir.join("server-pin.txt")
}

async fn resolve_server(address: Option<std::net::SocketAddr>) -> Option<std::net::SocketAddr> {
    if address.is_some() {
        return address;
    }
    tokio::task::spawn_blocking(|| tacet_net::discovery::discover(Duration::from_secs(3)))
        .await
        .ok()
        .and_then(Result::ok)
        .and_then(|found| found.first().map(|f| f.addr))
}

fn start_sensor(
    rt: &tokio::runtime::Runtime,
    config: &AppConfig,
    model: &Shared,
    collectors: Collectors,
) -> anyhow::Result<()> {
    let info = config
        .sensor_info(this_platform())
        .context("[sensor] 절이 필요하다")?;
    let remote = config.remote.clone().context("[remote] 절이 필요하다")?;
    let session_start = Arc::new(AtomicU64::new(now_epoch_ms()));
    let start_for_clock = session_start.clone();
    let clock: ClockFn = Arc::new(move || {
        Timestamp(now_epoch_ms().saturating_sub(start_for_clock.load(Ordering::SeqCst)))
    });
    let (tx, mut rx) = mpsc::channel(4096);
    spawn_collectors(rt, config, &clock, &tx, collectors);
    let model_task = model.clone();
    let state_dir = config.state_dir();
    let expected = config.expected_fingerprint().flatten();
    rt.spawn(async move {
        let mut statuses = std::collections::BTreeMap::new();
        let mut seq = 0u64;
        let mut dropped = 0u64;
        loop {
            // 연결 전에 쌓인 관측은 세션 시각이 맞지 않으니 버리고 그 수를 알린다. 상태는 남긴다.
            dropped += drain_offline(&mut rx, &mut statuses);
            update(&model_task, |m| {
                m.connection = Some(StatusLine::new(
                    Severity::Info,
                    offline_text("시험본부 서버를 찾는 중", dropped),
                ))
            });
            let Some(addr) = resolve_server(remote.address).await else {
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            };
            // 설정에 적은 지문이 저장된 지문보다 앞선다.
            let pinned = expected.or_else(|| {
                std::fs::read_to_string(pin_path(&state_dir))
                    .ok()
                    .and_then(|t| Fingerprint::parse(&t))
            });
            let (mut link, session, fingerprint) = match tacet_net::connect_sensor(
                addr,
                &remote.code,
                pinned,
                info.clone(),
            )
            .await
            {
                Ok(connected) => connected,
                Err(e) => {
                    update(&model_task, |m| {
                        m.connection = Some(StatusLine::new(
                            Severity::Problem,
                            offline_text(
                                &format!(
                                    "시험본부 서버에 연결하지 못했습니다 — 다시 시도하는 중 ({e})"
                                ),
                                dropped,
                            ),
                        ))
                    });
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
            };
            let pin_note = save_pin(&state_dir, &fingerprint);
            session_start.store(session.start_epoch_ms, Ordering::SeqCst);
            // 세션 시작 시각을 받기 전에 들어온 관측은 다른 기준의 시각이다.
            dropped += drain_offline(&mut rx, &mut statuses);
            update(&model_task, |m| {
                m.connection = Some(StatusLine::new(
                    if pin_note.is_empty() {
                        Severity::Ok
                    } else {
                        Severity::Warning
                    },
                    format!(
                        "시험본부 서버에 연결됨 ({addr} · 지문 {})",
                        fingerprint.short()
                    ) + &pin_note,
                ))
            });
            for (kind, status) in &statuses {
                let _ = link
                    .send(&Uplink::Status(*kind, Clone::clone(status)))
                    .await;
            }
            let mut heartbeat = tokio::time::interval(HEARTBEAT);
            heartbeat.tick().await;
            let mut collectors_open = true;
            loop {
                let event = tokio::select! {
                    event = rx.recv(), if collectors_open => event,
                    _ = heartbeat.tick() => {
                        if link.send(&Uplink::Heartbeat).await.is_err() {
                            break;
                        }
                        continue;
                    }
                };
                let Some(event) = event else {
                    // 수집기가 모두 끝났다. 하트비트로 연결은 유지해 서버가 이 센서를 "연결 끊김"이 아니라 마지막
                    // 수집기 상태(꺼짐)로 보게 한다.
                    collectors_open = false;
                    update(&model_task, |m| {
                        m.connection = Some(StatusLine::new(
                            Severity::Problem,
                            "이 노트북의 신호 감지가 모두 멈췄습니다 — 서버로 보낼 것이 없습니다",
                        ))
                    });
                    continue;
                };
                let uplink = match event {
                    CollectorEvent::Obs(obs) => {
                        seq += 1;
                        Uplink::Report(SensorReport {
                            sensor: info.id.clone(),
                            seq,
                            obs,
                        })
                    }
                    CollectorEvent::Status(kind, status) => {
                        statuses.insert(kind, status.clone());
                        let view = sensor_only_view(&info, true, &statuses);
                        update(&model_task, |m| m.view = view);
                        Uplink::Status(kind, status)
                    }
                };
                if link.send(&uplink).await.is_err() {
                    update(&model_task, |m| {
                        m.connection = Some(StatusLine::new(
                            Severity::Problem,
                            "시험본부 서버 연결이 끊겼습니다 — 다시 연결하는 중",
                        ))
                    });
                    break;
                }
            }
        }
    });
    Ok(())
}

/// 센서 하트비트 간격. 서버의 유휴 시간 제한보다 충분히 짧다.
const HEARTBEAT: Duration = Duration::from_secs(30);

/// 연결되지 않은 동안 쌓인 수집기 사건을 비운다. 상태는 기억하고, 버린 관측 수를 돌려준다.
fn drain_offline(
    rx: &mut mpsc::Receiver<CollectorEvent>,
    statuses: &mut std::collections::BTreeMap<
        tacet_core::obs::CollectorKind,
        tacet_core::obs::CollectorStatus,
    >,
) -> u64 {
    let mut dropped = 0;
    while let Ok(event) = rx.try_recv() {
        match event {
            CollectorEvent::Obs(_) => dropped += 1,
            CollectorEvent::Status(kind, status) => {
                statuses.insert(kind, status);
            }
        }
    }
    dropped
}

fn offline_text(what: &str, dropped: u64) -> String {
    if dropped == 0 {
        what.to_owned()
    } else {
        format!("{what} — 연결되지 않은 동안 관측 {dropped}건을 버렸다(그동안은 탐지 공백)")
    }
}

/// 고정할 서버 지문을 저장한다. 실패하면 화면 연결 줄에 붙일 경고를 돌려준다 — 저장되지 않으면 다음 연결도 처음 본
/// 지문을 믿게 되므로 감독관이 알아야 한다.
fn save_pin(state_dir: &Path, fingerprint: &Fingerprint) -> String {
    let saved = std::fs::create_dir_all(state_dir)
        .and_then(|()| write_private(&pin_path(state_dir), fingerprint.to_string().as_bytes()));
    match saved {
        Ok(()) => String::new(),
        Err(e) => {
            format!(" · 지문을 저장하지 못했다({e}) — 다음 연결 때도 서버 화면의 지문과 대조한다")
        }
    }
}

/// 이 사용자만 읽고 쓸 수 있는 파일로 쓴다(유닉스 0600). 처음부터 그 권한으로 만들어 잠깐이라도 열리지 않게 한다.
pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    // 이미 있던 파일은 만들 때의 권한이 적용되지 않으니 다시 좁힌다.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(bytes)?;
    file.sync_all()
}

fn start_viewer(rt: &tokio::runtime::Runtime, config: &AppConfig, model: &Shared) {
    let Some(remote) = config.remote.clone() else {
        return;
    };
    let state_dir = config.state_dir();
    let expected = config.expected_fingerprint().flatten();
    let model_task = model.clone();
    rt.spawn(async move {
        loop {
            update(&model_task, |m| {
                m.connection = Some(StatusLine::new(Severity::Info, "시험본부 서버를 찾는 중"))
            });
            let Some(addr) = resolve_server(remote.address).await else {
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            };
            // 설정에 적은 지문이 저장된 지문보다 앞선다.
            let pinned = expected.or_else(|| {
                std::fs::read_to_string(pin_path(&state_dir))
                    .ok()
                    .and_then(|t| Fingerprint::parse(&t))
            });
            match tacet_net::connect_viewer(addr, &remote.code, pinned).await {
                Ok((mut link, fingerprint)) => {
                    let pin_note = save_pin(&state_dir, &fingerprint);
                    update(&model_task, |m| {
                        m.connection = Some(StatusLine::new(
                            if pin_note.is_empty() {
                                Severity::Ok
                            } else {
                                Severity::Warning
                            },
                            format!(
                                "시험본부 서버에 연결됨 ({addr} · 지문 {})",
                                fingerprint.short()
                            ) + &pin_note,
                        ))
                    });
                    while let Ok(Some(view)) = link.next_view().await {
                        update(&model_task, |m| m.view = view);
                    }
                    update(&model_task, |m| {
                        m.connection = Some(StatusLine::new(
                            Severity::Problem,
                            "시험본부 서버 연결이 끊겼습니다 — 다시 연결하는 중",
                        ))
                    });
                }
                Err(e) => update(&model_task, |m| {
                    m.connection = Some(StatusLine::new(
                        Severity::Problem,
                        format!("시험본부 서버에 연결하지 못했습니다 — 다시 시도하는 중 ({e})"),
                    ))
                }),
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });
}

/// 세션 보고서를 앱 상태 폴더에 저장한다. 식별자는 세션 솔트 해시뿐이다.
pub fn save_report(hub: &Hub, dir: &Path) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("session-{}.json", now_epoch_ms()));
    write_private(&path, &serde_json::to_vec_pretty(hub.report())?)?;
    Ok(path)
}

/// 배치가 비었는지(서버 화면 안내용).
pub fn layout_is_empty(layout: &RoomLayout) -> bool {
    layout.rooms.is_empty()
}
