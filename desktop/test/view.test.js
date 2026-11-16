// 화면 표시용 순수 함수 시험. 판단은 core가 보낸 값(severity, rooms, everyone)으로만 한다.

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const V = require('../renderer/view');
const demo = require('../src/demo');

const view = (notices, alerts) => ({ phase: { watching: true, title: '감시 중' }, notices: notices || [], alerts: alerts || [] });
const notice = (severity) => ({ severity });

test('교실을 고르지 않으면 모든 알림, 고르면 그 교실 알림과 모든 교실 알림만 보인다', () => {
  const alerts = [
    { rooms: ['1-1반'], everyone: false, text: 'ㄱ' },
    { rooms: ['1-2반'], everyone: false, text: 'ㄴ' },
    { rooms: [], everyone: true, text: 'ㄷ' },
  ];
  const v = view([], alerts);
  assert.deepEqual(
    [V.visibleAlerts(v, null).map((a) => a.text), V.visibleAlerts(v, '1-1반').map((a) => a.text)],
    [['ㄱ', 'ㄴ', 'ㄷ'], ['ㄱ', 'ㄷ']],
  );
});

test('감독관 화면에는 주의·문제만, 개발자 화면에는 참고까지 보인다', () => {
  const v = view([notice('problem'), notice('warning'), notice('info')]);
  assert.deepEqual([V.visibleNotices(v, false).length, V.visibleNotices(v, true).length], [2, 3]);
});

test('상태 요약은 문제를 먼저, 그다음 주의를, 없으면 정상을 말하고, 받은 화면이 없으면 정보 없음이다', () => {
  assert.deepEqual(
    [
      V.health(view([notice('problem'), notice('warning')]), false),
      V.health(view([notice('warning')]), false),
      V.health(view([notice('info')]), false),
      V.health({ phase: { title: '' } }, false),
    ],
    [
      { severity: 'problem', text: '문제 1건 · 주의 1건' },
      { severity: 'warning', text: '주의 1건' },
      { severity: 'ok', text: '감지 장치 정상' },
      { severity: 'info', text: '상태 정보 없음' },
    ],
  );
});

test('남은 시간은 분과 초로 읽기 쉽게 쓴다', () => {
  assert.deepEqual([0, 5, 60, 125, -3, 'x'].map(V.duration), ['0초', '5초', '1분', '2분 5초', '0초', '0초']);
});

test('단계 배지는 감시 중, 남은 시간이 있는 준비 중, 아직 모름을 구분한다', () => {
  assert.deepEqual(
    [
      V.phaseBadge({ watching: true, title: '감시 중', remaining_secs: 0 }),
      V.phaseBadge({ watching: false, title: '준비 중', remaining_secs: 95 }),
      V.phaseBadge(null),
    ],
    [
      { watching: true, text: '감시 중' },
      { watching: false, text: '준비 중 · 1분 35초 남음' },
      { watching: false, text: '시작하는 중' },
    ],
  );
});

test('모르는 강도·무게는 가장 약한 표시로 그린다', () => {
  assert.deepEqual(
    [V.levelClass('High'), V.levelClass('Medium'), V.levelClass('무엇'), V.severityClass('problem'), V.severityClass('무엇')],
    ['high', 'medium', 'low', 'problem', 'info'],
  );
});

test('알림 배지는 언제나 확인 필요다', () => {
  assert.equal(V.CHECK_NEEDED, '확인 필요');
});

test('시연 모델의 식별자는 문서용 합성 범위만 쓴다', () => {
  const text = JSON.stringify(demo.model({ watching: true }));
  const macs = text.match(/[0-9A-F]{2}(:[0-9A-F]{2}){5}/gi) || [];
  assert.deepEqual(macs.filter((m) => !/^0[0-3]:00:5E:(00:53|90:10):/i.test(m)), []);
});

test('계기판 숫자는 두 자리, 순번은 1부터, 남은 시간은 분:초로 쓴다', () => {
  assert.deepEqual(
    [V.count(0), V.count(7), V.count(12), V.index(0), V.index(9), V.clock(65), V.clock(5), V.clock(-1)],
    ['00', '07', '12', '01', '10', '1:05', '0:05', '0:00'],
  );
});

test('의심 정도 계기는 높음 3칸, 보통 2칸, 낮음 1칸이다', () => {
  assert.deepEqual(['High', 'Medium', 'Low', '무엇'].map(V.meterCount), [3, 2, 1, 1]);
});
