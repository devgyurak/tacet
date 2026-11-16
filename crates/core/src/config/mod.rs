//! 판정 설정: 가중치, 시간 창, 교실 배치와 보정값, 화이트리스트.
//!
//! 설정 파일(TOML)은 앱이 읽어 이 타입으로 넘긴다. 기본값 가운데 실측으로 확인하지 않은 것은
//! 모두 미검증이다(AGENTS.md §3.6, `docs/rules/`).

use serde::{Deserialize, Serialize};

use crate::ids::{Mac, RoomId, SensorId, SensorPos};
use crate::rules::RuleId;

#[cfg(test)]
mod tests;

/// 규칙별 정수 가중치(설계 §4.2의 낮음~매우 높음을 정수로 매긴 것, AGENTS.md §2 규칙 3).
// TODO(field-unverified): 기본 가중치는 실측 전 값이다. 정하는 것은 decision 이슈다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Weights {
    /// R1 폰 제조사 ID(높음).
    pub r1: u32,
    /// R1a 스마트 안경 제조사 ID·서비스(매우 높음).
    pub r1a: u32,
    /// R1b Fast Pair 액세서리(낮음~중간).
    pub r1b: u32,
    /// R2 새 BLE 광고자가 교실 경계 안(중간).
    pub r2: u32,
    /// R3 새 BSSID가 교실 경계 안(중간).
    pub r3: u32,
    /// R4 locally administered BSSID(높음).
    pub r4: u32,
    /// R5 숨긴 SSID(높음).
    pub r5: u32,
    /// R6 `DIRECT-` SSID(높음).
    pub r6: u32,
    /// R7 LTE 업링크 대역 버스트(높음).
    pub r7: u32,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            r1: 4,
            r1a: 6,
            r1b: 1,
            r2: 2,
            r3: 2,
            r4: 4,
            r5: 4,
            r6: 4,
            r7: 4,
        }
    }
}

impl Weights {
    /// 규칙의 가중치.
    pub fn of(&self, rule: RuleId) -> u32 {
        match rule {
            RuleId::R1 => self.r1,
            RuleId::R1a => self.r1a,
            RuleId::R1b => self.r1b,
            RuleId::R2 => self.r2,
            RuleId::R3 => self.r3,
            RuleId::R4 => self.r4,
            RuleId::R5 => self.r5,
            RuleId::R6 => self.r6,
            RuleId::R7 => self.r7,
        }
    }
}

/// 판정 설정.
// TODO(field-unverified): 시간 창·문턱·감점의 기본값은 실측 전 값이다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DetectorConfig {
    /// 규칙 가중치.
    pub weights: Weights,
    /// 기준선 수집 시간(ms). 설계 §4.1 기본 3분.
    pub baseline_ms: u64,
    /// 규칙 적중을 점수에 더하는 시간 창(ms).
    pub score_window_ms: u64,
    /// 위치 판정 중앙값 창(ms). 설계 §4.7 "수십 초".
    pub locate_window_ms: u64,
    /// 알림을 내는 최소 점수.
    pub alert_score: u32,
    /// 의심 강도 "중간"이 되는 점수.
    pub medium_score: u32,
    /// 의심 강도 "높음"이 되는 점수.
    pub high_score: u32,
    /// 1·2위 교실 RSSI 중앙값 차이가 이 값(dB) 이상이면 신뢰도 높음(설계 §4.7 예시 10dB).
    pub high_confidence_db: i16,
    /// 이 값(dB) 이상이면 신뢰도 중간(설계 §4.7 예시 5dB).
    pub medium_confidence_db: i16,
    /// R7: 기준선 대비 이만큼(0.01dB 단위) 오르면 버스트로 본다.
    pub rf_burst_cdb: i32,
    /// BLE 주소 교체를 같은 기기로 묶을 때 허용하는 같은 센서 RSSI 차이(dB).
    pub ble_rotation_rssi_tolerance_db: i16,
    /// BLE 상관 시간 창(ms).
    pub ble_correlation_window_ms: u64,
    /// 수거폰 보관 위치에서 나온 신호의 감점(설계 §4.3).
    pub storage_penalty: u32,
    /// 건물 밖으로 판정된 신호의 감점(설계 §4.3).
    pub outside_penalty: u32,
    /// 인체 차폐 여유(dB). 모든 센서에서 경계보다 이만큼 이상 약해야 건물 밖 가능성으로 본다. 뒷주머니 차폐가
    /// 앞주머니보다 약 15dB 낮았다는 측정(설계 §4.4, [S6])에서 왔다.
    pub shield_margin_db: i16,
}

/// 판정 설정 값 오류.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SettingError {
    /// 알림 문턱이 0이거나 낮음 ≤ 중간 ≤ 높음 순서가 아니다.
    #[error("알림 문턱은 0보다 크고 alert_score ≤ medium_score ≤ high_score여야 한다")]
    ScoreOrder,
    /// 신뢰도 기준이 중간 ≤ 높음 순서가 아니다.
    #[error("신뢰도 기준은 medium_confidence_db ≤ high_confidence_db여야 한다")]
    ConfidenceOrder,
    /// 제조사 화이트리스트의 데이터 앞부분이 비어 그 회사 기기 전부를 빼게 된다.
    #[error("제조사 화이트리스트(회사 {0:#06X})의 prefix가 비었다 — 그 회사 기기 전부가 빠진다")]
    EmptyManufacturerPrefix(u16),
}

impl DetectorConfig {
    /// 값의 순서와 범위를 확인한다.
    pub fn validate(&self) -> Result<(), SettingError> {
        if self.alert_score == 0
            || self.alert_score > self.medium_score
            || self.medium_score > self.high_score
        {
            return Err(SettingError::ScoreOrder);
        }
        if self.medium_confidence_db > self.high_confidence_db {
            return Err(SettingError::ConfidenceOrder);
        }
        Ok(())
    }
}

impl Whitelist {
    /// 항목을 확인한다.
    pub fn validate(&self) -> Result<(), SettingError> {
        match self.entries.iter().find_map(|e| match e {
            WhitelistEntry::Manufacturer { company, prefix } if prefix.is_empty() => Some(*company),
            _ => None,
        }) {
            Some(company) => Err(SettingError::EmptyManufacturerPrefix(company)),
            None => Ok(()),
        }
    }
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            weights: Weights::default(),
            baseline_ms: 180_000,
            score_window_ms: 120_000,
            locate_window_ms: 30_000,
            alert_score: 4,
            medium_score: 6,
            high_score: 10,
            high_confidence_db: 10,
            medium_confidence_db: 5,
            rf_burst_cdb: 1_000,
            ble_rotation_rssi_tolerance_db: 6,
            ble_correlation_window_ms: 30_000,
            storage_penalty: 4,
            outside_penalty: 4,
            shield_margin_db: 15,
        }
    }
}

/// 수거폰 보관 위치에서 나오는 신호의 세기 범위(설계 §4.3). 이 센서에서 이 범위로 보이면 감점한다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageZone {
    /// 보관 위치를 기준선 단계에서 등록한 센서.
    pub sensor: SensorId,
    /// 범위 하한(dBm, 포함).
    pub min_dbm: i16,
    /// 범위 상한(dBm, 포함).
    pub max_dbm: i16,
}

/// 센서 배치와 보정값.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensorSpec {
    /// 센서 식별자.
    pub id: SensorId,
    /// 교실 안 위치.
    pub position: SensorPos,
    /// 블루투스(BLE 광고)의 "교실 안" 판정 경계(dBm). 교실마다 보정한다(설계 §4.4). 없으면 경계를 쓰는 판정은
    /// 미루고 "보정 필요"를 보고한다 — 고정 기본값을 쓰지 않는다(AGENTS.md §2 규칙 4).
    pub inside_boundary_dbm: Option<i16>,
    /// 와이파이의 "교실 안" 판정 경계(dBm). 와이파이는 송신 출력·대역·안테나가 달라 블루투스와 세기 척도가 다르므로 따로
    /// 잰다(`docs/rules/L-room-calibration.md`, 사용자 결정 2026-11-16). 없으면 블루투스 경계를 빌리지 않고 와이파이의 경계
    /// 판정을 미룬다 — 출력이 센 와이파이에 블루투스 경계를 쓰면 옆 교실 AP까지 "교실 안"이 된다.
    #[serde(default)]
    pub wifi_inside_boundary_dbm: Option<i16>,
    /// 감독관이 알아볼 자리 설명(예: "칠판에서 셋째 줄 가운데"). 안내 문구의 "어디"에만 쓰고 판정에는 쓰지 않는다 — 처음 설정의
    /// 5×5 자리 고르기는 앞·뒤보다 자세하다. 없으면 `position`의 앞쪽·뒤쪽을 쓴다.
    #[serde(default)]
    pub place: Option<String>,
}

/// 경계를 고르는 무선 종류.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Radio {
    /// BLE 광고.
    Bluetooth,
    /// 와이파이.
    Wifi,
}

impl SensorSpec {
    /// 무선 종류에 맞는 교실 경계. 없으면 `None`(그 무선 종류의 경계 판정을 미룬다).
    pub fn boundary(&self, radio: Radio) -> Option<i16> {
        match radio {
            Radio::Bluetooth => self.inside_boundary_dbm,
            Radio::Wifi => self.wifi_inside_boundary_dbm,
        }
    }
}

/// 교실.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Room {
    /// 교실 식별자.
    pub id: RoomId,
    /// 교실의 센서(보통 앞·뒤 2개).
    pub sensors: Vec<SensorSpec>,
    /// 인접 교실. 위치 신뢰도가 낮으면 이 교실 감독관에게도 알린다(설계 §4.7).
    #[serde(default)]
    pub adjacent: Vec<RoomId>,
}

/// 건물 배치.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RoomLayout {
    /// 교실 목록.
    pub rooms: Vec<Room>,
    /// 수거폰 보관 위치.
    #[serde(default)]
    pub storage: Vec<StorageZone>,
}

impl RoomLayout {
    /// 센서가 놓인 교실과 그 센서 설정.
    pub fn sensor(&self, id: &SensorId) -> Option<(&Room, &SensorSpec)> {
        self.rooms
            .iter()
            .find_map(|room| room.sensors.iter().find(|s| &s.id == id).map(|s| (room, s)))
    }

    /// 교실.
    pub fn room(&self, id: &RoomId) -> Option<&Room> {
        self.rooms.iter().find(|r| &r.id == id)
    }

    /// 설정 오류: 센서 ID 중복, 교실 ID 중복, 없는 인접 교실.
    pub fn validate(&self) -> Result<(), LayoutError> {
        let mut rooms: Vec<&RoomId> = self.rooms.iter().map(|r| &r.id).collect();
        rooms.sort();
        if let Some(pair) = rooms.windows(2).find(|w| w[0] == w[1]) {
            return Err(LayoutError::DuplicateRoom(pair[0].clone()));
        }
        let mut sensors: Vec<&SensorId> = self
            .rooms
            .iter()
            .flat_map(|r| r.sensors.iter().map(|s| &s.id))
            .collect();
        sensors.sort();
        if let Some(pair) = sensors.windows(2).find(|w| w[0] == w[1]) {
            return Err(LayoutError::DuplicateSensor(pair[0].clone()));
        }
        let unknown = self
            .rooms
            .iter()
            .flat_map(|r| r.adjacent.iter())
            .find(|adj| rooms.binary_search(adj).is_err());
        if let Some(adj) = unknown {
            return Err(LayoutError::UnknownAdjacentRoom(adj.clone()));
        }
        Ok(())
    }
}

/// 배치 설정 오류.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    /// 같은 교실 ID가 두 번 나온다.
    #[error("교실 ID가 중복됐다: {0}")]
    DuplicateRoom(RoomId),
    /// 같은 센서 ID가 두 번 나온다.
    #[error("센서 ID가 중복됐다: {0}")]
    DuplicateSensor(SensorId),
    /// 인접 교실이 배치에 없다.
    #[error("인접 교실이 배치에 없다: {0}")]
    UnknownAdjacentRoom(RoomId),
}

/// 화이트리스트 항목: 감독관 장비, 사전 신고한 의료기기·보청기(설계 §4.1, §4.3).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WhitelistEntry {
    /// BLE 주소나 BSSID.
    Mac(Mac),
    /// 랜덤 주소를 쓰는 BLE 기기(CGM 등): 제조사 ID와 데이터 앞부분이 맞으면 제외한다.
    Manufacturer {
        /// 회사 ID.
        company: u16,
        /// 제조사 데이터 앞부분.
        prefix: Vec<u8>,
    },
}

/// 화이트리스트.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Whitelist {
    /// 항목.
    pub entries: Vec<WhitelistEntry>,
}
