//! 스캔 결과 → [`WifiObs`] 변환(순수 함수). 모든 OS에서 합성 입력으로 테스트한다.

use tacet_core::ids::Mac;
use tacet_core::obs::{
    Band, CollectorStatus, OffReason, Permission, Ssid, WifiObs, band_channel_from_mhz,
};
use tacet_core::time::Timestamp;

#[cfg(test)]
mod tests;

/// RSSI를 i16 범위로 자른다(OS가 i32·NSInteger로 준다).
fn clamp_rssi(rssi: i64) -> i16 {
    i16::try_from(rssi.clamp(i64::from(i16::MIN), i64::from(i16::MAX))).unwrap_or(i16::MIN)
}

/// Windows `WLAN_BSS_ENTRY` 필드로 만든다. SSID 길이가 32를 넘으면 32로 자른다. 주파수를 모르면 `None`.
pub fn from_windows(
    bssid: [u8; 6],
    ssid: &[u8],
    ssid_len: u32,
    rssi: i32,
    center_khz: u32,
    at: Timestamp,
) -> Option<WifiObs> {
    let len = usize::try_from(ssid_len)
        .unwrap_or(usize::MAX)
        .min(ssid.len())
        .min(32);
    let (band, channel) = band_channel_from_mhz(center_khz / 1000)?;
    Some(WifiObs {
        bssid: Mac(bssid),
        ssid: Ssid::from_bytes(&ssid[..len]),
        rssi: clamp_rssi(i64::from(rssi)),
        channel,
        band,
        at,
    })
}

/// CoreWLAN `CWChannelBand` 값(0 모름, 1 2.4GHz, 2 5GHz, 3 6GHz)을 대역으로.
pub fn band_from_corewlan(code: i64) -> Option<Band> {
    match code {
        1 => Some(Band::B2_4),
        2 => Some(Band::B5),
        3 => Some(Band::B6),
        _ => None,
    }
}

/// CoreWLAN `CWNetwork` 필드로 만든다. BSSID가 없으면(위치 권한 없음) `None`. SSID 데이터가 없거나 비면 숨긴 네트워크다.
pub fn from_corewlan(
    bssid: Option<&str>,
    ssid_data: Option<&[u8]>,
    rssi: i64,
    channel: i64,
    band_code: i64,
    at: Timestamp,
) -> Option<WifiObs> {
    let bssid = Mac::parse(bssid?)?;
    let band = band_from_corewlan(band_code)?;
    let channel = u16::try_from(channel).ok()?;
    let ssid = ssid_data.map_or(Ssid::Hidden, Ssid::from_bytes);
    Some(WifiObs {
        bssid,
        ssid,
        rssi: clamp_rssi(rssi),
        channel,
        band,
        at,
    })
}

/// CoreWLAN 스캔 결과로 수집기 상태를 정한다. 네트워크가 보이는데 모두 BSSID가 비어 있으면 위치 권한이
/// 없는 것이다(설계 §7.2). 아무것도 안 보이면 권한 여부를 알 수 없으므로 수집 중으로 둔다.
pub fn corewlan_status(total: usize, without_bssid: usize) -> CollectorStatus {
    if total > 0 && without_bssid == total {
        CollectorStatus::Off(OffReason::PermissionDenied(Permission::Location))
    } else {
        CollectorStatus::Running
    }
}
