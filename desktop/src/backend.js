// 백엔드(Rust `tacet`)와 처음 설정에 관한 순수 함수. Electron 없이 node:test로 시험한다.
//
// 판정은 백엔드가 한다. 여기서는 감독관이 고른 교실·자리를 설정 파일(TOML)로 옮기고, 백엔드가 보내는 JSON 줄을
// 나누기만 한다(AGENTS.md §2 규칙 11).

'use strict';

const path = require('node:path');

/** 교실 그림의 칸 수(가로·세로). 칠판을 남쪽에 둔 5×5 격자에서 노트북 자리를 고른다. */
const GRID = 5;

/** 줄: 0이 칠판 바로 앞(남쪽), 4가 맨 뒤(북쪽). */
const ROWS = Object.freeze(['칠판 바로 앞', '앞쪽', '가운데 줄', '뒤쪽', '맨 뒤'].map((label) => Object.freeze({ label })));

/** 칸: 0이 서쪽 끝, 4가 동쪽 끝. */
const COLUMNS = Object.freeze(['서쪽 끝', '서쪽', '가운데', '동쪽', '동쪽 끝'].map((label) => Object.freeze({ label })));

const MIDDLE = (GRID - 1) / 2;

function validCell(value) {
  return Number.isInteger(value) && value >= 0 && value < GRID;
}

/** 칸의 자리 설명(예: "맨 뒤 동쪽", "교실 한가운데"). 안내 문구의 "어디"에 그대로 들어간다. */
function cellLabel(row, column) {
  if (row === MIDDLE && column === MIDDLE) return '한가운데';
  return `${ROWS[row].label} ${COLUMNS[column].label}`;
}

/**
 * core의 앞·뒤. 단독 모드는 교실에 노트북이 하나라 core가 앞뒤를 위치 판정에 쓰지 않는다(교실 센서가 둘 이상일 때만 —
 * `crates/core/src/locate/mod.rs`). 안내 문구의 자리는 `place`(칸 설명)로 따로 넘기므로 가운데 줄은 앞으로 둔다.
 */
function corePosition(row) {
  return row <= MIDDLE ? 'Front' : 'Back';
}

/**
 * 교실 보정에서 휴대폰을 둘 자리 추천: 이 노트북에서 가장 먼 구석. 경계는 "교실 끝 휴대폰이 얼마나 세게 잡히나"라서
 * 가장 먼 곳에서 재야 교실 안 기기가 모두 경계 안에 든다(`docs/rules/L-room-calibration.md`). 가운데 줄·칸이면 두 구석이
 * 똑같이 멀어 뒤쪽·서쪽 구석을 고른다(칠판 쪽은 감독관 책상과 사람이 많다).
 */
// TODO(field-unverified): 격자에서 먼 칸이 신호가 가장 약한 칸인지, 복도·옆 교실 벽에 붙은 구석에서 잰 경계가 이웃 기기를
// "교실 안"으로 끌어들이지 않는지(노트 "오탐 요인"·"미검증").
function calibrationSpot(setup) {
  const row = setup.row < MIDDLE ? GRID - 1 : setup.row > MIDDLE ? 0 : GRID - 1;
  const column = setup.column < MIDDLE ? GRID - 1 : setup.column > MIDDLE ? 0 : 0;
  return { row, column, label: cellLabel(row, column) };
}

/** 예전(앞·뒤 × 서·가운데·동) 설정을 5×5 칸으로 옮긴다. 이미 새 모양이면 그대로 둔다. */
function migrateSetup(setup) {
  if (!setup || typeof setup.row !== 'string') return setup;
  const rows = { front: 1, back: 3 };
  const columns = { west: 0, center: 2, east: 4 };
  return { ...setup, row: rows[setup.row], column: columns[setup.column] };
}

/** 기준선(시험 전 주변 신호 익히기) 시간 선택지(분). */
const BASELINE_MINUTES = Object.freeze([1, 3, 5]);
const DEFAULT_BASELINE_MINUTES = 3;

/** 교실 이름으로 쓸 수 있는지. 비었거나 너무 길거나 줄바꿈·따옴표가 있으면 안 된다. */
function validRoom(room) {
  if (typeof room !== 'string') return false;
  const trimmed = room.trim();
  return trimmed.length > 0 && trimmed.length <= 20 && !/["\\\n\r\t]/.test(trimmed);
}

/**
 * 교실 보정값(교실 끝 휴대폰 신호 세기, dBm)으로 쓸 수 있는 값인지. 판정 기준이 아니라 받은 값이 RSSI로 말이 되는지만 본다
 * (블루투스 RSSI는 -127–20dBm 정수).
 */
function validBoundary(value) {
  return Number.isInteger(value) && value >= -127 && value <= 20;
}

/** 처음 설정이 온전한지. 틀린 곳을 쉬운 말로 돌려준다(없으면 빈 목록). */
function setupProblems(setup) {
  const problems = [];
  if (!setup || !validRoom(setup.room)) problems.push('교실 이름을 1–20자로 적어 주세요(따옴표 없이).');
  if (!setup || !validCell(setup.row) || !validCell(setup.column)) problems.push('교실 그림에서 노트북이 놓인 칸을 눌러 주세요.');
  if (!setup || !BASELINE_MINUTES.includes(setup.baselineMinutes)) problems.push('준비 시간을 골라 주세요.');
  const badBoundary = (value) => value !== undefined && value !== null && !validBoundary(value);
  if (setup && (badBoundary(setup.boundaryDbm) || badBoundary(setup.wifiBoundaryDbm))) {
    problems.push('교실 보정값이 올바르지 않습니다. 교실 보정을 다시 해 주세요.');
  }
  return problems;
}

/** 이 노트북의 센서 이름(예: "1-1반-노트북"). 교실에 노트북이 하나라 교실 이름으로 충분하다. */
function sensorId(setup) {
  return `${setup.room.trim()}-노트북`;
}

/** 화면 머리글에 보일 자리 설명(예: "1-1반 · 맨 뒤 동쪽"). */
function placeLabel(setup) {
  return `${setup.room.trim()} · ${cellLabel(setup.row, setup.column)}`;
}

/** TOML 기본 문자열. JSON 문자열 이스케이프가 TOML 기본 문자열과 호환된다. */
function tomlString(value) {
  return JSON.stringify(String(value));
}

/** 예외 기기(감독관 장비, 사전 신고한 의료기기·보청기) 목록 파일 이름. 앱 데이터 폴더에 둔다. */
const WHITELIST_FILE = 'whitelist.toml';

/** 예외 기기 목록을 처음 열 때 만드는 틀. 주소 예시는 문서용 범위다(AGENTS.md §3.3). */
const WHITELIST_TEMPLATE = [
  '# 예외 기기 목록: 감독관 장비와 사전 신고한 의료기기·보청기. 여기 적은 기기는 알림과 상세 화면에 나오지 않는다.',
  '# 고친 뒤 개발자 모드의 "다시 시작"을 누르거나 앱을 다시 켠다. 형식이 틀리면 화면에 오류가 뜬다.',
  '# Windows 노트북은 주소(Mac)로 등록할 수 있다. macOS 노트북은 블루투스 기기 주소를 볼 수 없으므로 블루투스 기기는',
  '# 제조사 정보(Manufacturer: 회사 번호와 제조사 데이터 앞부분)로 등록한다.',
  '#',
  '# [whitelist]',
  '# entries = [',
  '#   { Mac = [0, 0, 94, 0, 83, 1] },',
  '#   { Manufacturer = { company = 76, prefix = [1, 2] } },',
  '# ]',
  '',
].join('\n');

/**
 * 감독관 기본 모드(이 노트북 혼자 감지)의 설정 파일. 교실 보정값은 넣지 않는다 — 고정 임계치를 쓰지 않고,
 * 보정은 설치 담당자가 실측해서 넣는다(AGENTS.md §2 규칙 4). 보정이 없으면 화면에 그 사실이 안내된다.
 */
function standaloneToml(setup, stateDir, whitelist = '') {
  const problems = setupProblems(setup);
  if (problems.length > 0) throw new Error(problems.join(' '));
  const room = setup.room.trim();
  const id = sensorId(setup);
  const position = corePosition(setup.row);
  const place = cellLabel(setup.row, setup.column);
  // 교실 보정값은 앱 안 보정(docs/rules/L-room-calibration.md)으로 잰 것만 넣는다. 블루투스·와이파이를 따로 둔다. 없으면 넣지 않는다.
  const boundary =
    (validBoundary(setup.boundaryDbm) ? `, inside_boundary_dbm = ${setup.boundaryDbm}` : '') +
    (validBoundary(setup.wifiBoundaryDbm) ? `, wifi_inside_boundary_dbm = ${setup.wifiBoundaryDbm}` : '');
  return [
    '# 이 파일은 앱이 처음 설정에서 만든다. 고치려면 앱에서 "처음 설정 다시 하기"를 쓴다.',
    'mode = "standalone"',
    `state_dir = ${tomlString(stateDir)}`,
    '',
    '[sensor]',
    `id = ${tomlString(id)}`,
    `room = ${tomlString(room)}`,
    `position = ${tomlString(position)}`,
    '',
    '[detector]',
    `baseline_ms = ${setup.baselineMinutes * 60 * 1000}`,
    '',
    '[[layout.rooms]]',
    `id = ${tomlString(room)}`,
    `sensors = [{ id = ${tomlString(id)}, position = ${tomlString(position)}, place = ${tomlString(place)}${boundary} }]`,
    '',
    // 예외 기기 목록은 설치 담당자가 적은 그대로 붙인다. 형식 검사는 백엔드가 하고, 틀리면 화면에 오류로 보인다.
    whitelist.trim() ? `# 예외 기기 목록(${WHITELIST_FILE})\n${whitelist.trim()}\n` : '',
  ].join('\n');
}

/**
 * 화면에 넘길 모델. 개발자 모드가 아니면 기술 정보(규칙 ID, 신호 세기, BSSID 같은 식별자, 서버 정보, 알림 기록)를
 * 뺀다 — 식별자는 필요한 사람에게만 보인다(AGENTS.md §3.3). 원본 모델은 바꾸지 않는다.
 */
function forAudience(model, developer) {
  if (!model || developer) return model;
  const view = model.view || {};
  return {
    ...model,
    header: [],
    log: [],
    view: {
      ...view,
      alerts: (view.alerts || []).map((a) => ({ ...a, text: '', explain: { ...a.explain, technical: [] } })),
      sensors: (view.sensors || []).map((s) => ({ ...s, collectors: (s.collectors || []).map((c) => ({ ...c, technical: '' })) })),
      notices: (view.notices || []).map((n) => ({ ...n, technical: '' })),
    },
  };
}

/** 백엔드 실행 파일 후보. 배포본은 앱 리소스의 bin, 개발 중에는 저장소의 target. 환경 변수가 먼저다. */
function backendCandidates({ packaged, resourcesPath, appRoot, platform, env }) {
  const exe = platform === 'win32' ? 'tacet.exe' : 'tacet';
  if (env && env.TACET_BIN) return [env.TACET_BIN];
  if (packaged) return [path.join(resourcesPath, 'bin', exe)];
  const repo = path.resolve(appRoot, '..');
  return [path.join(repo, 'target', 'release', exe), path.join(repo, 'target', 'debug', exe)];
}

/**
 * 후보 중 쓸 실행 파일. 있는 것 중 가장 최근에 빌드한 것을 고른다(개발 중 release·debug가 섞여 있어도 새 것을 쓰게).
 * `modified(path)`는 수정 시각(ms)을, 없으면 `null`을 돌려준다.
 */
function pickBackend(candidates, modified) {
  let best = null;
  let bestTime = -Infinity;
  for (const candidate of candidates) {
    const time = modified(candidate);
    if (time !== null && time > bestTime) {
      best = candidate;
      bestTime = time;
    }
  }
  return best;
}

/**
 * 백엔드 출력 조각을 줄 단위 메시지로 나눈다. 끝나지 않은 줄은 `rest`로 돌려 다음 조각과 잇는다. JSON이 아닌 줄은
 * 버린다(백엔드 출력도 남의 입력처럼 다룬다).
 */
function splitMessages(buffer, chunk) {
  const text = buffer + chunk;
  const lines = text.split('\n');
  const rest = lines.pop();
  const messages = [];
  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    try {
      const message = JSON.parse(trimmed);
      if (message && typeof message.type === 'string') messages.push(message);
    } catch {
      // 버린다.
    }
  }
  return { messages, rest };
}

/** 화면이 백엔드로 보낼 수 있는 요청. 그 밖의 것은 보내지 않는다. */
const COMMANDS = Object.freeze(['save_report', 'open_location_settings', 'shutdown', 'calibrate_far', 'calibrate_restart']);

function commandLine(type) {
  if (!COMMANDS.includes(type)) return null;
  return `${JSON.stringify({ type })}\n`;
}

/** 보정 목록 번호(예: "b3", "w1")인지. 화면이 보낸 값이라 모양을 확인하고 넘긴다. */
function validCandidateId(id, prefix) {
  return typeof id === 'string' && new RegExp(`^${prefix}[0-9]{1,6}$`).test(id);
}

/** 교실 보정에서 고른 휴대폰(블루투스)·핫스팟(와이파이) 요청 줄. 번호 모양이 틀리면 null. */
function selectLine(bluetooth, wifi) {
  if (!validCandidateId(bluetooth, 'b') || !validCandidateId(wifi, 'w')) return null;
  return `${JSON.stringify({ type: 'calibrate_select', bluetooth, wifi })}\n`;
}

module.exports = {
  ROWS,
  COLUMNS,
  BASELINE_MINUTES,
  DEFAULT_BASELINE_MINUTES,
  validRoom,
  validBoundary,
  setupProblems,
  sensorId,
  placeLabel,
  GRID,
  cellLabel,
  calibrationSpot,
  migrateSetup,
  standaloneToml,
  forAudience,
  WHITELIST_FILE,
  WHITELIST_TEMPLATE,
  backendCandidates,
  pickBackend,
  splitMessages,
  commandLine,
  selectLine,
};
