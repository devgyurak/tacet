//! 기기 키와 BLE 상관.
//!
//! BSSID는 어느 센서에서 보든 같은 값이라 그대로 키가 된다(설계 §4.7). BLE
//! 광고는 주소가 바뀌고(랜덤 MAC) macOS는 호스트마다 다른 UUID를 주므로, 같은 시간 창 안의 제조사
//! 데이터 일치로 묶는다(설계 §4.2, §7.2). macOS UUID는 다른 센서의 값과 직접 비교하지 않는다.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{BleId, Mac, SensorId};
use crate::obs::{BleObs, CellBand, Observation};
use crate::time::Timestamp;

#[cfg(test)]
mod tests;

/// 판정 단위가 되는 기기 키.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DeviceKey {
    /// BLE 주소(Windows 센서).
    BleAddr(Mac),
    /// macOS 센서의 호스트별 UUID. 센서와 짝으로만 의미가 있다.
    BleHostLocal(SensorId, u128),
    /// Wi-Fi BSSID.
    Bssid(Mac),
    /// 셀룰러 업링크 대역.
    Cell(CellBand),
}

impl DeviceKey {
    /// 관측이 그대로 주는 키(상관 전).
    pub fn raw(sensor: &SensorId, obs: &Observation) -> Self {
        match obs {
            Observation::Ble(ble) => match ble.id {
                BleId::Addr(mac) => DeviceKey::BleAddr(mac),
                BleId::HostLocal(uuid) => DeviceKey::BleHostLocal(sensor.clone(), uuid),
            },
            Observation::Wifi(wifi) => DeviceKey::Bssid(wifi.bssid),
            Observation::Rf(rf) => DeviceKey::Cell(rf.band),
        }
    }

    /// macOS 호스트별 UUID 키인지. 이런 키가 섞인 위치 판정은 신뢰도를 한 단계 낮춘다.
    pub fn is_host_local(&self) -> bool {
        matches!(self, DeviceKey::BleHostLocal(..))
    }
}

/// BLE 제조사 데이터 지문. 데이터가 없는 광고는 상관하지 않는다.
fn fingerprint(ble: &BleObs) -> Option<Vec<u8>> {
    if ble.manufacturer.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for (company, data) in &ble.manufacturer {
        out.extend_from_slice(&company.to_le_bytes());
        out.extend_from_slice(&u16::try_from(data.len()).unwrap_or(u16::MAX).to_le_bytes());
        out.extend_from_slice(data);
    }
    Some(out)
}

#[derive(Debug, Clone)]
struct Track {
    canonical: DeviceKey,
    sensor: SensorId,
    rssi: i16,
    last_seen: Timestamp,
}

/// BLE 관측을 같은 기기로 묶는 상관기.
#[derive(Debug, Clone, Default)]
pub struct Correlator {
    by_fingerprint: BTreeMap<Vec<u8>, Track>,
    by_raw: BTreeMap<DeviceKey, DeviceKey>,
    host_local: std::collections::BTreeSet<DeviceKey>,
}

/// 지문 표가 이만큼 커지면 시간 창 밖 항목을 치운다.
const PRUNE_AT: usize = 1024;

impl Correlator {
    /// 관측의 정규 키. BLE가 아니면 원래 키 그대로다.
    ///
    /// 같은 제조사 데이터가 `window_ms` 안에 다시 보이면 같은 기기로 본다. 같은 센서라면 RSSI 차이도
    /// `tolerance_db` 안이어야 한다(주소 교체). 다른 센서라면 시간 창만 본다(macOS UUID 상관).
    pub fn canonical(
        &mut self,
        sensor: &SensorId,
        obs: &Observation,
        window_ms: u64,
        tolerance_db: i16,
    ) -> DeviceKey {
        let raw = DeviceKey::raw(sensor, obs);
        let Observation::Ble(ble) = obs else {
            return raw;
        };
        if let Some(known) = self.by_raw.get(&raw) {
            let known = known.clone();
            self.touch(ble, sensor, &known);
            return known;
        }
        let canonical = fingerprint(ble)
            .and_then(|fp| self.by_fingerprint.get(&fp))
            .filter(|track| ble.at.since(track.last_seen) <= window_ms)
            .filter(|track| {
                &track.sensor != sensor
                    || (i32::from(track.rssi) - i32::from(ble.rssi)).abs()
                        <= i32::from(tolerance_db)
            })
            .map_or_else(|| raw.clone(), |track| track.canonical.clone());
        if raw.is_host_local() {
            self.host_local.insert(canonical.clone());
        }
        self.by_raw.insert(raw, canonical.clone());
        self.touch(ble, sensor, &canonical);
        // 오래된 지문은 다시 묶일 일이 없으니 가끔 치운다(세션 내내 쌓이지 않게).
        if self.by_fingerprint.len() > PRUNE_AT {
            self.by_fingerprint
                .retain(|_, track| ble.at.since(track.last_seen) <= window_ms);
        }
        canonical
    }

    /// 정규 키에 macOS 호스트별 UUID가 하나라도 묶였는지. 위치 신뢰도를 한 단계 낮추는 데 쓴다.
    pub fn has_host_local_member(&self, canonical: &DeviceKey) -> bool {
        self.host_local.contains(canonical)
    }

    fn touch(&mut self, ble: &BleObs, sensor: &SensorId, canonical: &DeviceKey) {
        if let Some(fp) = fingerprint(ble) {
            let track = Track {
                canonical: canonical.clone(),
                sensor: sensor.clone(),
                rssi: ble.rssi,
                last_seen: ble.at,
            };
            self.by_fingerprint.insert(fp, track);
        }
    }

    /// 정규 키 `canonical`로 묶인 원래 키들(정렬됨).
    pub fn members_of(&self, canonical: &DeviceKey) -> Vec<DeviceKey> {
        self.by_raw
            .iter()
            .filter(|(_, c)| *c == canonical)
            .map(|(raw, _)| raw.clone())
            .collect()
    }
}
