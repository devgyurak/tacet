//! 광고 데이터 → [`BleObs`] 변환(순수 함수). 모든 OS에서 사양대로 조립한 바이트로 테스트한다.
//!
//! AD 구조 형식은 Bluetooth 코어 규격 Vol 3 Part C §11과 Supplement to the Core Specification Part A를 따른다.

use tacet_core::ids::{BleId, Mac, ServiceUuid};
use tacet_core::obs::BleObs;
use tacet_core::time::Timestamp;

#[cfg(test)]
mod tests;

/// AD 형식: 16비트 서비스 UUID 목록(불완전·완전).
pub const AD_UUID16_INCOMPLETE: u8 = 0x02;
/// AD 형식: 16비트 서비스 UUID 완전 목록.
pub const AD_UUID16_COMPLETE: u8 = 0x03;
/// AD 형식: 128비트 서비스 UUID 불완전 목록.
pub const AD_UUID128_INCOMPLETE: u8 = 0x06;
/// AD 형식: 128비트 서비스 UUID 완전 목록.
pub const AD_UUID128_COMPLETE: u8 = 0x07;
/// AD 형식: 16비트 UUID 서비스 데이터.
pub const AD_SERVICE_DATA16: u8 = 0x16;
/// AD 형식: 제조사 데이터.
pub const AD_MANUFACTURER: u8 = 0xFF;
/// AD 형식: 짧은 기기 이름(Shortened Local Name).
pub const AD_NAME_SHORT: u8 = 0x08;
/// AD 형식: 완전한 기기 이름(Complete Local Name).
pub const AD_NAME_COMPLETE: u8 = 0x09;

/// 플랫폼이 준 광고 한 건(정렬 전).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawAdvertisement {
    /// 주소(Windows). macOS는 주지 않는다.
    pub addr: Option<[u8; 6]>,
    /// 호스트별 UUID(macOS).
    pub host_uuid: Option<u128>,
    /// RSSI(dBm).
    pub rssi: Option<i16>,
    /// 제조사 데이터.
    pub manufacturer: Vec<(u16, Vec<u8>)>,
    /// 서비스 UUID.
    pub services: Vec<u128>,
    /// 서비스 데이터.
    pub service_data: Vec<(u128, Vec<u8>)>,
    /// 광고 이름(정리 전 바이트). 교실 보정 목록에만 쓴다.
    pub name: Option<Vec<u8>>,
}

/// Windows가 주는 64비트 주소 값(하위 48비트)을 MAC 바이트로 바꾼다.
pub fn addr_from_u64(value: u64) -> [u8; 6] {
    let b = value.to_be_bytes();
    [b[2], b[3], b[4], b[5], b[6], b[7]]
}

/// UUID 문자열(하이픈 포함 가능)을 128비트 값으로 읽는다.
pub fn uuid_from_text(text: &str) -> Option<u128> {
    let hex: String = text.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        return None;
    }
    u128::from_str_radix(&hex, 16).ok()
}

/// AD 데이터 구간 하나(형식, 내용)를 광고에 반영한다. 모르는 형식과 길이가 틀린 구간은 버린다.
pub fn apply_section(raw: &mut RawAdvertisement, ad_type: u8, data: &[u8]) {
    match ad_type {
        AD_UUID16_INCOMPLETE | AD_UUID16_COMPLETE => {
            raw.services.extend(
                data.as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| ServiceUuid::from_u16(u16::from_le_bytes(*c)).0),
            );
        }
        AD_UUID128_INCOMPLETE | AD_UUID128_COMPLETE => {
            raw.services.extend(
                data.as_chunks::<16>()
                    .0
                    .iter()
                    .map(|c| u128::from_le_bytes(*c)),
            );
        }
        AD_SERVICE_DATA16 if data.len() >= 2 => {
            let uuid = ServiceUuid::from_u16(u16::from_le_bytes([data[0], data[1]])).0;
            raw.service_data.push((uuid, data[2..].to_vec()));
        }
        // 완전한 이름이 짧은 이름보다 앞선다(Supplement to the Core Specification Part A §1.2).
        AD_NAME_COMPLETE => raw.name = Some(data.to_vec()),
        AD_NAME_SHORT if raw.name.is_none() => raw.name = Some(data.to_vec()),
        AD_MANUFACTURER if data.len() >= 2 => {
            raw.manufacturer
                .push((u16::from_le_bytes([data[0], data[1]]), data[2..].to_vec()));
        }
        _ => {}
    }
}

/// 원시 광고 패킷(AD 구조의 연속)을 구간으로 나눈다. 길이가 남은 바이트를 넘으면 거기서 멈춘다.
pub fn parse_ad_structures(payload: &[u8]) -> Vec<(u8, &[u8])> {
    let mut out = Vec::new();
    let mut rest = payload;
    while let Some((&len, tail)) = rest.split_first() {
        let len = usize::from(len);
        if len == 0 || len > tail.len() {
            break;
        }
        let (section, next) = tail.split_at(len);
        out.push((section[0], &section[1..]));
        rest = next;
    }
    out
}

/// 이름 다루기를 적용한다: 감지 중이면 광고 이름을 버린다(AGENTS.md §3.2).
pub fn apply_names(mut obs: BleObs, names: crate::Names) -> BleObs {
    if names == crate::Names::Drop {
        obs.name = None;
    }
    obs
}

/// 관측값으로 바꾼다. RSSI가 없거나 식별자가 없으면 `None`. 주소가 전부 0이면(macOS) UUID를 쓴다.
pub fn to_obs(raw: RawAdvertisement, at: Timestamp) -> Option<BleObs> {
    let rssi = raw.rssi?;
    let id = match (
        raw.addr.filter(|a| a.iter().any(|b| *b != 0)),
        raw.host_uuid,
    ) {
        (Some(addr), _) => BleId::Addr(Mac(addr)),
        (None, Some(uuid)) => BleId::HostLocal(uuid),
        (None, None) => return None,
    };
    let mut manufacturer = raw.manufacturer;
    manufacturer.sort();
    let mut services: Vec<ServiceUuid> = raw.services.into_iter().map(ServiceUuid).collect();
    services.sort();
    services.dedup();
    let mut service_data: Vec<(ServiceUuid, Vec<u8>)> = raw
        .service_data
        .into_iter()
        .map(|(u, d)| (ServiceUuid(u), d))
        .collect();
    service_data.sort();
    Some(BleObs {
        id,
        rssi,
        manufacturer,
        services,
        service_data,
        name: raw.name.as_deref().and_then(tacet_core::obs::ble_name),
        at,
    })
}
