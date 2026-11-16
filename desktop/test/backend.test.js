// 처음 설정·백엔드 잇기 순수 함수 시험. 시험 이름은 Rust 규칙처럼 "조건 → 동작 → 관찰값"으로 쓴다.

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');

const backend = require('../src/backend');
const demo = require('../src/demo');

// 5×5 교실 그림의 맨 뒤 줄(4), 동쪽(3).
const SETUP = Object.freeze({ room: ' 1-1반 ', row: 4, column: 3, baselineMinutes: 3 });

test('온전한 처음 설정이면 문제 목록이 비어 있다', () => {
  assert.deepEqual(backend.setupProblems(SETUP), []);
});

test('빈 설정이면 교실·자리·준비 시간 세 가지를 모두 알려 준다', () => {
  assert.equal(backend.setupProblems({}).length, 3);
  assert.equal(backend.setupProblems(null).length, 3);
});

test('격자 밖 칸(5, -1, 소수, 문자열)은 자리로 받지 않는다', () => {
  const bad = [{ row: 5 }, { row: -1 }, { column: 1.5 }, { column: 'east' }];
  assert.deepEqual(
    bad.map((cell) => backend.setupProblems({ ...SETUP, ...cell }).length),
    [1, 1, 1, 1],
  );
});

test('따옴표·줄바꿈·너무 긴 교실 이름은 받지 않는다', () => {
  assert.deepEqual(
    ['1-1"반', '1-1\n반', 'ㄱ'.repeat(21), '   ', '1-1반'].map(backend.validRoom),
    [false, false, false, false, true],
  );
});

test('센서 이름과 자리 설명은 교실 이름의 앞뒤 공백을 지운다', () => {
  assert.deepEqual(
    [backend.sensorId(SETUP), backend.placeLabel(SETUP)],
    ['1-1반-노트북', '1-1반 · 맨 뒤 동쪽'],
  );
});

test('단독 모드 설정 파일에는 교실·자리·준비 시간이 들어가고 교실 보정값은 들어가지 않는다', () => {
  const toml = backend.standaloneToml(SETUP, '/상태');
  assert.match(toml, /^mode = "standalone"$/m);
  assert.match(toml, /^id = "1-1반-노트북"$/m);
  assert.match(toml, /^position = "Back"$/m);
  assert.match(toml, /place = "맨 뒤 동쪽"/);
  assert.match(toml, /^baseline_ms = 180000$/m);
  assert.match(toml, /^state_dir = "\/상태"$/m);
  assert.doesNotMatch(toml, /inside_boundary_dbm/);
});

test('잘못된 설정으로는 설정 파일을 만들지 않는다', () => {
  assert.throws(() => backend.standaloneToml({ ...SETUP, row: 7 }, '/상태'), /노트북이 놓인 칸/);
});

test('칸 설명 → 가운데는 한가운데, 가운데 줄은 줄 이름을 붙인다', () => {
  assert.deepEqual(
    [backend.cellLabel(2, 2), backend.cellLabel(2, 0), backend.cellLabel(0, 4), backend.cellLabel(4, 2)],
    ['한가운데', '가운데 줄 서쪽 끝', '칠판 바로 앞 동쪽 끝', '맨 뒤 가운데'],
  );
});

test('칠판 쪽 세 줄은 core의 앞, 뒤 두 줄은 뒤로 넘긴다', () => {
  const position = (row) => backend.standaloneToml({ ...SETUP, row }, '/상태').match(/^position = "(\w+)"$/m)[1];
  assert.deepEqual([0, 1, 2, 3, 4].map(position), ['Front', 'Front', 'Front', 'Back', 'Back']);
});

test('보정 자리 추천 → 노트북에서 가장 먼 구석을 고른다', () => {
  const spot = (row, column) => backend.calibrationSpot({ row, column }).label;
  assert.deepEqual(
    [spot(4, 3), spot(0, 0), spot(1, 4), spot(3, 1)],
    ['칠판 바로 앞 서쪽 끝', '맨 뒤 동쪽 끝', '맨 뒤 서쪽 끝', '칠판 바로 앞 동쪽 끝'],
  );
});

test('가운데 줄·칸 노트북 → 두 구석이 똑같이 멀어 뒤쪽·서쪽 구석을 추천한다', () => {
  const spot = (row, column) => backend.calibrationSpot({ row, column });
  assert.deepEqual(
    [spot(2, 2), spot(2, 4), spot(0, 2), spot(2, 0), spot(4, 2)],
    [
      { row: 4, column: 0, label: '맨 뒤 서쪽 끝' },
      { row: 4, column: 0, label: '맨 뒤 서쪽 끝' },
      { row: 4, column: 0, label: '맨 뒤 서쪽 끝' },
      { row: 4, column: 4, label: '맨 뒤 동쪽 끝' },
      { row: 0, column: 0, label: '칠판 바로 앞 서쪽 끝' },
    ],
  );
});

test('예전 앞·뒤 × 서·가운데·동 설정 → 5×5 칸으로 옮기고 보정값은 그대로 둔다', () => {
  const old = { room: '1-1반', row: 'back', column: 'center', baselineMinutes: 3, boundaryDbm: -70, wifiBoundaryDbm: -60 };
  const moved = backend.migrateSetup(old);
  assert.deepEqual(
    [moved.row, moved.column, moved.boundaryDbm, moved.wifiBoundaryDbm, backend.setupProblems(moved), backend.migrateSetup(SETUP) === SETUP],
    [3, 2, -70, -60, [], true],
  );
});

test('상태 폴더 경로의 역슬래시·따옴표는 TOML 문자열 안에서 이스케이프된다', () => {
  const toml = backend.standaloneToml(SETUP, 'C:\\사용자\\"앱"');
  assert.match(toml, /^state_dir = "C:\\\\사용자\\\\\\"앱\\""$/m);
});

test('환경 변수가 있으면 그 실행 파일만, 배포본이면 리소스의 bin, 개발 중이면 release와 debug가 후보다', () => {
  const base = { packaged: false, resourcesPath: '/res', appRoot: '/repo/desktop', platform: 'darwin', env: {} };
  assert.deepEqual(backend.backendCandidates({ ...base, env: { TACET_BIN: '/x/c' } }), ['/x/c']);
  assert.deepEqual(backend.backendCandidates({ ...base, packaged: true, platform: 'win32' }), [path.join('/res', 'bin', 'tacet.exe')]);
  assert.deepEqual(backend.backendCandidates(base), [
    path.join('/repo', 'target', 'release', 'tacet'),
    path.join('/repo', 'target', 'debug', 'tacet'),
  ]);
});

test('후보 중 있는 것 가운데 가장 최근 빌드를 고르고, 하나도 없으면 null이다', () => {
  const times = { '/release': 10, '/debug': 20 };
  const modified = (p) => (p in times ? times[p] : null);
  assert.deepEqual(
    [backend.pickBackend(['/release', '/debug'], modified), backend.pickBackend(['/release', '/없음'], modified), backend.pickBackend(['/없음'], modified)],
    ['/debug', '/release', null],
  );
});

test('출력 조각이 줄 중간에서 끊겨도 다음 조각과 이어 메시지로 나누고, JSON이 아닌 줄은 버린다', () => {
  const first = backend.splitMessages('', '{"type":"model","model":{"a":1}}\n잡음\n{"type":"err');
  const second = backend.splitMessages(first.rest, 'or","message":"오류"}\n');
  assert.deepEqual(
    [first.messages, first.rest, second.messages, second.rest],
    [[{ type: 'model', model: { a: 1 } }], '{"type":"err', [{ type: 'error', message: '오류' }], ''],
  );
});

test('정해진 요청만 한 줄 JSON으로 만들고 그 밖의 것은 보내지 않는다', () => {
  assert.deepEqual(
    [backend.commandLine('save_report'), backend.commandLine('shutdown'), backend.commandLine('format_disk')],
    ['{"type":"save_report"}\n', '{"type":"shutdown"}\n', null],
  );
});

test('예외 기기 목록이 있으면 설정 파일 끝에 붙고, 비어 있으면 붙지 않는다', () => {
  const list = '[whitelist]\nentries = [{ Mac = [0, 0, 94, 0, 83, 1] }]';
  const withList = backend.standaloneToml(SETUP, '/상태', list);
  const without = backend.standaloneToml(SETUP, '/상태', '   ');
  assert.deepEqual(
    [withList.trimEnd().endsWith('entries = [{ Mac = [0, 0, 94, 0, 83, 1] }]'), without.includes('[whitelist]')],
    [true, false],
  );
});

test('예외 기기 목록 틀은 모두 주석이라 그대로 붙여도 설정이 바뀌지 않는다', () => {
  const lines = backend.WHITELIST_TEMPLATE.split('\n').filter((l) => l.trim());
  assert.deepEqual(lines.filter((l) => !l.startsWith('#')), []);
});

test('개발자 모드가 아니면 기술 정보와 식별자를 빼고, 개발자 모드면 그대로 넘긴다', () => {
  const model = demo.model({ watching: true });
  const shown = JSON.stringify(backend.forAudience(model, false));
  assert.deepEqual(
    [shown.includes('02:00:5E:00:53:2A'), shown.includes('R5'), shown.includes('이름을 숨긴 와이파이'), backend.forAudience(model, true) === model, backend.forAudience(null, false)],
    [false, false, true, true, null],
  );
});

test('교실 보정값이 있으면 이 노트북 센서의 경계로 들어가고, 없으면 들어가지 않는다', () => {
  assert.deepEqual(
    [
      backend.standaloneToml({ ...SETUP, boundaryDbm: -78 }, '/상태').includes('place = "맨 뒤 동쪽", inside_boundary_dbm = -78 }'),
      backend.standaloneToml(SETUP, '/상태').includes('inside_boundary_dbm'),
    ],
    [true, false],
  );
});

test('보정값은 RSSI로 말이 되는 정수만 받는다', () => {
  assert.deepEqual([-78, -127, 20, -128, 21, -77.5, '-78'].map(backend.validBoundary), [true, true, true, false, false, false, false]);
  assert.equal(backend.setupProblems({ ...SETUP, boundaryDbm: -200 }).length, 1);
});

test('보정 요청은 백엔드로 보낼 수 있다', () => {
  assert.deepEqual(
    [backend.commandLine('calibrate_far'), backend.commandLine('calibrate_restart')],
    ['{"type":"calibrate_far"}\n', '{"type":"calibrate_restart"}\n'],
  );
});

test('고르기 요청은 목록 번호 모양일 때만 만든다', () => {
  assert.deepEqual(
    [backend.selectLine('b3', 'w1'), backend.selectLine('w3', 'w1'), backend.selectLine('b3', 'w1"}'), backend.selectLine('', 'w1')],
    ['{"type":"calibrate_select","bluetooth":"b3","wifi":"w1"}\n', null, null, null],
  );
});

test('블루투스·와이파이 보정값이 각각 설정 파일에 들어간다', () => {
  const toml = backend.standaloneToml({ ...SETUP, boundaryDbm: -78, wifiBoundaryDbm: -61 }, '/상태');
  assert.ok(toml.includes('inside_boundary_dbm = -78, wifi_inside_boundary_dbm = -61 }'));
});
