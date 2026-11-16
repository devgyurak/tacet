//! 판정 엔진. 단독 모드와 서버 모드가 같은 엔진을 쓴다(AGENTS.md §2 "실행 모드").
//!
//! 흐름: 센서 보고서 → 화이트리스트 → 기기 상관 → (기준선 동안) 기준선 → 규칙 → 점수 → 위치 판정 →
//! 알림 후보. 같은 설정과 같은 보고서 열이면 언제나 같은 결과가 나온다(AGENTS.md §2 규칙 3).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::baseline::Baseline;
use crate::config::{DetectorConfig, Radio, RoomLayout, Whitelist, WhitelistEntry};
use crate::device::{Correlator, DeviceKey};
use crate::ids::{BleId, Mac, RoomId, SensorId};
use crate::locate::{Confidence, Localization, locate_radio, lower_median};
use crate::obs::{CellBand, Observation, SensorReport, Ssid};
use crate::rules::{RuleContext, RuleId, evaluate};
use crate::time::Timestamp;

#[cfg(test)]
mod tests;

/// 세션 단계.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    /// 기준선 수집 중(설계 §4.1).
    Baseline,
    /// 감시 중.
    Watching,
}

/// 의심 강도.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Level {
    /// 낮음.
    Low,
    /// 중간.
    Medium,
    /// 높음.
    High,
}

/// 감독관 화면에 보일 기기 설명. 세션 중 표시만 하고 저장하지 않는다(AGENTS.md §3.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceLabel {
    /// BLE 광고자. 알려진 제조사 회사 ID가 있으면 담는다.
    Ble {
        /// 첫 제조사 회사 ID.
        company: Option<u16>,
    },
    /// Wi-Fi 네트워크.
    Wifi {
        /// BSSID.
        bssid: Mac,
        /// 숨긴 SSID인지.
        hidden: bool,
        /// Wi-Fi Direct인지.
        direct: bool,
    },
    /// 셀룰러 업링크.
    Cell {
        /// 대역.
        band: CellBand,
    },
}

impl DeviceLabel {
    fn of(obs: &Observation) -> Self {
        match obs {
            Observation::Ble(ble) => DeviceLabel::Ble {
                company: ble.manufacturer.first().map(|(c, _)| *c),
            },
            Observation::Wifi(wifi) => DeviceLabel::Wifi {
                bssid: wifi.bssid,
                hidden: wifi.ssid == Ssid::Hidden,
                direct: matches!(&wifi.ssid, Ssid::Named(n) if n.starts_with(crate::rules::WIFI_DIRECT_PREFIX)),
            },
            Observation::Rf(rf) => DeviceLabel::Cell { band: rf.band },
        }
    }
}

/// 알림 후보. 문구는 [`crate::text`]가 만든다 — 언제나 "확인 필요"다(AGENTS.md §3.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertCandidate {
    /// 기기 키.
    pub device: DeviceKey,
    /// 화면 설명.
    pub label: DeviceLabel,
    /// 의심 강도.
    pub level: Level,
    /// 의심 점수.
    pub score: u32,
    /// 시간 창 안에서 맞힌 규칙(정렬됨).
    pub rules: Vec<RuleId>,
    /// 위치 판정.
    pub localization: Option<Localization>,
    /// 알릴 교실: 추정 교실, 신뢰도가 낮으면 인접 교실까지(설계 §4.7). 교실 미정이면 비어 있다(전체 알림).
    pub notify_rooms: Vec<RoomId>,
    /// 점수에서 뺀 감점(정렬됨). 감독관이 점수가 낮은 이유를 알 수 있게 보인다.
    pub penalties: Vec<Penalty>,
    /// 만든 시각.
    pub at: Timestamp,
}

/// 감점 사유(설계 §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Penalty {
    /// 수거폰 보관 위치에서 나온 BLE 폰 광고. 폰 광고 규칙(R1·R1b·R2) 점수에만 건다.
    StorageZone,
    /// 모든 보정된 센서에서 교실 경계보다 인체 차폐 여유 이상 약하다 — 건물 밖 가능성.
    FarOutside,
}

/// 엔진이 보고하는 설정·운영 경고.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EngineWarning {
    /// 센서에 교실 보정값이 없어 경계 판정(R2·R3, 교실 안팎)을 미뤘다(AGENTS.md §2 규칙 4).
    CalibrationMissing(SensorId),
    /// 배치에 없는 센서의 보고서를 버렸다.
    UnknownSensor(SensorId),
    /// macOS 센서는 BLE 주소를 주지 않아 MAC 화이트리스트가 그 센서의 BLE 관측에 먹지 않는다. 의료기기는
    /// 제조사 데이터 화이트리스트로 등록해야 한다(설계 §7.2).
    MacWhitelistNotApplicable(SensorId),
    /// 센서에 와이파이 교실 보정값이 없어 와이파이의 경계 판정(R3, 교실 안팎)을 미뤘다. 블루투스 경계는 빌리지 않는다.
    WifiCalibrationMissing(SensorId),
}

fn radio_of(obs: &Observation) -> Radio {
    match obs {
        Observation::Wifi(_) => Radio::Wifi,
        _ => Radio::Bluetooth,
    }
}

#[derive(Debug, Clone)]
struct Alerted {
    level: Level,
    room: Option<RoomId>,
    notified: BTreeSet<RoomId>,
}

#[derive(Debug, Clone)]
struct DeviceState {
    label: DeviceLabel,
    is_ble: bool,
    hits: BTreeMap<RuleId, Timestamp>,
    strength: BTreeMap<SensorId, VecDeque<(Timestamp, i16)>>,
    alerted: Option<Alerted>,
    /// 마지막으로 문턱 이상이었던 시각. 점수 창보다 오래 조용했으면 다음 알림은 새 알림이다.
    last_active: Option<Timestamp>,
}

/// 판정 엔진.
#[derive(Debug, Clone)]
pub struct Engine {
    cfg: DetectorConfig,
    layout: RoomLayout,
    whitelist: Whitelist,
    baseline: Baseline,
    correlator: Correlator,
    devices: BTreeMap<DeviceKey, DeviceState>,
    warnings: BTreeSet<EngineWarning>,
    now: Timestamp,
}

/// 폰 광고 규칙. 보관 위치 감점은 이 규칙들의 점수에만 건다.
const PHONE_ADVERT_RULES: [RuleId; 3] = [RuleId::R1, RuleId::R1b, RuleId::R2];

impl Engine {
    /// 세션 시작(시각 0)에 엔진을 만든다.
    pub fn new(cfg: DetectorConfig, layout: RoomLayout, whitelist: Whitelist) -> Self {
        Self {
            cfg,
            layout,
            whitelist,
            baseline: Baseline::default(),
            correlator: Correlator::default(),
            devices: BTreeMap::new(),
            warnings: BTreeSet::new(),
            now: Timestamp(0),
        }
    }

    /// 설정.
    pub fn config(&self) -> &DetectorConfig {
        &self.cfg
    }

    /// 배치.
    pub fn layout(&self) -> &RoomLayout {
        &self.layout
    }

    /// 현재 단계. 지금 시각이 기준선 시간을 지났으면 감시 중이다.
    pub fn phase(&self) -> Phase {
        if self.now.0 >= self.cfg.baseline_ms {
            Phase::Watching
        } else {
            Phase::Baseline
        }
    }

    /// 기준선이 끝나기까지 남은 밀리초.
    pub fn baseline_remaining_ms(&self) -> u64 {
        Timestamp(self.cfg.baseline_ms).since(self.now)
    }

    /// 기준선 기기 수.
    pub fn baseline_devices(&self) -> usize {
        self.baseline.device_count()
    }

    /// 지금까지의 경고(정렬됨).
    pub fn warnings(&self) -> Vec<EngineWarning> {
        self.warnings.iter().cloned().collect()
    }

    /// 시각을 앞으로 옮긴다. 시각은 뒤로 가지 않는다. 미래 시각 검증은 보고서를 받는 쪽(서버)이 한다.
    pub fn tick(&mut self, now: Timestamp) {
        self.now = self.now.max(now);
    }

    /// 보고서 하나를 처리하고, 새로 생기거나 강해지거나 교실이 바뀐 알림 후보를 돌려준다.
    ///
    /// 기준선 여부는 엔진의 현재 시각이 아니라 **보고서 시각**으로 정한다 — 기준선 끝 직전 관측이 늦게
    /// 도착해도 기준선에 들어간다. 정확히 `baseline_ms`인 관측은 감시 단계다.
    pub fn ingest(&mut self, report: &SensorReport) -> Vec<AlertCandidate> {
        let obs = &report.obs;
        self.tick(obs.at());
        if self.is_whitelisted(obs) {
            return Vec::new();
        }
        let Some((_, spec)) = self.layout.sensor(&report.sensor) else {
            self.warnings
                .insert(EngineWarning::UnknownSensor(report.sensor.clone()));
            return Vec::new();
        };
        let radio = radio_of(obs);
        let boundary = spec.boundary(radio);
        match (boundary, radio, obs) {
            (Some(_), _, _) | (None, _, Observation::Rf(_)) => {}
            (None, Radio::Wifi, _) => {
                self.warnings
                    .insert(EngineWarning::WifiCalibrationMissing(report.sensor.clone()));
            }
            (None, Radio::Bluetooth, _) => {
                self.warnings
                    .insert(EngineWarning::CalibrationMissing(report.sensor.clone()));
            }
        }
        if let Observation::Ble(ble) = obs
            && matches!(ble.id, BleId::HostLocal(_))
            && self
                .whitelist
                .entries
                .iter()
                .any(|e| matches!(e, WhitelistEntry::Mac(_)))
        {
            self.warnings
                .insert(EngineWarning::MacWhitelistNotApplicable(
                    report.sensor.clone(),
                ));
        }
        let key = self.correlator.canonical(
            &report.sensor,
            obs,
            self.cfg.ble_correlation_window_ms,
            self.cfg.ble_rotation_rssi_tolerance_db,
        );

        if obs.at().0 < self.cfg.baseline_ms {
            self.baseline.add_device(key);
            if let Observation::Rf(rf) = obs {
                self.baseline.add_rf(&report.sensor, rf.band, rf.power_cdbm);
            }
            return Vec::new();
        }

        let ctx = RuleContext {
            in_baseline: self.baseline.contains(&key),
            inside_boundary_dbm: boundary,
            rf_reference_cdbm: match obs {
                Observation::Rf(rf) => self.baseline.rf_reference(&report.sensor, rf.band),
                _ => None,
            },
            rf_burst_cdb: self.cfg.rf_burst_cdb,
        };
        let hits = evaluate(obs, &ctx);
        if hits.is_empty() && !self.devices.contains_key(&key) {
            return Vec::new();
        }
        let at = obs.at();
        let state = self
            .devices
            .entry(key.clone())
            .or_insert_with(|| DeviceState {
                label: DeviceLabel::of(obs),
                is_ble: matches!(obs, Observation::Ble(_)),
                hits: BTreeMap::new(),
                strength: BTreeMap::new(),
                alerted: None,
                last_active: None,
            });
        // 늦게 도착한 보고서가 적중 시각을 과거로 되돌리지 않게 가장 늦은 시각만 남긴다. 세기 표본은 시각 순으로
        // 끼워 넣어 오래된 표본 정리가 앞에서부터 맞게 동작하게 한다.
        for rule in hits {
            let last = state.hits.entry(rule).or_insert(at);
            *last = (*last).max(at);
        }
        if let Some(strength) = obs.strength_dbm() {
            let samples = state.strength.entry(report.sensor.clone()).or_default();
            let index = samples.partition_point(|(t, _)| *t <= at);
            samples.insert(index, (at, strength));
        }
        self.evaluate_device(&key, at).into_iter().collect()
    }

    fn is_whitelisted(&self, obs: &Observation) -> bool {
        self.whitelist
            .entries
            .iter()
            .any(|entry| match (entry, obs) {
                (WhitelistEntry::Mac(mac), Observation::Ble(ble)) => ble.id == BleId::Addr(*mac),
                (WhitelistEntry::Mac(mac), Observation::Wifi(wifi)) => wifi.bssid == *mac,
                (WhitelistEntry::Manufacturer { company, prefix }, Observation::Ble(ble)) => ble
                    .manufacturer
                    .iter()
                    .any(|(c, data)| c == company && data.starts_with(prefix)),
                _ => false,
            })
    }

    /// 지금 알림 상태인 기기마다 현재 판정(대시보드의 "지금 상태"). 알림 이벤트와 달리 신뢰도가
    /// 오르거나 근거가 바뀐 것도 반영한다. 기기 키 순으로 정렬된다.
    pub fn snapshot(&self) -> Vec<AlertCandidate> {
        self.devices
            .iter()
            .filter(|(_, state)| state.alerted.is_some())
            .filter_map(|(key, _)| self.assess(key, self.now))
            .collect()
    }

    /// 기기 하나의 현재 판정. 문턱 아래면 `None`. 상태를 바꾸지 않는다.
    fn assess(&self, key: &DeviceKey, at: Timestamp) -> Option<AlertCandidate> {
        let cfg = &self.cfg;
        let state = self.devices.get(key)?;
        let host_local = key.is_host_local() || self.correlator.has_host_local_member(key);
        let window: BTreeMap<SensorId, Vec<i16>> = state
            .strength
            .iter()
            .map(|(id, samples)| {
                let recent = samples
                    .iter()
                    .filter(|(t, _)| at.since(*t) <= cfg.locate_window_ms)
                    .map(|(_, v)| *v);
                (id.clone(), recent.collect::<Vec<i16>>())
            })
            .filter(|(_, values)| !values.is_empty())
            .collect();
        let radio = match state.label {
            DeviceLabel::Wifi { .. } => Radio::Wifi,
            _ => Radio::Bluetooth,
        };
        let localization = locate_radio(&window, &self.layout, host_local, cfg, radio);

        let rules: Vec<RuleId> = state
            .hits
            .iter()
            .filter(|(_, t)| at.since(**t) <= cfg.score_window_ms)
            .map(|(r, _)| *r)
            .collect();
        let gained = rules
            .iter()
            .fold(0u32, |sum, r| sum.saturating_add(cfg.weights.of(*r)));

        let mut penalties = Vec::new();
        let mut penalty = 0u32;
        // 보관 위치: BLE 기기이고, 보관 위치 센서의 최근 중앙값이 그 범위에 있을 때만. 폰 광고 점수까지만 뺀다.
        let in_storage = state.is_ble
            && self.layout.storage.iter().any(|zone| {
                window
                    .get(&zone.sensor)
                    .and_then(|v| lower_median(v))
                    .is_some_and(|m| (zone.min_dbm..=zone.max_dbm).contains(&m))
            });
        if in_storage {
            let phone_points = rules
                .iter()
                .filter(|r| PHONE_ADVERT_RULES.contains(r))
                .fold(0u32, |sum, r| sum.saturating_add(cfg.weights.of(*r)));
            penalty = penalty.saturating_add(cfg.storage_penalty.min(phone_points));
            penalties.push(Penalty::StorageZone);
        }
        if localization.as_ref().is_some_and(|l| l.likely_outside) {
            penalty = penalty.saturating_add(cfg.outside_penalty);
            penalties.push(Penalty::FarOutside);
        }
        let score = gained.saturating_sub(penalty);
        let level = if score >= cfg.high_score {
            Level::High
        } else if score >= cfg.medium_score {
            Level::Medium
        } else if score >= cfg.alert_score {
            Level::Low
        } else {
            return None;
        };
        let notify_rooms = notify_rooms(&self.layout, localization.as_ref());
        Some(AlertCandidate {
            device: key.clone(),
            label: state.label.clone(),
            level,
            score,
            rules,
            localization,
            notify_rooms,
            penalties,
            at,
        })
    }

    fn evaluate_device(&mut self, key: &DeviceKey, at: Timestamp) -> Option<AlertCandidate> {
        let window_ms = self.cfg.locate_window_ms.max(self.cfg.score_window_ms);
        let quiet_ms = self.cfg.score_window_ms;
        if let Some(state) = self.devices.get_mut(key) {
            // 오래된 세기 표본은 버려 메모리를 묶어 둔다. 판정 창보다 오래된 것만 버린다.
            for samples in state.strength.values_mut() {
                while samples
                    .front()
                    .is_some_and(|(t, _)| at.since(*t) > window_ms)
                {
                    samples.pop_front();
                }
            }
            state.strength.retain(|_, samples| !samples.is_empty());
            // 점수 창보다 오래 조용했던 기기가 돌아오면 새 알림이다.
            if state.last_active.is_some_and(|t| at.since(t) > quiet_ms) {
                state.alerted = None;
            }
        }
        let candidate = self.assess(key, at);
        let state = self.devices.get_mut(key)?;
        let candidate = candidate?;
        state.last_active = Some(at);
        let room = candidate.localization.as_ref().and_then(|l| l.room.clone());
        let confident = candidate
            .localization
            .as_ref()
            .is_some_and(|l| l.confidence >= Confidence::Medium);
        let changed = match &state.alerted {
            None => true,
            Some(prev) => {
                candidate.level > prev.level
                    // 교실 변경은 새 판정의 신뢰도가 "중" 이상일 때만 — 비슷한 두 교실 사이의 흔들림으로 다시 알리지 않는다.
                    || (room != prev.room && confident)
                    || candidate.notify_rooms.iter().any(|r| !prev.notified.contains(r))
            }
        };
        if !changed {
            return None;
        }
        let previous = state.alerted.take();
        let mut notified = previous
            .as_ref()
            .map(|p| p.notified.clone())
            .unwrap_or_default();
        notified.extend(candidate.notify_rooms.iter().cloned());
        let kept_room = match &previous {
            Some(p) if room != p.room && !confident => p.room.clone(),
            _ => room,
        };
        let level = previous
            .as_ref()
            .map_or(candidate.level, |p| p.level.max(candidate.level));
        state.alerted = Some(Alerted {
            level,
            room: kept_room,
            notified,
        });
        Some(candidate)
    }
}

/// 알릴 교실: 추정 교실과, 신뢰도가 낮으면 그 인접 교실(설계 §4.7). 정렬·중복 제거됨.
fn notify_rooms(layout: &RoomLayout, localization: Option<&Localization>) -> Vec<RoomId> {
    let Some(loc) = localization else {
        return Vec::new();
    };
    let Some(room) = &loc.room else {
        return Vec::new();
    };
    let mut rooms = vec![room.clone()];
    if loc.confidence == Confidence::Low {
        rooms.extend(
            layout
                .room(room)
                .map(|r| r.adjacent.clone())
                .unwrap_or_default(),
        );
    }
    rooms.sort();
    rooms.dedup();
    rooms
}
