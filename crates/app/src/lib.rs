//! 시험장 무선 기기 탐지기 데스크톱 앱(설계 §5.0).
//!
//! 하나의 앱이 설정에 따라 감독관 화면(기본)·단독·센서·서버로 동작한다. 판정은 언제나 core가 하고(AGENTS.md §2),
//! 이 크레이트는 수집기·통신을 배선하는 화면 없는 백엔드다. 화면은 Electron(`desktop/`)이 그리고 표준 입출력
//! JSON으로 이 프로세스와 이야기한다([`ipc`]). 화면 문구는 한국어이고 알림은 "확인 필요"다(AGENTS.md §3.4, §9).

pub mod calibrate;
pub mod config;
pub mod hub;
pub mod ipc;
pub mod platform;
pub mod runtime;
