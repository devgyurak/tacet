//! OS별 부분: 절전 방지, 설정 화면 열기, 상태 저장 위치, 메인 스레드 대기(AGENTS.md §2 규칙 6 — app에만 둔다).

use std::path::PathBuf;
use std::process::{Child, Command};

#[cfg(test)]
mod tests;

/// 앱 상태(설정, 서버 인증서, 보고서)를 두는 곳. 저장소 밖이다.
pub fn state_dir() -> PathBuf {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library").join("Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
            })
    };
    base.unwrap_or_else(std::env::temp_dir).join("tacet")
}

/// 절전 방지. 버리면 풀린다.
#[derive(Debug)]
pub struct SleepGuard {
    child: Option<Child>,
}

impl Drop for SleepGuard {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
        }
    }
}

/// 시험 동안 잠자기·화면 꺼짐을 막는다(설계 §7.4). 실패해도 앱은 계속 돈다.
pub fn prevent_sleep() -> SleepGuard {
    #[cfg(windows)]
    windows_keep_awake();
    let child = if cfg!(target_os = "macos") {
        // caffeinate는 macOS 기본 도구다. 이 프로세스가 끝나면 함께 끝난다.
        Command::new("/usr/bin/caffeinate")
            .args(["-dimsu", "-w", &std::process::id().to_string()])
            .spawn()
            .ok()
    } else {
        None
    };
    SleepGuard { child }
}

#[cfg(windows)]
#[expect(unsafe_code, reason = "Win32 전원 API")]
fn windows_keep_awake() {
    use windows::Win32::System::Power::{
        ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
    };
    // SAFETY: 인자는 플래그 값뿐이고 포인터를 넘기지 않는다. 호출한 스레드가 사는 동안 효과가 이어진다(UI 스레드).
    unsafe {
        SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED);
    }
}

/// 위치 권한 설정 화면을 연다(권한 온보딩, 설계 §7.1–§7.2).
pub fn open_location_settings() {
    let _ = if cfg!(windows) {
        Command::new("cmd")
            .args(["/C", "start", "ms-settings:privacy-location"])
            .spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_LocationServices")
            .spawn()
    } else {
        Command::new("true").spawn()
    };
}

/// 메인 스레드를 영원히 붙잡는다. macOS는 메인 실행 루프를 1초씩 돌린다 — 위치 권한 요청(CoreLocation)이 메인
/// 스레드의 실행 루프에서 처리되기 때문이다. 실행 루프에 처리할 소스가 없으면 곧바로 돌아오므로 잠깐 쉬고 다시 돈다
/// (`run()` 한 번으로는 소스가 없을 때 끝나 버린다). 다른 OS는 그냥 잠든다. 프로세스는 다른 스레드가 `exit`로 끝낸다.
pub fn park_main_thread() -> ! {
    loop {
        #[cfg(target_os = "macos")]
        {
            let until = objc2_foundation::NSDate::dateWithTimeIntervalSinceNow(1.0);
            objc2_foundation::NSRunLoop::mainRunLoop().runUntilDate(&until);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}
