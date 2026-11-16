//! 탐지 규칙 R1–R7(설계 §4.2). 규칙마다 `docs/rules/` 노트가 있다.
//!
//! 규칙은 관측 하나와 그 관측을 둘러싼 사실(기준선에 있었는지, 보고한 센서의 보정 경계, 대역 기준
//! 전력)만 보고 적중 여부를 정한다. 점수 합산과 위치 판정은 [`crate::engine`]이 한다.

use serde::{Deserialize, Serialize};

use crate::ids::ServiceUuid;
use crate::obs::{Observation, Ssid};

#[cfg(test)]
mod tests;

/// 폰 제조사 회사 ID: Apple, Samsung, Google(설계 §4.2 R1, 출처 [S1][S2]).
pub const PHONE_VENDOR_IDS: [u16; 3] = [0x004C, 0x0075, 0x00E0];

/// 스마트 안경 계열 회사 ID: Meta, Meta Platforms Technologies, Luxottica, Snap(설계 §4.2 R1a, 출처 [S3][S5b]).
/// 회사 ID 자체는 SIG 할당 번호지만 "안경이 이 ID로 광고한다"는 매핑은 보고된 내용이다(`docs/rules/R1a-glasses-vendor.md`).
// TODO(field-unverified): 기종별 광고 회사 ID·서비스 실측.
pub const GLASSES_VENDOR_IDS: [u16; 4] = [0x01AB, 0x058E, 0x0D53, 0x03C2];

/// Oculus 서비스 UUID(설계 §4.2 R1a, 출처 [S5b]).
pub const OCULUS_SERVICE: u16 = 0xFD5F;

/// Google Fast Pair 서비스(설계 §4.2 R1b, 출처 [S5]). 폰이 아니라 액세서리 쪽 광고다.
pub const FAST_PAIR_SERVICE: u16 = 0xFE2C;

/// Wi-Fi Direct 그룹 소유자의 SSID 접두사(Wi-Fi P2P 규격, 설계 §4.2 R6). 규격이 대문자로 정한다.
pub const WIFI_DIRECT_PREFIX: &str = "DIRECT-";

/// 규칙 식별자.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RuleId {
    /// 폰 제조사 ID.
    R1,
    /// 스마트 안경 제조사 ID·서비스.
    R1a,
    /// Fast Pair 액세서리.
    R1b,
    /// 새 BLE 광고자가 교실 경계 안.
    R2,
    /// 새 BSSID가 교실 경계 안.
    R3,
    /// locally administered BSSID.
    R4,
    /// 숨긴 SSID.
    R5,
    /// Wi-Fi Direct SSID.
    R6,
    /// LTE 업링크 대역 버스트.
    R7,
}

/// 규칙을 평가할 때 쓰는 사실.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleContext {
    /// 이 기기가 기준선에 있었는지. 기준선 기기는 점수를 받지 않는다(R7 제외 — R7은 상대 변화다).
    pub in_baseline: bool,
    /// 보고한 센서의 "교실 안" 경계(dBm). 없으면 R2·R3은 평가하지 않는다(AGENTS.md §2 규칙 4).
    pub inside_boundary_dbm: Option<i16>,
    /// R7: 그 대역의 기준선 전력(0.01dBm). 없으면 R7은 평가하지 않는다.
    pub rf_reference_cdbm: Option<i32>,
    /// R7: 버스트로 볼 상승폭(0.01dB).
    pub rf_burst_cdb: i32,
}

/// 관측 하나가 맞힌 규칙. 정렬돼 있다.
pub fn evaluate(obs: &Observation, ctx: &RuleContext) -> Vec<RuleId> {
    if let Observation::Rf(rf) = obs {
        let burst = ctx
            .rf_reference_cdbm
            .is_some_and(|base| rf.power_cdbm >= base.saturating_add(ctx.rf_burst_cdb));
        return if burst { vec![RuleId::R7] } else { Vec::new() };
    }
    if ctx.in_baseline {
        return Vec::new();
    }
    let inside = |rssi: i16| {
        ctx.inside_boundary_dbm
            .is_some_and(|boundary| rssi >= boundary)
    };
    let mut hits = Vec::new();
    match obs {
        Observation::Ble(ble) => {
            let companies = || ble.manufacturer.iter().map(|(company, _)| *company);
            let has_service = |short: u16| {
                let uuid = ServiceUuid::from_u16(short);
                ble.services.contains(&uuid) || ble.service_data.iter().any(|(u, _)| *u == uuid)
            };
            if companies().any(|c| PHONE_VENDOR_IDS.contains(&c)) {
                hits.push(RuleId::R1);
            }
            if companies().any(|c| GLASSES_VENDOR_IDS.contains(&c)) || has_service(OCULUS_SERVICE) {
                hits.push(RuleId::R1a);
            }
            if has_service(FAST_PAIR_SERVICE) {
                hits.push(RuleId::R1b);
            }
            if inside(ble.rssi) {
                hits.push(RuleId::R2);
            }
        }
        Observation::Wifi(wifi) => {
            if inside(wifi.rssi) {
                hits.push(RuleId::R3);
            }
            if wifi.bssid.is_locally_administered() && !wifi.bssid.is_multicast() {
                hits.push(RuleId::R4);
            }
            match &wifi.ssid {
                Ssid::Hidden => hits.push(RuleId::R5),
                Ssid::Named(name) if name.starts_with(WIFI_DIRECT_PREFIX) => hits.push(RuleId::R6),
                Ssid::Named(_) => {}
            }
        }
        Observation::Rf(_) => {}
    }
    hits
}
