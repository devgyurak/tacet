//! 앱 안 교실 보정(`docs/rules/L-room-calibration.md`, 설계 §4.4).
//!
//! 감독관이 지금 잡히는 블루투스 기기와 와이파이 목록에서 자기 휴대폰(블루투스)과 핫스팟을 하나씩 고르고, 그 휴대폰을 교실
//! 끝에 두면, 두 신호가 교실 끝에서 이 노트북에 얼마나 세게 잡히는지(아래쪽 중앙값)를 블루투스·와이파이 교실 경계로 정한다
//! (사용자 결정 2026-11-16: 목록에서 고르기, 둘 다 필수). 고정 dBm 판정 상수는 없다 — 비교는 같은 기기의 고를 때 세기와 교실
//! 끝 세기 사이의 차이뿐이다(AGENTS.md §2 규칙 4). 순수하다: 시각은 값으로 받는다.
//!
//! 목록에는 주변 사람의 기기 이름·와이파이 이름이 보인다. 세션 중 화면 표시만 하고 저장하지 않는다(AGENTS.md §3.3).

use std::collections::{BTreeMap, VecDeque};

use serde::Serialize;

use crate::config::Radio;
use crate::device::DeviceKey;
use crate::ids::SensorId;
use crate::locate::lower_median;
use crate::obs::{Band, Observation, Ssid};
use crate::text;
use crate::time::Timestamp;

#[cfg(test)]
mod tests;

/// 이만큼 안에 본 기기만 목록에 보인다(ms). 와이파이 스캔 간격(약 8초)보다 넉넉하다.
pub const LIST_WINDOW_MS: u64 = 20_000;
/// 목록에 보일 무선 종류별 최대 기기 수(센 순).
pub const LIST_LIMIT: usize = 30;
/// 고를 때 세기를 낼 최근 표본 수.
pub const RECENT_SAMPLES: usize = 5;
/// 교실 끝 단계의 최소 길이(ms). 흔들림을 고르게 하려고 몇 번 이상 잰다.
// TODO(field-unverified): 교실 끝 RSSI 흔들림([S8])을 덮는 데 필요한 시간.
pub const FAR_MS: u64 = 20_000;
/// 교실 끝에서 이만큼 지나도 표본이 모자라면 실패한다(ms). 와이파이는 스캔이 느려(약 8초마다) 넉넉히 둔다.
// TODO(field-unverified)
pub const FAR_TIMEOUT_MS: u64 = 60_000;
/// 무선 종류마다 교실 끝에서 필요한 최소 표본 수.
// TODO(field-unverified)
pub const MIN_SAMPLES: usize = 3;
/// 교실 끝 값은 고를 때 값보다 적어도 이만큼 약해야 한다(dB, 상대값). 휴대폰을 1–2m만 옮기고 끝낸 측정을 막는다 — 그런
/// 경계면 노트북 몇 걸음 밖의 새 기기를 모두 교실 밖으로 보게 된다.
// TODO(field-unverified): 교실 끝까지의 실제 감쇠 분포([S7] 벽 하나 1m에 약 6dB).
pub const MIN_SEPARATION_DB: i16 = 10;

/// 보정 단계.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// 목록에서 휴대폰(블루투스)과 핫스팟(와이파이)을 고르는 중.
    #[default]
    Listing,
    /// 골랐다 — 휴대폰을 교실 끝으로 옮길 차례.
    ReadyForFar,
    /// 휴대폰을 교실 끝에 두고 재는 중.
    Far,
    /// 두 경계를 정했다.
    Done,
    /// 실패했다. 다시 고른다.
    Failed,
}

/// 실패 이유.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CalibrationProblem {
    /// 블루투스 기기와 핫스팟을 둘 다 고르지 않았다.
    NotSelected,
    /// 고른 기기가 지금 목록에 없다(꺼졌거나 멀어졌다).
    Gone,
    /// 교실 끝에서 고른 기기를 충분히 보지 못했다.
    NotSeenAtEdge,
    /// 교실 끝 신호가 고를 때보다 충분히 약하지 않다.
    NotFarther,
    /// 아직 고르지 않았다.
    TooEarly,
}

/// 목록의 기기 한 줄. 식별자는 넣지 않는다 — `id`는 이 보정 안에서만 쓰는 번호다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidate {
    /// 고를 때 쓰는 번호(예: "b3", "w1").
    pub id: String,
    /// 무선 종류.
    pub radio: Radio,
    /// 이름(광고 이름·와이파이 이름, 없으면 제조사·"이름 없는 …").
    pub label: String,
    /// 덧붙임(제조사, 대역).
    pub detail: String,
    /// 최근 세기(dBm, 아래쪽 중앙값).
    pub rssi: i16,
}

/// 화면에 보낼 진행 상황.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct CalibrationProgress {
    /// 단계.
    pub stage: Stage,
    /// 교실 끝 단계에서 지난 초.
    pub elapsed_secs: u64,
    /// 교실 끝 단계의 최소 길이(초).
    pub total_secs: u64,
    /// 지금 잡히는 블루투스 기기(센 순). 고르는 단계에서만 채운다.
    pub bluetooth: Vec<Candidate>,
    /// 지금 잡히는 와이파이(센 순). 고르는 단계에서만 채운다.
    pub wifi: Vec<Candidate>,
    /// 고른 블루투스 기기.
    pub selected_bluetooth: Option<Candidate>,
    /// 고른 핫스팟.
    pub selected_wifi: Option<Candidate>,
    /// 교실 끝에서 모은 블루투스 표본 수.
    pub bluetooth_samples: usize,
    /// 교실 끝에서 모은 와이파이 표본 수.
    pub wifi_samples: usize,
    /// 필요한 표본 수.
    pub needed_samples: usize,
    /// 정한 블루투스 교실 경계(dBm).
    pub bluetooth_dbm: Option<i16>,
    /// 정한 와이파이 교실 경계(dBm).
    pub wifi_dbm: Option<i16>,
    /// 쉬운 말 안내.
    pub message: String,
    /// 실패 이유.
    pub problem: Option<CalibrationProblem>,
    /// 실패한 무선 종류.
    pub problem_radio: Option<Radio>,
}

#[derive(Debug, Clone)]
struct Seen {
    number: u32,
    radio: Radio,
    label: String,
    detail: String,
    recent: VecDeque<(Timestamp, i16)>,
}

impl Seen {
    fn id(&self) -> String {
        match self.radio {
            Radio::Bluetooth => format!("b{}", self.number),
            Radio::Wifi => format!("w{}", self.number),
        }
    }

    fn strength(&self) -> Option<i16> {
        let values: Vec<i16> = self.recent.iter().map(|(_, v)| *v).collect();
        lower_median(&values)
    }

    fn last_seen(&self) -> Option<Timestamp> {
        self.recent.back().map(|(t, _)| *t)
    }

    fn candidate(&self) -> Option<Candidate> {
        Some(Candidate {
            id: self.id(),
            radio: self.radio,
            label: self.label.clone(),
            detail: self.detail.clone(),
            rssi: self.strength()?,
        })
    }
}

#[derive(Debug, Clone)]
struct Selection {
    key: DeviceKey,
    reference: i16,
    edge: Vec<i16>,
}

/// 보정 상태.
#[derive(Debug, Clone, Default)]
pub struct Calibrator {
    stage: Stage,
    started: Timestamp,
    seen: BTreeMap<DeviceKey, Seen>,
    next_number: u32,
    bluetooth: Option<Selection>,
    wifi: Option<Selection>,
    boundaries: Option<(i16, i16)>,
    problem: Option<(CalibrationProblem, Option<Radio>)>,
}

/// 이름 없는 블루투스 기기 표시. 이름이 있는 광고가 오면 이것을 덮어쓴다.
const NO_NAME_PREFIX: &str = "이름 없는";

fn ble_label(name: Option<&String>, company: Option<u16>) -> (String, String) {
    let company_name = company.and_then(text::company_name);
    match (name, company_name) {
        (Some(name), Some(company)) => (name.clone(), company.to_owned()),
        (Some(name), None) => (name.clone(), String::new()),
        (None, Some(company)) => (format!("{NO_NAME_PREFIX} {company} 기기"), String::new()),
        (None, None) => (format!("{NO_NAME_PREFIX} 블루투스 기기"), String::new()),
    }
}

fn band_label(band: Band) -> &'static str {
    match band {
        Band::B2_4 => "2.4GHz",
        Band::B5 => "5GHz",
        Band::B6 => "6GHz",
    }
}

impl Calibrator {
    /// 처음(고르는 단계)으로 돌아간다. 지금까지 본 기기 목록은 남기고 고른 것과 결과는 버린다.
    pub fn restart(&mut self) {
        self.stage = Stage::Listing;
        self.bluetooth = None;
        self.wifi = None;
        self.boundaries = None;
        self.problem = None;
    }

    /// 관측을 넣는다. BLE 광고와 와이파이만 쓴다. 고르는 목록을 갱신하고, 교실 끝 단계면 고른 기기의 표본을 모은다.
    pub fn observe(&mut self, sensor: &SensorId, obs: &Observation) {
        let (radio, rssi, at, label, detail) = match obs {
            Observation::Ble(ble) => {
                let (label, detail) = ble_label(
                    ble.name.as_ref().map(|n| &n.0),
                    ble.manufacturer.first().map(|(c, _)| *c),
                );
                (Radio::Bluetooth, ble.rssi, ble.at, label, detail)
            }
            Observation::Wifi(wifi) => {
                let label = match &wifi.ssid {
                    Ssid::Named(name) => name.clone(),
                    Ssid::Hidden => "이름을 숨긴 와이파이".to_owned(),
                };
                (
                    Radio::Wifi,
                    wifi.rssi,
                    wifi.at,
                    label,
                    band_label(wifi.band).to_owned(),
                )
            }
            _ => return,
        };
        let key = DeviceKey::raw(sensor, obs);
        let next = self.next_number + 1;
        let entry = self.seen.entry(key.clone()).or_insert_with(|| Seen {
            number: next,
            radio,
            label: String::new(),
            detail: String::new(),
            recent: VecDeque::new(),
        });
        if entry.number == next {
            self.next_number = next;
        }
        // 이름은 일부 광고에만 실려 온다 — 이름 없는 광고가 이미 알게 된 이름을 지우지 않게 한다.
        if entry.label.is_empty() || !label.starts_with(NO_NAME_PREFIX) {
            entry.label = label;
            entry.detail = detail;
        }
        entry.recent.push_back((at, rssi));
        while entry.recent.len() > RECENT_SAMPLES {
            entry.recent.pop_front();
        }
        self.prune(at);
        if self.stage == Stage::Far && at >= self.started {
            for selection in [&mut self.bluetooth, &mut self.wifi].into_iter().flatten() {
                if selection.key == key {
                    selection.edge.push(rssi);
                }
            }
        }
    }

    /// 목록 창보다 오래 안 보인 기기를 지운다(고른 기기는 남긴다). 붐비는 곳에서 보정 화면을 오래 열어 두어도, 주소를 바꾸는
    /// 기기 때문에 표가 끝없이 커지지 않게 한다.
    fn prune(&mut self, at: Timestamp) {
        let keep: Vec<DeviceKey> = [&self.bluetooth, &self.wifi]
            .into_iter()
            .flatten()
            .map(|s| s.key.clone())
            .collect();
        self.seen.retain(|key, seen| {
            keep.contains(key)
                || seen
                    .last_seen()
                    .is_some_and(|t| at.since(t) <= LIST_WINDOW_MS)
        });
    }

    fn find(
        &self,
        id: &str,
        radio: Radio,
        at: Timestamp,
    ) -> Result<(DeviceKey, i16), CalibrationProblem> {
        let (key, seen) = self
            .seen
            .iter()
            .find(|(_, s)| s.radio == radio && s.id() == id)
            .ok_or(CalibrationProblem::NotSelected)?;
        let recent = seen
            .last_seen()
            .is_some_and(|t| at.since(t) <= LIST_WINDOW_MS);
        match (recent, seen.strength()) {
            (true, Some(strength)) => Ok((key.clone(), strength)),
            _ => Err(CalibrationProblem::Gone),
        }
    }

    /// 목록에서 휴대폰(블루투스)과 핫스팟(와이파이)을 고른다. 고를 때의 세기를 교실 끝과 비교할 기준으로 둔다.
    pub fn select(
        &mut self,
        bluetooth: &str,
        wifi: &str,
        at: Timestamp,
    ) -> Result<(), CalibrationProblem> {
        if self.stage != Stage::Listing && self.stage != Stage::Failed {
            return Err(CalibrationProblem::TooEarly);
        }
        let picked = self
            .find(bluetooth, Radio::Bluetooth, at)
            .map_err(|p| (p, Radio::Bluetooth))
            .and_then(|b| {
                self.find(wifi, Radio::Wifi, at)
                    .map(|w| (b, w))
                    .map_err(|p| (p, Radio::Wifi))
            });
        match picked {
            Ok(((ble_key, ble_ref), (wifi_key, wifi_ref))) => {
                self.restart();
                self.bluetooth = Some(Selection {
                    key: ble_key,
                    reference: ble_ref,
                    edge: Vec::new(),
                });
                self.wifi = Some(Selection {
                    key: wifi_key,
                    reference: wifi_ref,
                    edge: Vec::new(),
                });
                self.stage = Stage::ReadyForFar;
                Ok(())
            }
            Err((problem, radio)) => {
                self.problem = Some((problem, Some(radio)));
                self.stage = Stage::Failed;
                Err(problem)
            }
        }
    }

    /// 교실 끝 단계를 시작한다. 고른 뒤에만 된다.
    pub fn start_far(&mut self, at: Timestamp) -> Result<(), CalibrationProblem> {
        if self.stage != Stage::ReadyForFar {
            return Err(CalibrationProblem::TooEarly);
        }
        self.stage = Stage::Far;
        self.started = at;
        for selection in [&mut self.bluetooth, &mut self.wifi].into_iter().flatten() {
            selection.edge.clear();
        }
        Ok(())
    }

    fn samples(selection: &Option<Selection>) -> usize {
        selection.as_ref().map_or(0, |s| s.edge.len())
    }

    /// 시각을 옮긴다. 교실 끝 단계 시간이 차고 두 무선 모두 표본이 충분하면 결과를 정하고, 너무 오래 걸리면 실패한다.
    pub fn tick(&mut self, at: Timestamp) {
        if self.stage != Stage::Far {
            return;
        }
        let elapsed = at.since(self.started);
        let ble = Self::samples(&self.bluetooth);
        let wifi = Self::samples(&self.wifi);
        if elapsed >= FAR_MS && ble >= MIN_SAMPLES && wifi >= MIN_SAMPLES {
            self.finish();
        } else if elapsed >= FAR_TIMEOUT_MS {
            let radio = if ble < MIN_SAMPLES {
                Radio::Bluetooth
            } else {
                Radio::Wifi
            };
            self.fail(CalibrationProblem::NotSeenAtEdge, radio);
        }
    }

    fn fail(&mut self, problem: CalibrationProblem, radio: Radio) {
        self.stage = Stage::Failed;
        self.problem = Some((problem, Some(radio)));
    }

    /// 무선 종류 하나의 경계: 교실 끝 아래쪽 중앙값. 고를 때보다 `MIN_SEPARATION_DB` 이상 약해야 한다.
    fn edge(selection: &Option<Selection>) -> Result<i16, CalibrationProblem> {
        let selection = selection.as_ref().ok_or(CalibrationProblem::NotSelected)?;
        let edge = lower_median(&selection.edge).ok_or(CalibrationProblem::NotSeenAtEdge)?;
        if i32::from(selection.reference) - i32::from(edge) < i32::from(MIN_SEPARATION_DB) {
            return Err(CalibrationProblem::NotFarther);
        }
        Ok(edge)
    }

    fn finish(&mut self) {
        match (Self::edge(&self.bluetooth), Self::edge(&self.wifi)) {
            (Ok(ble), Ok(wifi)) => {
                self.boundaries = Some((ble, wifi));
                self.stage = Stage::Done;
            }
            (Err(problem), _) => self.fail(problem, Radio::Bluetooth),
            (_, Err(problem)) => self.fail(problem, Radio::Wifi),
        }
    }

    fn list(&self, radio: Radio, at: Timestamp) -> Vec<Candidate> {
        let mut list: Vec<Candidate> = self
            .seen
            .values()
            .filter(|s| {
                s.radio == radio && s.last_seen().is_some_and(|t| at.since(t) <= LIST_WINDOW_MS)
            })
            .filter_map(Seen::candidate)
            .collect();
        // 센 순, 같으면 번호 순(결정성).
        list.sort_by(|a, b| b.rssi.cmp(&a.rssi).then_with(|| a.id.cmp(&b.id)));
        list.truncate(LIST_LIMIT);
        list
    }

    fn selected(&self, selection: &Option<Selection>) -> Option<Candidate> {
        let key = &selection.as_ref()?.key;
        self.seen.get(key).and_then(Seen::candidate)
    }

    /// 지금 진행 상황.
    pub fn progress(&self, at: Timestamp) -> CalibrationProgress {
        let listing = matches!(self.stage, Stage::Listing | Stage::Failed);
        let (problem, problem_radio) = self.problem.map_or((None, None), |(p, r)| (Some(p), r));
        CalibrationProgress {
            stage: self.stage,
            elapsed_secs: if self.stage == Stage::Far {
                at.since(self.started).min(FAR_MS) / 1000
            } else {
                0
            },
            total_secs: FAR_MS / 1000,
            bluetooth: if listing {
                self.list(Radio::Bluetooth, at)
            } else {
                Vec::new()
            },
            wifi: if listing {
                self.list(Radio::Wifi, at)
            } else {
                Vec::new()
            },
            selected_bluetooth: self.selected(&self.bluetooth),
            selected_wifi: self.selected(&self.wifi),
            bluetooth_samples: Self::samples(&self.bluetooth),
            wifi_samples: Self::samples(&self.wifi),
            needed_samples: MIN_SAMPLES,
            bluetooth_dbm: self.boundaries.map(|(b, _)| b),
            wifi_dbm: self.boundaries.map(|(_, w)| w),
            message: message(self.stage, problem, problem_radio),
            problem,
            problem_radio,
        }
    }
}

fn radio_name(radio: Option<Radio>) -> &'static str {
    match radio {
        Some(Radio::Wifi) => "핫스팟(와이파이)",
        _ => "휴대폰(블루투스)",
    }
}

fn message(stage: Stage, problem: Option<CalibrationProblem>, radio: Option<Radio>) -> String {
    let which = radio_name(radio);
    match (stage, problem) {
        (_, Some(CalibrationProblem::NotSelected)) => {
            "목록에서 내 휴대폰(블루투스)과 내 핫스팟(와이파이)을 하나씩 골라 주세요.".to_owned()
        }
        (_, Some(CalibrationProblem::Gone)) => format!(
            "고른 {which} 신호가 지금 잡히지 않습니다. 휴대폰의 블루투스와 개인 핫스팟이 켜져 있는지 확인하고 다시 골라 주세요."
        ),
        (_, Some(CalibrationProblem::NotSeenAtEdge)) => format!(
            "교실 끝에서 {which} 신호를 충분히 받지 못했습니다. 휴대폰 화면을 켜 두고 블루투스와 핫스팟이 켜져 있는지 확인한 뒤 다시 재 주세요."
        ),
        (_, Some(CalibrationProblem::NotFarther)) => format!(
            "교실 끝의 {which} 신호가 고를 때보다 충분히 약하지 않습니다. 고를 때는 휴대폰을 노트북 가까이 두고, 잴 때는 노트북에서 가장 먼 자리에 두었는지 확인해 주세요."
        ),
        (_, Some(CalibrationProblem::TooEarly)) => "먼저 목록에서 내 휴대폰과 핫스팟을 골라 주세요.".to_owned(),
        (Stage::Listing, None) => "휴대폰의 블루투스와 개인 핫스팟을 켜고 노트북 가까이 두세요. 아래 목록에서 내 휴대폰(블루투스)과 내 핫스팟(와이파이)을 하나씩 고르세요. 가까이 있는 기기가 위에 옵니다.".to_owned(),
        (Stage::ReadyForFar, None) => "이제 휴대폰을 교실에서 이 노트북과 가장 먼 자리(교실 끝)의 책상 위에 두고, 휴대폰에서 떨어진 뒤 '교실 끝에 두었어요'를 눌러 주세요. 블루투스와 핫스팟은 켜 둔 채로 둡니다.".to_owned(),
        (Stage::Far, None) => "교실 끝에 둔 휴대폰의 블루투스와 핫스팟 신호를 재는 중입니다. 와이파이는 몇 초마다 한 번씩 잡혀 1분 가까이 걸릴 수 있습니다. 휴대폰 가까이에 서 있지 마세요.".to_owned(),
        (Stage::Done, None) => "교실 보정을 마쳤습니다. 저장하면 이 값을 기준으로 새 기기가 교실 안일 가능성을 살핍니다. 휴대폰 한 대로 한 자리에서 잰 값이라 참고용입니다.".to_owned(),
        (Stage::Failed, None) => "보정하지 못했습니다. 다시 골라 주세요.".to_owned(),
    }
}
