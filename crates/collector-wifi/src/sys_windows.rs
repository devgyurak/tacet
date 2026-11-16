//! Windows 어댑터(WLAN API). 24H2부터 정밀 위치 권한이 없으면 `ERROR_ACCESS_DENIED`를 돌려준다(설계 §7.1).

use std::thread;
use std::time::Duration;

use tacet_core::obs::{
    CollectorEvent, CollectorKind, CollectorStatus, Observation, OffReason, Permission, WifiObs,
};
use tacet_core::time::ClockFn;
use tokio::sync::mpsc;
use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_SUCCESS, HANDLE};
use windows::Win32::NetworkManagement::WiFi::{
    WLAN_BSS_ENTRY, WLAN_BSS_LIST, WLAN_INTERFACE_INFO, WLAN_INTERFACE_INFO_LIST, WlanCloseHandle,
    WlanEnumInterfaces, WlanFreeMemory, WlanGetNetworkBssList, WlanOpenHandle, WlanScan,
    dot11_BSS_type_any,
};
use windows::core::GUID;

use crate::SCAN_INTERVAL;
use crate::bss::from_windows;

/// `WlanScan` 뒤 결과가 갱신되기를 기다리는 시간(MS 문서: 드라이버는 4초 안에 스캔을 끝내야 한다).
const SCAN_SETTLE: Duration = Duration::from_secs(4);

fn code_to_reason(code: u32) -> OffReason {
    if code == ERROR_ACCESS_DENIED.0 {
        OffReason::PermissionDenied(Permission::PreciseLocation)
    } else {
        OffReason::Error(format!("WLAN API 오류 코드 {code}"))
    }
}

struct Client(HANDLE);

impl Drop for Client {
    #[expect(unsafe_code, reason = "WLAN API FFI")]
    fn drop(&mut self) {
        // SAFETY: self.0은 WlanOpenHandle이 성공해 돌려준 핸들이고 여기서 한 번만 닫는다.
        unsafe {
            WlanCloseHandle(self.0, None);
        }
    }
}

#[expect(unsafe_code, reason = "WLAN API FFI")]
fn open() -> Result<Client, OffReason> {
    let mut negotiated = 0u32;
    let mut handle = HANDLE::default();
    // SAFETY: 두 출력 포인터는 이 함수의 지역 변수를 가리키고 호출 동안 살아 있다. 버전 2는 Vista 이후 API다.
    let code = unsafe { WlanOpenHandle(2, None, &mut negotiated, &mut handle) };
    if code != ERROR_SUCCESS.0 {
        return Err(code_to_reason(code));
    }
    Ok(Client(handle))
}

#[expect(unsafe_code, reason = "WLAN API FFI")]
fn interfaces(client: &Client) -> Result<Vec<GUID>, OffReason> {
    let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
    // SAFETY: 핸들은 열려 있고, 출력 포인터는 지역 변수다. 성공하면 API가 할당한 목록을 WlanFreeMemory로 푼다.
    let code = unsafe { WlanEnumInterfaces(client.0, None, &mut list) };
    if code != ERROR_SUCCESS.0 || list.is_null() {
        return Err(code_to_reason(code));
    }
    // SAFETY: list는 성공한 호출이 돌려준 유효한 목록이다. InterfaceInfo는 선언상 길이 1이지만 실제로는
    // dwNumberOfItems개의 연속 항목을 가진 가변 길이 배열이다. 그래서 길이 1 배열의 참조(as_ptr)가 아니라 목록
    // 할당 전체에서 파생한 원시 포인터(&raw const)로 읽는다. 읽은 GUID는 복사하고 목록을 푼다.
    let guids = unsafe {
        let count = (*list).dwNumberOfItems as usize;
        let first = (&raw const (*list).InterfaceInfo).cast::<WLAN_INTERFACE_INFO>();
        let guids = (0..count).map(|i| (*first.add(i)).InterfaceGuid).collect();
        WlanFreeMemory(list.cast());
        guids
    };
    Ok(guids)
}

#[expect(unsafe_code, reason = "WLAN API FFI")]
fn bss_list(client: &Client, guid: &GUID, clock: &ClockFn) -> Result<Vec<WifiObs>, OffReason> {
    let mut list: *mut WLAN_BSS_LIST = std::ptr::null_mut();
    // SAFETY: 핸들은 열려 있고 guid는 열거한 인터페이스다. 출력 포인터는 지역 변수다.
    let code = unsafe {
        WlanGetNetworkBssList(
            client.0,
            guid,
            None,
            dot11_BSS_type_any,
            false,
            None,
            &mut list,
        )
    };
    if code != ERROR_SUCCESS.0 || list.is_null() {
        return Err(code_to_reason(code));
    }
    let at = clock();
    // SAFETY: list는 성공한 호출이 돌려준 유효한 목록이고 wlanBssEntries는 dwNumberOfItems개의 연속 항목인 가변
    // 길이 배열이다. 목록 할당 전체에서 파생한 원시 포인터로 읽고, 필요한 필드만 값으로 복사한 뒤 목록을 푼다.
    let out = unsafe {
        let count = (*list).dwNumberOfItems as usize;
        let first = (&raw const (*list).wlanBssEntries).cast::<WLAN_BSS_ENTRY>();
        let out = (0..count)
            .filter_map(|i| {
                let e = &*first.add(i);
                from_windows(
                    e.dot11Bssid,
                    &e.dot11Ssid.ucSSID,
                    e.dot11Ssid.uSSIDLength,
                    e.lRssi,
                    e.ulChCenterFrequency,
                    at,
                )
            })
            .collect();
        WlanFreeMemory(list.cast());
        out
    };
    Ok(out)
}

#[expect(unsafe_code, reason = "WLAN API FFI")]
fn scan(client: &Client, guid: &GUID) -> Result<(), OffReason> {
    // SAFETY: 핸들은 열려 있고 guid는 열거한 인터페이스다. SSID·IE를 넘기지 않는다(우리가 프레임 내용을 정하지 않는다).
    let code = unsafe { WlanScan(client.0, guid, None, None, None) };
    if code == ERROR_SUCCESS.0 {
        Ok(())
    } else {
        Err(code_to_reason(code))
    }
}

/// 한 번 스캔한다. 인터페이스 하나의 실패는 그 인터페이스만 건너뛴다 — 모두 실패했을 때만 오류다.
fn cycle(
    client: &Client,
    clock: &ClockFn,
    tx: &mpsc::Sender<CollectorEvent>,
) -> Result<bool, OffReason> {
    let guids = interfaces(client)?;
    if guids.is_empty() {
        return Err(OffReason::DeviceMissing);
    }
    let scanned: Vec<Result<(), OffReason>> = guids.iter().map(|guid| scan(client, guid)).collect();
    thread::sleep(SCAN_SETTLE);
    let mut first_error = None;
    let mut any_ok = false;
    for (guid, scan_result) in guids.iter().zip(scanned) {
        match scan_result.and_then(|()| bss_list(client, guid, clock)) {
            Ok(list) => {
                any_ok = true;
                for obs in list {
                    if tx
                        .blocking_send(CollectorEvent::Obs(Observation::Wifi(obs)))
                        .is_err()
                    {
                        return Ok(false);
                    }
                }
            }
            Err(reason) => {
                first_error.get_or_insert(reason);
            }
        }
    }
    match first_error {
        Some(reason) if !any_ok => Err(reason),
        _ => Ok(true),
    }
}

fn send_status(
    tx: &mpsc::Sender<CollectorEvent>,
    last: &mut Option<CollectorStatus>,
    now: CollectorStatus,
) -> bool {
    if last.as_ref() == Some(&now) {
        return true;
    }
    let sent = tx
        .blocking_send(CollectorEvent::Status(CollectorKind::Wifi, now.clone()))
        .is_ok();
    *last = Some(now);
    sent
}

pub(crate) fn scan_loop(clock: &ClockFn, tx: &mpsc::Sender<CollectorEvent>) {
    let mut last: Option<CollectorStatus> = None;
    // 핸들을 못 열면(서비스 중지, 권한) 꺼짐을 알리고 다시 시도한다 — 사용자가 고치면 이어서 수집한다.
    let client = loop {
        match open() {
            Ok(client) => break client,
            Err(reason) => {
                if !send_status(tx, &mut last, CollectorStatus::Off(reason)) {
                    return;
                }
                thread::sleep(SCAN_INTERVAL);
            }
        }
    };
    loop {
        let (keep_going, now_status) = match cycle(&client, clock, tx) {
            Ok(keep) => (keep, CollectorStatus::Running),
            Err(reason) => (true, CollectorStatus::Off(reason)),
        };
        if !keep_going || !send_status(tx, &mut last, now_status) {
            return;
        }
        thread::sleep(SCAN_INTERVAL.saturating_sub(SCAN_SETTLE));
    }
}
