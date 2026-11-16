//! 기준선(설계 §4.1).
//!
//! 폰 수거와 전원 차단을 확인한 뒤 N분 동안 본 기기와 대역 전력을 기준선으로 삼는다. 학교 AP, 옆
//! 교실 기기, 감독관 장비가 여기에 들어간다. 기준선이 굳은 뒤 새로 나타난 기기만 점수를 받는다.

use std::collections::{BTreeMap, BTreeSet};

use crate::device::DeviceKey;
use crate::ids::SensorId;
use crate::locate::lower_median;
use crate::obs::CellBand;

#[cfg(test)]
mod tests;

/// 기준선.
#[derive(Debug, Clone, Default)]
pub struct Baseline {
    devices: BTreeSet<DeviceKey>,
    rf_samples: BTreeMap<(SensorId, CellBand), Vec<i32>>,
}

impl Baseline {
    /// 기준선 시간 안에 본 기기를 더한다. 기준선 시간인지는 엔진이 보고서 시각으로 정한다.
    pub fn add_device(&mut self, key: DeviceKey) {
        self.devices.insert(key);
    }

    /// 기준선 수집 중의 대역 전력(0.01dBm)을 더한다. 센서마다 안테나와 위치가 달라 센서별로 따로 둔다.
    pub fn add_rf(&mut self, sensor: &SensorId, band: CellBand, power_cdbm: i32) {
        self.rf_samples
            .entry((sensor.clone(), band))
            .or_default()
            .push(power_cdbm);
    }

    /// 기준선에 있던 기기인지.
    pub fn contains(&self, key: &DeviceKey) -> bool {
        self.devices.contains(key)
    }

    /// 센서·대역의 기준 전력(기준선 동안의 중앙값, 0.01dBm). 그 관측이 없었으면 `None`.
    pub fn rf_reference(&self, sensor: &SensorId, band: CellBand) -> Option<i32> {
        self.rf_samples
            .get(&(sensor.clone(), band))
            .and_then(|samples| lower_median(samples))
    }

    /// 기준선 기기 수.
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}
