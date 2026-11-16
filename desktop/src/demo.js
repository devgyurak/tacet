// 장비 없이 화면을 보는 시연용 모델(`--demo`). 백엔드가 보내는 모델과 모양이 같다. 식별자는 문서용 합성 값만
// 쓴다(AGENTS.md §3.3). 시연·화면 확인·스크린샷에만 쓰고 판정에는 쓰지 않는다.

'use strict';

function alert(level, title, place, extra) {
  return {
    level,
    rooms: ['1-1반'],
    everyone: false,
    audience: '1-1반 교실',
    explain: {
      title,
      level: { High: '의심 높음', Medium: '의심 보통', Low: '의심 낮음' }[level],
      place,
      certainty: '위치 확실도 높음 — 이 교실 노트북의 신호가 다른 교실보다 확실히 셉니다.',
      reasons: extra.reasons,
      caveats: extra.caveats || [],
      guide: `${place}을 중심으로 둘러보고, 켜져 있는 전자기기가 있는지 확인해 주세요.`,
      technical: extra.technical,
    },
    text: `[확인 필요] ${extra.technical[0]}`,
  };
}

/** 시연 모델. `phase`로 준비 중·감시 중을 고른다. */
function model({ watching = true, remainingSecs = 95 } = {}) {
  return {
    mode: 'standalone',
    mode_label: '감독관 화면',
    view: {
      phase: watching
        ? { watching: true, title: '감시 중', detail: '시험 시작 뒤 새로 나타난 무선 신호를 지켜보고 있습니다.', remaining_secs: 0 }
        : {
            watching: false,
            title: '준비 중 — 시험 전 주변 신호를 익히는 중',
            detail: '시험 전부터 있던 기기(감독관 장비, 교실 설비 등)는 알림에서 뺍니다. 이 동안에는 알림이 나오지 않습니다.',
            remaining_secs: remainingSecs,
          },
      rooms: ['1-1반'],
      alerts: watching
        ? [
            alert('High', '이름을 숨긴 와이파이 (핫스팟일 수 있음)', '1-1반 교실 뒤쪽', {
              reasons: [
                '이름을 숨긴 와이파이입니다. 숨긴 핫스팟일 수 있습니다.',
                '휴대폰 핫스팟이 흔히 쓰는 임의 주소의 와이파이입니다.',
                '시험 전에는 없던 와이파이가 교실 안 거리에서 잡혔습니다.',
              ],
              technical: [
                '숨긴 Wi-Fi 감지 (BSSID 02:00:5E:00:53:2A)',
                '점수 11 · 적중 규칙: R3 새 Wi-Fi가 교실 경계 안, R4 랜덤 MAC 핫스팟, R5 숨긴 Wi-Fi',
                '추정 위치: 1-1반 교실 뒤쪽 · 신뢰도 높음',
                '근거: 1-1반 뒤 센서 -52dBm',
              ],
            }),
            alert('Medium', '휴대폰일 수 있는 블루투스 신호 (Apple)', '1-1반 교실', {
              reasons: [
                '휴대폰 제조사(Apple·Samsung·Google)의 블루투스 신호입니다. 휴대폰일 수 있지만 같은 제조사의 이어폰·시계·보청기일 수도 있습니다.',
                '시험 전에는 없던 블루투스 기기가 교실 안 거리에서 잡혔습니다.',
              ],
              caveats: ['수거한 휴대폰 보관 위치에서 나온 신호일 수 있어 의심 정도를 낮췄습니다.'],
              technical: ['BLE 광고 감지 (Apple)', '점수 6 · 적중 규칙: R1 폰 제조사 BLE 광고, R2 새 BLE 기기가 교실 경계 안'],
            }),
          ]
        : [],
      sensors: [
        {
          sensor: '1-1반-뒤',
          place: '1-1반 교실 뒤쪽 노트북',
          connected: true,
          severity: 'problem',
          summary: '문제 있음 — 일부 신호를 받지 못함',
          collectors: [
            { name: '블루투스 신호 감지', state: '정상 작동 중', severity: 'ok', technical: 'BLE 수집: 수집 중' },
            {
              name: '와이파이 신호 감지',
              state: 'macOS 위치 권한이 없어 와이파이 주소를 볼 수 없습니다.',
              severity: 'problem',
              technical: 'Wi-Fi 수집: 꺼짐 — 위치 권한 없음',
            },
          ],
        },
      ],
      notices: [
        {
          severity: 'problem',
          place: '1-1반 교실 뒤쪽 노트북',
          problem: '와이파이 신호 감지: macOS 위치 권한이 없어 와이파이 주소를 볼 수 없습니다.',
          effect: '이 자리에서 휴대폰 핫스팟(숨긴 와이파이 포함)을 놓칠 수 있습니다.',
          action: '시스템 설정 > 개인정보 보호 및 보안 > 위치 서비스에서 이 앱을 허용하세요.',
          technical: '1-1반-뒤 — Wi-Fi 수집: 꺼짐 — 위치 권한 없음',
          fix: 'open_location_settings',
        },
        {
          severity: 'warning',
          place: '1-1반 교실 뒤쪽 노트북',
          problem: '이 자리의 신호 기준값(교실 보정)이 설정되지 않았습니다.',
          effect: '기기가 교실 안인지 밖인지 가리지 못해 일부 알림이 나오지 않고 위치가 부정확할 수 있습니다.',
          action: "아래 '교실 보정하기'를 눌러 휴대폰으로 보정하세요(1분쯤 걸립니다). 건물 단위 구성은 설치 담당자가 설정 파일에 넣습니다.",
          technical: '1-1반-뒤 센서: 교실 보정값 없음',
          fix: 'calibrate',
        },
      ],
    },
    header: [],
    connection: null,
    log: ['[확인 필요] 숨긴 Wi-Fi 감지 (BSSID 02:00:5E:00:53:2A)'],
    notice: null,
    can_save_report: true,
  };
}

/** 시연용 교실 보정 진행 상황(`--screen calibration-<단계>`). 문구는 core의 보정 문구와 같고, 기기 이름은 합성 예시다. */
function calibration(stage) {
  const bluetooth = [
    { id: 'b1', radio: 'Bluetooth', label: '시험 휴대폰', detail: 'Apple', rssi: -41 },
    { id: 'b4', radio: 'Bluetooth', label: '이름 없는 Samsung 기기', detail: '', rssi: -68 },
    { id: 'b2', radio: 'Bluetooth', label: '이름 없는 블루투스 기기', detail: '', rssi: -79 },
  ];
  const wifi = [
    { id: 'w3', radio: 'Wifi', label: '시험 핫스팟', detail: '5GHz', rssi: -37 },
    { id: 'w5', radio: 'Wifi', label: '학교 와이파이(예시)', detail: '2.4GHz', rssi: -63 },
    { id: 'w6', radio: 'Wifi', label: '이름을 숨긴 와이파이', detail: '2.4GHz', rssi: -82 },
  ];
  const base = {
    stage,
    elapsed_secs: 0,
    total_secs: 20,
    bluetooth: [],
    wifi: [],
    selected_bluetooth: bluetooth[0],
    selected_wifi: wifi[0],
    bluetooth_samples: 0,
    wifi_samples: 0,
    needed_samples: 3,
    bluetooth_dbm: null,
    wifi_dbm: null,
    problem: null,
    problem_radio: null,
  };
  const steps = {
    listing: {
      bluetooth,
      wifi,
      selected_bluetooth: null,
      selected_wifi: null,
      message: '휴대폰의 블루투스와 개인 핫스팟을 켜고 노트북 가까이 두세요. 아래 목록에서 내 휴대폰(블루투스)과 내 핫스팟(와이파이)을 하나씩 고르세요. 가까이 있는 기기가 위에 옵니다.',
    },
    ready_for_far: {
      message: "이제 휴대폰을 교실에서 이 노트북과 가장 먼 자리(교실 끝)의 책상 위에 두고, 휴대폰에서 떨어진 뒤 '교실 끝에 두었어요'를 눌러 주세요. 블루투스와 핫스팟은 켜 둔 채로 둡니다.",
    },
    far: {
      elapsed_secs: 14,
      bluetooth_samples: 9,
      wifi_samples: 1,
      message: '교실 끝에 둔 휴대폰의 블루투스와 핫스팟 신호를 재는 중입니다. 와이파이는 몇 초마다 한 번씩 잡혀 1분 가까이 걸릴 수 있습니다. 휴대폰 가까이에 서 있지 마세요.',
    },
    done: {
      bluetooth_dbm: -78,
      wifi_dbm: -61,
      message: '교실 보정을 마쳤습니다. 저장하면 이 값을 기준으로 새 기기가 교실 안일 가능성을 살핍니다. 휴대폰 한 대로 한 자리에서 잰 값이라 참고용입니다.',
    },
  };
  return { progress: { ...base, ...steps[stage] }, bluetooth_problem: null, wifi_problem: null, ended: false };
}

/** 시연용 처음 설정. */
const SETUP = Object.freeze({ room: '1-1반', row: 4, column: 3, baselineMinutes: 3 });

module.exports = { model, calibration, SETUP };
