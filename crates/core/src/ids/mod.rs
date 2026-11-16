//! 무선 식별자와 센서·교실 식별자.
//!
//! 식별자는 세션 메모리에만 있고 세션이 끝나면 폐기한다(AGENTS.md §2 규칙 12). 테스트와 문서는
//! 합성 값만 쓴다(AGENTS.md §3.3).

use std::fmt;

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

/// 48비트 MAC 주소(BLE 주소, Wi-Fi BSSID).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Mac(pub [u8; 6]);

impl Mac {
    /// 첫 옥텟의 locally administered 비트(0x02). 랜덤 MAC과 폰 핫스팟의 특성이다(IEEE 802, 설계 §4.2 R4).
    pub const fn is_locally_administered(self) -> bool {
        self.0[0] & 0x02 != 0
    }

    /// 첫 옥텟의 multicast 비트(0x01).
    pub const fn is_multicast(self) -> bool {
        self.0[0] & 0x01 != 0
    }

    /// `00:00:5E:00:53:01` 꼴(대소문자 무관, `-` 구분자도 허용) 문자열을 읽는다. 형식이 틀리면 `None`.
    pub fn parse(text: &str) -> Option<Self> {
        let parts: Vec<&str> = text.trim().split([':', '-']).collect();
        if parts.len() != 6 {
            return None;
        }
        let mut out = [0u8; 6];
        for (slot, part) in out.iter_mut().zip(parts) {
            if part.len() != 2 {
                return None;
            }
            *slot = u8::from_str_radix(part, 16).ok()?;
        }
        Some(Self(out))
    }
}

impl fmt::Display for Mac {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [a, b, c, d, e, g] = self.0;
        write!(f, "{a:02X}:{b:02X}:{c:02X}:{d:02X}:{e:02X}:{g:02X}")
    }
}

/// BLE 서비스 UUID(128비트). 16비트 할당 번호는 Bluetooth 기본 UUID로 늘려 담는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ServiceUuid(pub u128);

impl ServiceUuid {
    /// Bluetooth 기본 UUID `0000xxxx-0000-1000-8000-00805F9B34FB`.
    const BASE: u128 = 0x0000_0000_0000_1000_8000_0080_5F9B_34FB;

    /// 16비트 할당 번호로 만든다.
    pub const fn from_u16(short: u16) -> Self {
        Self(Self::BASE | ((short as u128) << 96))
    }

    /// 16비트 할당 번호라면 그 값.
    pub const fn as_u16(self) -> Option<u16> {
        let short = ((self.0 >> 96) & 0xFFFF) as u16;
        if self.0 == Self::from_u16(short).0 {
            Some(short)
        } else {
            None
        }
    }
}

/// BLE 광고자 식별자. 플랫폼이 주는 것이 다르다(설계 §7.2, AGENTS.md §2 규칙 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BleId {
    /// Windows: 실제(랜덤일 수 있는) 주소.
    Addr(Mac),
    /// macOS: CoreBluetooth가 호스트마다 다르게 주는 UUID. 다른 센서의 관측과 같은 값으로 비교하지 않는다.
    HostLocal(u128),
}

/// 센서 식별자(예: `3-2-뒤`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SensorId(pub String);

impl SensorId {
    /// 문자열로 만든다.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for SensorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 교실 식별자(예: `3-2`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RoomId(pub String);

impl RoomId {
    /// 문자열로 만든다.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for RoomId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 교실 안 센서 위치(설계 §4.4 앞뒤 2개 구성).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SensorPos {
    /// 교실 앞(교탁 쪽).
    Front,
    /// 교실 뒤.
    Back,
}

/// 센서가 도는 운영체제.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Platform {
    /// Windows 11.
    Windows,
    /// macOS.
    MacOs,
    /// 리눅스(개발·검증용).
    Linux,
}
