//! 세션 시각.
//!
//! core는 시계를 읽지 않는다(AGENTS.md §2 규칙 2). 관측값은 세션 시작 기준 밀리초를 달고 들어온다.
//! 단독 모드는 수집기가 단조 시계로, 센서 모드는 NTP로 맞춘 시계로 찍는다.

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

/// 세션 시작 기준 밀리초.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct Timestamp(pub u64);

impl Timestamp {
    /// 초 단위로 만든다.
    pub const fn from_secs(secs: u64) -> Self {
        Self(secs.saturating_mul(1000))
    }

    /// `self`에서 `earlier`까지 지난 밀리초. `earlier`가 더 늦으면 0.
    pub const fn since(self, earlier: Timestamp) -> u64 {
        self.0.saturating_sub(earlier.0)
    }

    /// `ms`밀리초 뒤의 시각.
    pub const fn plus_ms(self, ms: u64) -> Self {
        Self(self.0.saturating_add(ms))
    }
}

/// 수집기가 관측 시각을 찍는 시계. 앱이 만들어 넘긴다 — core는 이 함수를 부르지 않는다.
pub type ClockFn = std::sync::Arc<dyn Fn() -> Timestamp + Send + Sync>;
