//! 합성 관측·보고서 빌더(AGENTS.md §3.3, §5.7). 실제 식별자를 쓰지 않는다.
//!
//! - MAC: `00:00:5E:00:53:xx`(RFC 7042 문서용), 랜덤 MAC 예시는 `02:00:5E:00:53:xx`
//! - macOS UUID: 작은 정수
//!
//! 하네스 함수는 반복과 분기를 써도 되고, 테스트 본문은 안 된다(AGENTS.md §5.3).

use crate::config::{DetectorConfig, Room, RoomLayout, SensorSpec, StorageZone, Whitelist};
use crate::engine::{AlertCandidate, Engine};
use crate::ids::{BleId, Mac, Platform, RoomId, SensorId, SensorPos, ServiceUuid};
use crate::obs::{
    Band, BleObs, CellBand, Observation, RfObs, SensorInfo, SensorReport, Ssid, WifiObs,
};
use crate::rules::{RuleContext, RuleId, evaluate};
use crate::time::Timestamp;

/// 문서용 MAC `00:00:5E:00:53:xx`.
pub const fn doc_mac(last: u8) -> Mac {
    Mac([0x00, 0x00, 0x5E, 0x00, 0x53, last])
}

/// 랜덤 MAC 예시 `02:00:5E:00:53:xx`(locally administered 비트 켬).
pub const fn doc_random_mac(last: u8) -> Mac {
    Mac([0x02, 0x00, 0x5E, 0x00, 0x53, last])
}

/// 초 단위 시각.
pub const fn secs(s: u64) -> Timestamp {
    Timestamp::from_secs(s)
}

/// BLE 관측 빌더.
#[derive(Debug, Clone)]
pub struct BleBuilder(BleObs);

/// BLE 관측을 문서용 주소로 시작한다.
pub fn ble(addr_last: u8) -> BleBuilder {
    BleBuilder(BleObs {
        id: BleId::Addr(doc_random_mac(addr_last)),
        rssi: -60,
        manufacturer: Vec::new(),
        services: Vec::new(),
        service_data: Vec::new(),
        name: None,
        at: Timestamp(0),
    })
}

impl BleBuilder {
    /// macOS 호스트별 UUID로 바꾼다.
    pub fn host_local(mut self, uuid: u128) -> Self {
        self.0.id = BleId::HostLocal(uuid);
        self
    }

    /// 광고 이름을 붙인다.
    pub fn name(mut self, name: &str) -> Self {
        self.0.name = Some(crate::obs::BleName(name.to_owned()));
        self
    }

    /// RSSI.
    pub fn rssi(mut self, rssi: i16) -> Self {
        self.0.rssi = rssi;
        self
    }
    /// 제조사 데이터를 더한다(회사 ID 순 정렬 유지).
    pub fn vendor(mut self, company: u16, data: &[u8]) -> Self {
        self.0.manufacturer.push((company, data.to_vec()));
        self.0.manufacturer.sort();
        self
    }
    /// 16비트 서비스 UUID를 더한다.
    pub fn service(mut self, short: u16) -> Self {
        self.0.services.push(ServiceUuid::from_u16(short));
        self.0.services.sort();
        self
    }
    /// 16비트 서비스 데이터를 더한다.
    pub fn service_data(mut self, short: u16, data: &[u8]) -> Self {
        self.0
            .service_data
            .push((ServiceUuid::from_u16(short), data.to_vec()));
        self.0.service_data.sort();
        self
    }
    /// 시각(초).
    pub fn at_secs(mut self, s: u64) -> Self {
        self.0.at = secs(s);
        self
    }
    /// 관측값.
    pub fn build(self) -> Observation {
        Observation::Ble(self.0)
    }

    /// 관측을 감싸지 않은 BLE 관측 그대로.
    pub fn build_ble(self) -> BleObs {
        self.0
    }
}

/// Wi-Fi 관측 빌더.
#[derive(Debug, Clone)]
pub struct WifiBuilder(WifiObs);

/// Wi-Fi 관측을 문서용 BSSID와 일반 SSID로 시작한다.
pub fn wifi(bssid: Mac) -> WifiBuilder {
    WifiBuilder(WifiObs {
        bssid,
        ssid: Ssid::Named("학교-무선".to_owned()),
        rssi: -60,
        channel: 6,
        band: Band::B2_4,
        at: Timestamp(0),
    })
}

impl WifiBuilder {
    /// SSID 바이트(숨김 표현 시험용).
    pub fn ssid_bytes(mut self, bytes: &[u8]) -> Self {
        self.0.ssid = Ssid::from_bytes(bytes);
        self
    }
    /// SSID 이름.
    pub fn ssid(mut self, name: &str) -> Self {
        self.0.ssid = Ssid::Named(name.to_owned());
        self
    }
    /// RSSI.
    pub fn rssi(mut self, rssi: i16) -> Self {
        self.0.rssi = rssi;
        self
    }
    /// 대역과 채널.
    pub fn channel(mut self, band: Band, channel: u16) -> Self {
        self.0.band = band;
        self.0.channel = channel;
        self
    }
    /// 시각(초).
    pub fn at_secs(mut self, s: u64) -> Self {
        self.0.at = secs(s);
        self
    }
    /// 관측값.
    pub fn build(self) -> Observation {
        Observation::Wifi(self.0)
    }
}

/// 이름을 숨긴 와이파이(문서용 주소 `00:00:5E:00:53:xx`, 랜덤 비트 없음). 교실 경계 안이면 R3 + R5 = 6점(보통)이다.
/// 여러 센서가 같은 BSSID를 보므로 위치 판정·전송 시험의 "새 기기"로 쓴다.
pub fn hidden_wifi(last: u8, rssi: i16, at_secs: u64) -> Observation {
    wifi(doc_mac(last))
        .ssid_bytes(&[])
        .rssi(rssi)
        .at_secs(at_secs)
        .build()
}

/// SDR 대역 전력 관측(dBm 정수).
pub fn rf(band: CellBand, power_dbm: i32, at_secs: u64) -> Observation {
    Observation::Rf(RfObs {
        band,
        power_cdbm: power_dbm * 100,
        at: secs(at_secs),
    })
}

/// 센서 보고서.
pub fn report(sensor: &str, seq: u64, obs: Observation) -> SensorReport {
    SensorReport {
        sensor: SensorId::new(sensor),
        seq,
        obs,
    }
}

/// 센서 정보.
pub fn sensor_info(id: &str, room: &str, position: SensorPos) -> SensorInfo {
    SensorInfo {
        id: SensorId::new(id),
        room: RoomId::new(room),
        position,
        platform: Platform::Windows,
    }
}

fn spec(id: &str, position: SensorPos, boundary: Option<i16>) -> SensorSpec {
    SensorSpec {
        id: SensorId::new(id),
        position,
        inside_boundary_dbm: boundary,
        // 시험 교실은 와이파이 경계도 같은 합성값으로 둔다(무선별 시험은 따로 바꾼다).
        wifi_inside_boundary_dbm: boundary,
        place: None,
    }
}

/// 교실 하나에 센서 하나(`solo`), 보정 경계는 `boundary`.
pub fn single_room(boundary: Option<i16>) -> RoomLayout {
    RoomLayout {
        rooms: vec![Room {
            id: RoomId::new("3-2"),
            sensors: vec![spec("solo", SensorPos::Front, boundary)],
            adjacent: vec![],
        }],
        storage: vec![],
    }
}

/// 나란한 세 교실 3-1, 3-2, 3-3. 교실마다 앞·뒤 센서(`3-1-앞` 등), 경계는 모두 -75dBm(합성값).
pub fn three_rooms() -> RoomLayout {
    let room = |id: &str, adjacent: &[&str]| Room {
        id: RoomId::new(id),
        sensors: vec![
            spec(&format!("{id}-앞"), SensorPos::Front, Some(-75)),
            spec(&format!("{id}-뒤"), SensorPos::Back, Some(-75)),
        ],
        adjacent: adjacent.iter().map(|a| RoomId::new(*a)).collect(),
    };
    RoomLayout {
        rooms: vec![
            room("3-1", &["3-2"]),
            room("3-2", &["3-1", "3-3"]),
            room("3-3", &["3-2"]),
        ],
        storage: vec![],
    }
}

/// 수거폰 보관 위치를 더한 배치.
pub fn with_storage(
    mut layout: RoomLayout,
    sensor: &str,
    min_dbm: i16,
    max_dbm: i16,
) -> RoomLayout {
    layout.storage.push(StorageZone {
        sensor: SensorId::new(sensor),
        min_dbm,
        max_dbm,
    });
    layout
}

/// 기준선 10초짜리 설정(시험 빠르게).
pub fn quick_config() -> DetectorConfig {
    DetectorConfig {
        baseline_ms: 10_000,
        ..DetectorConfig::default()
    }
}

/// 기준선 이후 빈 기준선으로 규칙만 평가한다. 보정 경계는 `boundary`.
pub fn rules_for(obs: &Observation, boundary: Option<i16>) -> Vec<RuleId> {
    let ctx = RuleContext {
        in_baseline: false,
        inside_boundary_dbm: boundary,
        rf_reference_cdbm: None,
        rf_burst_cdb: 1_000,
    };
    evaluate(obs, &ctx)
}

/// 회사 ID마다 BLE 광고를 만들어 `rule`을 맞히지 못한 ID 목록을 돌려준다.
pub fn vendors_missing_rule(ids: &[u16], rule: RuleId) -> Vec<u16> {
    ids.iter()
        .copied()
        .filter(|id| !rules_for(&ble(1).vendor(*id, &[1, 2]).build(), None).contains(&rule))
        .collect()
}

/// 보고서 열을 엔진에 넣고 나온 알림 후보를 모두 돌려준다.
pub fn run(engine: &mut Engine, reports: &[SensorReport]) -> Vec<AlertCandidate> {
    reports.iter().flat_map(|r| engine.ingest(r)).collect()
}

/// 설정·배치로 엔진을 만들어 보고서 열을 돌린다.
pub fn run_scenario(
    cfg: DetectorConfig,
    layout: RoomLayout,
    whitelist: Whitelist,
    reports: &[SensorReport],
) -> Vec<AlertCandidate> {
    let mut engine = Engine::new(cfg, layout, whitelist);
    run(&mut engine, reports)
}

/// 같은 관측을 `from_secs`부터 `count`초 동안 1초마다 반복한 보고서 열.
pub fn repeat(
    sensor: &str,
    count: u64,
    from_secs: u64,
    make: impl Fn(u64) -> Observation,
) -> Vec<SensorReport> {
    (0..count)
        .map(|i| report(sensor, from_secs + i, make(from_secs + i)))
        .collect()
}

/// 기기 하나가 여러 센서에 동시에 보이는 다교실 보고서 열(기준선 이후 `from_secs`부터 `count`초).
pub fn multi_sensor(
    readings: &[(&str, i16)],
    from_secs: u64,
    count: u64,
    make: impl Fn(i16, u64) -> Observation,
) -> Vec<SensorReport> {
    let mut out = Vec::new();
    for i in 0..count {
        for (sensor, rssi) in readings {
            out.push(report(sensor, from_secs + i, make(*rssi, from_secs + i)));
        }
    }
    out
}

/// 보고서 열을 돌린 뒤 엔진의 현재 판정(스냅숏)을 돌려준다.
pub fn snapshot_after(
    cfg: DetectorConfig,
    layout: RoomLayout,
    whitelist: Whitelist,
    reports: &[SensorReport],
) -> Vec<AlertCandidate> {
    let mut engine = Engine::new(cfg, layout, whitelist);
    run(&mut engine, reports);
    engine.snapshot()
}
