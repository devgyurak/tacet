//! macOS 어댑터(CoreWLAN). 위치 권한이 없으면 BSSID가 비어 온다(설계 §7.2).

use std::thread;

use objc2_core_location::CLLocationManager;
use objc2_core_wlan::CWWiFiClient;
use tacet_core::obs::{CollectorEvent, CollectorKind, CollectorStatus, Observation, OffReason};
use tacet_core::time::ClockFn;
use tokio::sync::mpsc;

use crate::SCAN_INTERVAL;
use crate::bss::{corewlan_status, from_corewlan};

/// 위치 권한을 요청한다. 앱의 메인 스레드에서 부른다. 관리자는 앱이 끝날 때까지 살아 있어야 하므로 놓아 둔다.
/// 메인 스레드가 아니면 아무것도 하지 않는다.
#[expect(unsafe_code, reason = "CoreLocation FFI")]
pub(crate) fn request_location_permission() {
    if objc2::MainThreadMarker::new().is_none() {
        return;
    }
    // SAFETY: CLLocationManager는 메인 스레드에서 만들고 불러야 하고, 바로 위에서 메인 스레드임을 확인했다.
    // 인자가 없는 메서드라 넘기는 포인터가 없다.
    let manager = unsafe { CLLocationManager::new() };
    // SAFETY: 위와 같다. 권한 요청은 비동기로 처리되고 결과는 다음 스캔의 BSSID 유무로 확인한다.
    unsafe { manager.requestWhenInUseAuthorization() };
    std::mem::forget(manager);
}

struct Network {
    bssid: Option<String>,
    ssid: Option<Vec<u8>>,
    rssi: i64,
    channel: i64,
    band: i64,
}

#[expect(unsafe_code, reason = "CoreWLAN FFI")]
fn scan_once() -> Result<Vec<Network>, OffReason> {
    // SAFETY: sharedWiFiClient와 interface는 인자 없는 접근자이고 반환값은 Retained가 수명을 관리한다.
    let client = unsafe { CWWiFiClient::sharedWiFiClient() };
    // SAFETY: 위와 같다.
    let Some(interface) = (unsafe { client.interface() }) else {
        return Err(OffReason::DeviceMissing);
    };
    // SAFETY: 이름 없는(None) 스캔은 주변 네트워크 전체를 돌려준다. 우리 코드는 프레임을 만들지 않는다.
    let networks = unsafe { interface.scanForNetworksWithName_error(None) }
        .map_err(|e| OffReason::Error(e.to_string()))?;
    let mut out = Vec::new();
    for network in networks.iter() {
        // SAFETY: 아래 접근자는 모두 인자 없는 읽기이고, 반환된 객체는 이 반복 안에서만 쓴다.
        let entry = unsafe {
            let channel = network.wlanChannel();
            Network {
                bssid: network.bssid().map(|s| s.to_string()),
                ssid: network.ssidData().map(|d| d.to_vec()),
                rssi: network.rssiValue() as i64,
                channel: channel.as_ref().map_or(0, |c| c.channelNumber() as i64),
                band: channel.as_ref().map_or(0, |c| c.channelBand().0 as i64),
            }
        };
        out.push(entry);
    }
    Ok(out)
}

fn status(tx: &mpsc::Sender<CollectorEvent>, status: CollectorStatus) -> bool {
    tx.blocking_send(CollectorEvent::Status(CollectorKind::Wifi, status))
        .is_ok()
}

pub(crate) fn scan_loop(clock: &ClockFn, tx: &mpsc::Sender<CollectorEvent>) {
    let mut last: Option<CollectorStatus> = None;
    loop {
        let now_status = match scan_once() {
            Ok(networks) => {
                let at = clock();
                let without_bssid = networks.iter().filter(|n| n.bssid.is_none()).count();
                for n in &networks {
                    let obs = from_corewlan(
                        n.bssid.as_deref(),
                        n.ssid.as_deref(),
                        n.rssi,
                        n.channel,
                        n.band,
                        at,
                    );
                    if let Some(obs) = obs
                        && tx
                            .blocking_send(CollectorEvent::Obs(Observation::Wifi(obs)))
                            .is_err()
                    {
                        return;
                    }
                }
                corewlan_status(networks.len(), without_bssid)
            }
            Err(reason) => CollectorStatus::Off(reason),
        };
        if last.as_ref() != Some(&now_status) {
            if !status(tx, now_status.clone()) {
                return;
            }
            last = Some(now_status);
        }
        if tx.is_closed() {
            return;
        }
        thread::sleep(SCAN_INTERVAL);
    }
}
