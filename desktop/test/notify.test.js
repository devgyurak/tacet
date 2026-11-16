// 운영체제 알림에 쓰는 순수 함수 시험. 알릴지 말지는 core가 보낸 알림·안내와 "이번 세션에 처음인가"로만 정한다.

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const N = require('../src/notify');
const demo = require('../src/demo');

const alert = (title, place, rooms, level) => ({
  rooms: rooms || ['1-1반'],
  everyone: false,
  explain: { title, place, level: level || '의심 높음' },
});
const view = (alerts, notices) => ({ alerts: alerts || [], notices: notices || [] });
const notice = (severity, problem) => ({ severity, place: '1-1반 교실 뒤쪽 노트북', problem });

test('처음 보인 알림 → 새것으로 고른다 → 같은 모델을 다시 받으면 고르지 않는다', () => {
  const m = view([alert('새 블루투스 기기 신호', '1-1반 교실')]);
  const first = N.freshAlerts({}, m, '1-1반');
  const second = N.freshAlerts(first.seen, m, '1-1반');
  assert.deepEqual([first.found.length, second.found.length], [1, 0]);
});

test('알림이 잠깐 사라졌다 다시 보임 → 새것으로 고르지 않는다', () => {
  const a = alert('새 블루투스 기기 신호', '1-1반 교실');
  const first = N.freshAlerts({}, view([a]), '1-1반');
  const gone = N.freshAlerts(first.seen, view([]), '1-1반');
  const back = N.freshAlerts(gone.seen, view([a]), '1-1반');
  assert.equal(back.found.length, 0);
});

test('같은 종류·같은 곳 알림이 하나 더 생김 → 늘어난 하나만 새것이다', () => {
  const a = alert('휴대폰일 수 있는 블루투스 신호 (Apple)', '1-1반 교실');
  const first = N.freshAlerts({}, view([a]), '1-1반');
  const more = N.freshAlerts(first.seen, view([a, a]), '1-1반');
  assert.equal(more.found.length, 1);
});

test('다른 교실에만 알릴 알림 → 이 노트북 교실에서는 고르지 않는다', () => {
  const m = view([alert('새 와이파이 (핫스팟일 수 있음)', '1-2반 교실', ['1-2반'])]);
  assert.equal(N.freshAlerts({}, m, '1-1반').found.length, 0);
});

test('교실을 모르는 개발자 설정 → 모든 알림을 고른다', () => {
  const m = view([alert('ㄱ', '1-1반 교실', ['1-1반']), alert('ㄴ', '1-2반 교실', ['1-2반'])]);
  assert.equal(N.freshAlerts({}, m, null).found.length, 2);
});

test('문제·주의·참고 안내 → 문제만 새것으로 고른다', () => {
  const m = view([], [notice('problem', '블루투스가 꺼져 있습니다.'), notice('warning', 'ㄴ'), notice('info', 'ㄷ')]);
  assert.deepEqual(
    N.freshProblems({}, m).found.map((n) => n.problem),
    ['블루투스가 꺼져 있습니다.'],
  );
});

test('알림 문구 → 확인 필요·어디·무엇·의심 정도만 쓰고 기술 정보는 쓰지 않는다', () => {
  const a = demo.model({ watching: true, remainingSecs: 0 }).view.alerts[0];
  const n = N.alertNotification(a);
  const text = `${n.title} ${n.body}`;
  assert.deepEqual(
    [n.title, n.body, text.includes('BSSID'), text.includes('02:00:5E')],
    ['확인 필요 — 1-1반 교실 뒤쪽', '이름을 숨긴 와이파이 (핫스팟일 수 있음) · 의심 높음', false, false],
  );
});

test('문제 안내 문구 → 어디와 무엇이 문제인지 쓴다', () => {
  const n = N.problemNotification(notice('problem', '블루투스가 꺼져 있습니다.'));
  assert.deepEqual(n, { title: '감지 문제 — 1-1반 교실 뒤쪽 노트북', body: '블루투스가 꺼져 있습니다.' });
});

test('창을 처음 닫음 → 운영체제마다 아이콘이 있는 곳을 알려 준다', () => {
  assert.deepEqual(
    [N.backgroundNotification('darwin').body.includes('메뉴 막대'), N.backgroundNotification('win32').body.includes('작업 표시줄')],
    [true, true],
  );
});
