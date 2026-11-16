//! 관측값과 수집기 상태.
//!
//! 수집기는 OS·칩·USB 데이터를 여기 타입으로 바꾸는 데서 멈춘다(AGENTS.md §2 규칙 5). 쓰는 필드는
//! AGENTS.md §3.2의 허용 목록뿐이다: 광고 메타데이터, 비콘 헤더, 연결 존재와 헤더 수준 정보, 대역 에너지.

use serde::{Deserialize, Serialize};

use crate::ids::{BleId, Mac, Platform, RoomId, SensorId, SensorPos, ServiceUuid};
use crate::time::Timestamp;

#[cfg(test)]
mod tests;

/// Wi-Fi 대역.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Band {
    /// 2.4GHz.
    B2_4,
    /// 5GHz.
    B5,
    /// 6GHz(Wi-Fi 6E/7, 설계 §3 T4).
    B6,
}

/// 중심 주파수(MHz)에서 대역과 채널 번호를 구한다(IEEE 802.11 채널 번호 체계). 2.4GHz와 6GHz는 같은
/// 채널 번호를 쓰므로 번호만으로 대역을 알 수 없다 — 그래서 대역을 함께 돌려준다.
pub fn band_channel_from_mhz(freq_mhz: u32) -> Option<(Band, u16)> {
    let channel = |base: u32| u16::try_from((freq_mhz - base) / 5).ok();
    match freq_mhz {
        2484 => Some((Band::B2_4, 14)),
        2412..=2472 => channel(2407).map(|c| (Band::B2_4, c)),
        // 6GHz는 5950MHz 기준이고 채널 2(5935MHz)만 예외다.
        5935 => Some((Band::B6, 2)),
        5955..=7115 => channel(5950).map(|c| (Band::B6, c)),
        5160..=5925 => channel(5000).map(|c| (Band::B5, c)),
        _ => None,
    }
}

/// SSID. 숨긴 네트워크는 길이 0이나 NUL로 채운 SSID 두 가지로 나타난다(설계 §4.2 R5).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Ssid {
    /// 숨긴 네트워크.
    Hidden,
    /// 이름이 있는 네트워크. UTF-8이 아닌 바이트는 대체 문자로 바꾼다.
    Named(String),
}

impl Ssid {
    /// 비콘의 SSID 바이트(최대 32)로 만든다.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        if bytes.iter().all(|&b| b == 0) {
            return Self::Hidden;
        }
        Self::Named(String::from_utf8_lossy(bytes).into_owned())
    }
}

/// BLE 광고 관측.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BleObs {
    /// 광고자 식별자.
    pub id: BleId,
    /// 수신 세기(dBm).
    pub rssi: i16,
    /// 제조사 데이터(회사 ID, 데이터). 회사 ID 순으로 정렬돼 있어야 한다.
    pub manufacturer: Vec<(u16, Vec<u8>)>,
    /// 광고된 서비스 UUID. 정렬돼 있어야 한다.
    pub services: Vec<ServiceUuid>,
    /// 서비스 데이터(UUID, 데이터). UUID 순으로 정렬돼 있어야 한다.
    pub service_data: Vec<(ServiceUuid, Vec<u8>)>,
    /// 광고에 실린 기기 이름(Local Name). 교실 보정 모드에서만 채우고, 감독관이 자기 휴대폰을 고르는 목록에만 쓴다 —
    /// 판정·알림·보고서·로그에는 넣지 않고 저장하지 않는다(AGENTS.md §3.2, 사용자 결정 2026-11-16). 직렬화하지 않으므로
    /// 녹화(`.obslog`)·센서 → 서버 전송에 실리지 않고, `Debug`로도 보이지 않는다([`BleName`]). 없으면 `None`.
    #[serde(skip)]
    pub name: Option<BleName>,
    /// 관측 시각.
    pub at: Timestamp,
}

/// 광고 이름. 남의 기기 이름이라 `Debug` 출력에 내용을 보이지 않는다.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BleName(pub String);

impl std::fmt::Debug for BleName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BleName(<가림>)")
    }
}

/// 광고 이름 최대 글자 수. 남이 보낸 문자열이라 화면을 넘치지 않게 자른다.
pub const BLE_NAME_MAX_CHARS: usize = 40;

/// 광고 이름 바이트를 화면에 보일 수 있는 이름으로 정리한다: UTF-8이 아닌 바이트는 대체 문자로, 제어 문자는 빼고, 앞뒤 공백을
/// 지우고, [`BLE_NAME_MAX_CHARS`]자로 자른다. 남는 글자가 없으면 `None`.
pub fn ble_name(bytes: &[u8]) -> Option<BleName> {
    let text = String::from_utf8_lossy(bytes);
    let cleaned: String = text.chars().filter(|c| !c.is_control()).collect();
    let trimmed: String = cleaned.trim().chars().take(BLE_NAME_MAX_CHARS).collect();
    let trimmed = trimmed.trim_end().to_owned();
    (!trimmed.is_empty()).then_some(BleName(trimmed))
}

/// Wi-Fi 비콘(관리 모드 스캔 결과) 관측.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WifiObs {
    /// BSSID.
    pub bssid: Mac,
    /// SSID.
    pub ssid: Ssid,
    /// 수신 세기(dBm).
    pub rssi: i16,
    /// 채널 번호.
    pub channel: u16,
    /// 대역.
    pub band: Band,
    /// 관측 시각.
    pub at: Timestamp,
}

/// LTE FDD 업링크 대역(설계 §4.5). 국내 5G NSA 단말도 LTE 앵커 업링크를 남긴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CellBand {
    /// B1 업링크 1920–1980MHz.
    B1,
    /// B3 업링크 1710–1785MHz.
    B3,
    /// B5 업링크 824–849MHz.
    B5,
    /// B7 업링크 2500–2570MHz.
    B7,
}

impl CellBand {
    /// 모든 대역.
    pub const ALL: [CellBand; 4] = [CellBand::B1, CellBand::B3, CellBand::B5, CellBand::B7];

    /// 업링크 범위(시작, 끝, kHz). 3GPP TS 36.101 표 5.5-1.
    pub const fn uplink_khz(self) -> (u32, u32) {
        match self {
            CellBand::B1 => (1_920_000, 1_980_000),
            CellBand::B3 => (1_710_000, 1_785_000),
            CellBand::B5 => (824_000, 849_000),
            CellBand::B7 => (2_500_000, 2_570_000),
        }
    }
}

/// SDR 대역 에너지 관측. 복조하지 않는다(AGENTS.md §3.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RfObs {
    /// 대역.
    pub band: CellBand,
    /// 대역 평균 전력(0.01dBm 단위 정수). core의 비교를 정수로 두려고 수집기가 바꿔 넣는다.
    pub power_cdbm: i32,
    /// 관측 시각.
    pub at: Timestamp,
}

/// 수집기가 내는 관측값.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Observation {
    /// BLE 광고.
    Ble(BleObs),
    /// Wi-Fi 비콘.
    Wifi(WifiObs),
    /// 셀룰러 업링크 대역 에너지.
    Rf(RfObs),
}

impl Observation {
    /// 관측 시각.
    pub fn at(&self) -> Timestamp {
        match self {
            Observation::Ble(o) => o.at,
            Observation::Wifi(o) => o.at,
            Observation::Rf(o) => o.at,
        }
    }

    /// 위치 판정에 쓰는 세기(dBm). SDR 대역 전력은 dBFS 기준 상대값이라 교실 경계(dBm)와 같은 눈금이 아니므로
    /// 위치·감점 계산에 쓰지 않는다(`None`). R7은 센서별 기준선 대비로만 판정한다.
    pub fn strength_dbm(&self) -> Option<i16> {
        match self {
            Observation::Ble(o) => Some(o.rssi),
            Observation::Wifi(o) => Some(o.rssi),
            Observation::Rf(_) => None,
        }
    }
}

/// 센서 정보. 센서 모드에서는 서버로 보내는 보고서마다 붙는다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensorInfo {
    /// 센서 식별자.
    pub id: SensorId,
    /// 센서가 놓인 교실.
    pub room: RoomId,
    /// 교실 안 위치.
    pub position: SensorPos,
    /// 운영체제.
    pub platform: Platform,
}

/// 센서가 서버(또는 단독 모드의 core)로 넘기는 단위(설계 §5.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensorReport {
    /// 보낸 센서.
    pub sensor: SensorId,
    /// 센서별 일련번호. 같은 시각의 관측 순서를 정하는 데 쓴다.
    pub seq: u64,
    /// 관측값.
    pub obs: Observation,
}

/// 수집기 종류.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CollectorKind {
    /// BLE 광고 스캔.
    Ble,
    /// Wi-Fi 스캔.
    Wifi,
    /// SDR 업링크 에너지(R7).
    Sdr,
}

/// 수집에 필요한 권한.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Permission {
    /// Windows 정밀 위치(24H2부터 BSSID에 필요, 설계 §7.1).
    PreciseLocation,
    /// macOS 위치 서비스(CoreWLAN BSSID에 필요, 설계 §7.2).
    Location,
    /// 블루투스 사용 권한.
    Bluetooth,
}

/// 수집기가 꺼진 이유. 조용히 꺼진 수집기는 탐지 공백이므로 반드시 보고한다(AGENTS.md §2 규칙 5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OffReason {
    /// 권한이 없다.
    PermissionDenied(Permission),
    /// 어댑터·장비가 없다.
    DeviceMissing,
    /// 장비 드라이버가 바인딩되지 않았다(Windows WinUSB).
    DriverMissing,
    /// 이 OS나 빌드에서 지원하지 않는다.
    Unsupported(String),
    /// 그 밖의 오류.
    Error(String),
}

/// 수집기 상태.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollectorStatus {
    /// 시작하는 중.
    Starting,
    /// 수집 중.
    Running,
    /// 꺼짐.
    Off(OffReason),
}

/// 수집기가 앱으로 보내는 사건.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollectorEvent {
    /// 관측값.
    Obs(Observation),
    /// 상태 변화.
    Status(CollectorKind, CollectorStatus),
}
