//! 다교실 위치 판정(설계 §4.7, 노트 `docs/rules/L-room-locator.md`).
//!
//! 판정은 두 단계다. 어느 센서에서든 잡히면 건물 내 탐지이고, 여러 센서의 RSSI 중앙값을 비교해 어느
//! 교실인지 추정한다. 좌석 단위로는 특정하지 않는다 — 교실과 앞뒤까지가 한계다(설계 §4.4).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::{DetectorConfig, Radio, RoomLayout};
use crate::ids::{RoomId, SensorId, SensorPos};

#[cfg(test)]
mod tests;

/// 정수 중앙값. 표본이 짝수 개면 아래쪽 가운데 값을 쓴다 — 평균을 내면 반올림이 끼어든다(AGENTS.md §2 규칙 3).
pub fn lower_median<T: Ord + Copy>(samples: &[T]) -> Option<T> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    sorted.get((sorted.len() - 1) / 2).copied()
}

/// 위치 판정 신뢰도.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Confidence {
    /// 낮음 — 인접 교실 감독관에게도 알린다.
    Low,
    /// 중간.
    Medium,
    /// 높음.
    High,
}

impl Confidence {
    /// 한 단계 낮춘다(macOS 호스트별 UUID 상관, 설계 §7.2).
    pub fn lowered(self) -> Self {
        match self {
            Confidence::High => Confidence::Medium,
            Confidence::Medium | Confidence::Low => Confidence::Low,
        }
    }
}

/// 위치 판정 결과(설계 §5.4 `Localization`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Localization {
    /// 추정 교실. `None`이면 건물 내 탐지이되 교실 미정이다.
    pub room: Option<RoomId>,
    /// 교실 안 앞뒤(그 교실에 센서가 둘 이상일 때만).
    pub position: Option<SensorPos>,
    /// 신뢰도.
    pub confidence: Confidence,
    /// 모든 센서의 경계 아래라 건물 밖일 가능성이 있다(설계 §4.3 감점 대상).
    pub likely_outside: bool,
    /// 가장 센 센서에 보정값이 없어 경계 판정을 못 했다.
    pub needs_calibration: bool,
    /// 근거: 센서별 RSSI 중앙값(센 순, 같으면 센서 ID 순).
    pub evidence: Vec<(SensorId, i16)>,
}

/// 센서별 세기 표본으로 위치를 추정한다. 배치에 없는 센서의 표본은 쓰지 않는다. 표본이 하나도 없으면 `None`.
pub fn locate(
    samples: &BTreeMap<SensorId, Vec<i16>>,
    layout: &RoomLayout,
    host_local: bool,
    cfg: &DetectorConfig,
) -> Option<Localization> {
    locate_radio(samples, layout, host_local, cfg, Radio::Bluetooth)
}

/// [`locate`]와 같되 무선 종류에 맞는 교실 경계(블루투스·와이파이)로 판정한다.
pub fn locate_radio(
    samples: &BTreeMap<SensorId, Vec<i16>>,
    layout: &RoomLayout,
    host_local: bool,
    cfg: &DetectorConfig,
    radio: Radio,
) -> Option<Localization> {
    let mut evidence: Vec<(SensorId, i16)> = samples
        .iter()
        .filter(|(id, _)| layout.sensor(id).is_some())
        .filter_map(|(id, values)| lower_median(values).map(|m| (id.clone(), m)))
        .collect();
    evidence.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let (top_sensor, top_value) = evidence.first().cloned()?;
    let (top_room, top_spec) = layout.sensor(&top_sensor)?;

    // 교실별 최댓값으로 순위를 매긴다. evidence가 이미 센 순이므로 처음 나온 교실 값이 그 교실의 최댓값이다.
    let mut room_best: Vec<(RoomId, i16)> = Vec::new();
    for (sensor, value) in &evidence {
        let Some((room, _)) = layout.sensor(sensor) else {
            continue;
        };
        if !room_best.iter().any(|(r, _)| r == &room.id) {
            room_best.push((room.id.clone(), *value));
        }
    }

    let needs_calibration = top_spec.boundary(radio).is_none();
    // 건물 밖 가능성: 모든 센서가 보정돼 있고, 모두 경계보다 인체 차폐 여유 이상 약할 때만이다. 몸에 가려진
    // 폰은 경계 아래로 10dB 넘게 떨어질 수 있어 "경계 아래"만으로는 교실 안 숨긴 폰과 구분하지 못한다(설계 §4.4).
    let likely_outside = evidence.iter().all(|(sensor, value)| {
        layout
            .sensor(sensor)
            .and_then(|(_, spec)| spec.boundary(radio))
            .is_some_and(|boundary| {
                i32::from(*value) < i32::from(boundary) - i32::from(cfg.shield_margin_db)
            })
    });

    let mut confidence = match room_best.get(1) {
        Some((_, second)) => {
            let diff = i32::from(top_value) - i32::from(*second);
            if diff >= i32::from(cfg.high_confidence_db) {
                Confidence::High
            } else if diff >= i32::from(cfg.medium_confidence_db) {
                Confidence::Medium
            } else {
                Confidence::Low
            }
        }
        // 교실이 하나뿐인 배치는 비교 대상이 없다. 여러 교실 배치에서 한 교실만 봤다면 아직 다른 교실 센서가
        // 못 받은 것일 수 있으니 낮게 보고 인접 교실에도 알린다.
        // TODO(field-unverified): 한 교실만 본 기기의 신뢰도.
        None if layout.rooms.len() <= 1 => Confidence::High,
        None => Confidence::Low,
    };
    if host_local {
        confidence = confidence.lowered();
    }
    if needs_calibration {
        confidence = Confidence::Low;
    }

    let room = (!likely_outside).then(|| top_room.id.clone());
    // 앞뒤는 그 교실의 다른 센서가 못 봤거나, 두 센서 차이가 신뢰도 "중" 기준 이상일 때만 보인다 — 1–2dB 차이로
    // "뒤쪽"이라고 하면 정밀해 보이기만 한다.
    let other_in_room = evidence
        .iter()
        .filter(|(sensor, _)| sensor != &top_sensor)
        .find(|(sensor, _)| {
            layout
                .sensor(sensor)
                .is_some_and(|(r, _)| r.id == top_room.id)
        })
        .map(|(_, v)| *v);
    let distinct = other_in_room.is_none_or(|other| {
        i32::from(top_value) - i32::from(other) >= i32::from(cfg.medium_confidence_db)
    });
    let position = room
        .as_ref()
        .filter(|_| top_room.sensors.len() > 1 && distinct)
        .map(|_| top_spec.position);
    Some(Localization {
        room,
        position,
        confidence,
        likely_outside,
        needs_calibration,
        evidence,
    })
}
