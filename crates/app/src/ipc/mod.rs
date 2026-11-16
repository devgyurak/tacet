//! 화면(Electron, `desktop/`)과의 통신.
//!
//! Electron이 이 바이너리를 자식 프로세스로 띄우고 표준 입출력으로 JSON 한 줄씩 주고받는다. 네트워크 포트를 열지
//! 않으므로 같은 망의 다른 기기가 끼어들 수 없다. 화면은 판단하지 않는다 — 받은 [`UiModel`]을 그릴 뿐이고, 보내는 것은
//! 사용자의 요청(보고서 저장, 설정 열기)뿐이다(AGENTS.md §2 규칙 11).

use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};

use crate::platform;
use crate::runtime::{Handles, Shared, UiModel, save_report};

#[cfg(test)]
mod tests;

/// 화면으로 보내는 메시지.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Outgoing {
    /// 화면 모델 전체. 바뀔 때마다 보낸다.
    Model {
        /// 모델(크기가 커서 상자에 담는다).
        model: Box<UiModel>,
    },
    /// 교실 보정 진행 상황(`tacet calibrate`).
    Calibration {
        /// 진행 상황(목록이 커서 상자에 담는다).
        progress: Box<tacet_core::calibrate::CalibrationProgress>,
        /// 블루투스 수집이 꺼졌으면 그 이유(쉬운 말).
        bluetooth_problem: Option<String>,
        /// 와이파이 수집이 꺼졌으면 그 이유(쉬운 말). 보정은 블루투스와 와이파이를 둘 다 잰다.
        wifi_problem: Option<String>,
    },
    /// 시작하지 못했다(설정 오류 등). 화면이 그대로 보여 준다.
    Error {
        /// 쉬운 말 설명.
        message: String,
    },
}

/// 화면에서 오는 요청.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    /// 세션 보고서 저장(식별자는 해시만).
    SaveReport,
    /// 위치 권한 설정 화면 열기.
    OpenLocationSettings,
    /// 교실 보정: 목록에서 고른 휴대폰(블루투스)과 핫스팟(와이파이). 값은 보정 목록의 번호다(식별자가 아니다).
    CalibrateSelect {
        /// 블루투스 기기 번호(예: "b3").
        bluetooth: String,
        /// 와이파이 번호(예: "w1").
        wifi: String,
    },
    /// 교실 보정: 휴대폰을 교실 끝에 두고 재기 시작.
    CalibrateFar,
    /// 교실 보정: 처음(고르는 단계)으로.
    CalibrateRestart,
    /// 끝내기.
    Shutdown,
}

/// 메시지를 한 줄 JSON으로 만든다.
pub fn encode(message: &Outgoing) -> String {
    // 모델은 문자열·정수·열거형뿐이라 직렬화가 실패하지 않는다. 그래도 실패하면 화면에 오류로 알린다.
    serde_json::to_string(message).unwrap_or_else(|e| {
        format!(
            r#"{{"type":"error","message":"화면 모델을 만들지 못했다: {}"}}"#,
            e.to_string().replace('"', "'")
        )
    })
}

/// 요청 한 줄을 읽는다. 모르는 줄은 버린다(화면도 남이 보낸 입력처럼 다룬다).
pub fn parse_command(line: &str) -> Option<Command> {
    serde_json::from_str(line.trim()).ok()
}

/// 락을 잡는다. 다른 스레드가 쥔 채 패닉해 오염됐어도 안의 값을 그대로 쓴다 — 화면이 조용히 멈추는 것보다 마지막
/// 상태라도 계속 보내는 편이 낫고, 수집기 꺼짐은 모델 안의 상태로 따로 보고된다.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 요청을 처리하고 화면에 알릴 문구를 돌려준다.
pub fn handle(command: Command, handles: &Handles, state_dir: &Path) -> Option<String> {
    match command {
        Command::OpenLocationSettings => {
            platform::open_location_settings();
            None
        }
        Command::SaveReport => {
            let hub = lock(handles.hub.as_ref()?);
            Some(match save_report(&hub, &state_dir.join("reports")) {
                Ok(path) => format!("보고서를 저장했습니다: {}", path.display()),
                Err(e) => format!("보고서를 저장하지 못했습니다: {e}"),
            })
        }
        // 보정 명령은 보정 모드(`calibrate`)에서만 뜻이 있다.
        Command::CalibrateSelect { .. }
        | Command::CalibrateFar
        | Command::CalibrateRestart
        | Command::Shutdown => None,
    }
}

/// 모델이 지난번 보낸 것과 다르면 한 줄로 보낸다. 보냈으면 `true`.
pub fn pump(
    model: &Shared,
    last: &mut Option<UiModel>,
    out: &mut impl Write,
) -> std::io::Result<bool> {
    let current = lock(model).clone();
    if last.as_ref() == Some(&current) {
        return Ok(false);
    }
    writeln!(
        out,
        "{}",
        encode(&Outgoing::Model {
            model: Box::new(current.clone())
        })
    )?;
    out.flush()?;
    *last = Some(current);
    Ok(true)
}

/// 요청 줄을 끝(입력 닫힘 또는 [`Command::Shutdown`])까지 처리한다. 화면 알림은 모델의 `notice`에 넣는다.
pub fn serve_commands(input: impl BufRead, handles: &Handles, state_dir: &Path) {
    for line in input.lines() {
        // UTF-8이 아닌 줄은 버리고 계속한다. 입력이 닫히거나 읽기가 실패할 때만 끝낸다.
        let line = match line {
            Ok(line) => line,
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => continue,
            Err(_) => return,
        };
        let Some(command) = parse_command(&line) else {
            continue;
        };
        if command == Command::Shutdown {
            return;
        }
        if let Some(notice) = handle(command, handles, state_dir) {
            lock(&handles.model).notice = Some(notice);
        }
    }
}
