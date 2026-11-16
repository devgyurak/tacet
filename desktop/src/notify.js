// 운영체제 알림(창을 닫아 둔 동안에도 감독관이 알 수 있게)에 쓰는 순수 함수.
//
// 화면은 판단하지 않는다(AGENTS.md §2 규칙 11). 무엇을 알릴지는 core가 보낸 알림·안내(무게, 알릴 교실)를 그대로 따르고,
// 여기서는 "이번 세션에 처음 보인 것인가"만 가린다. 알림 문구에는 식별자를 넣지 않는다 — 운영체제 알림 센터가 기록을
// 남기므로 core가 만든 쉬운 말(무엇·어디·의심 정도)만 쓴다(AGENTS.md §3.3).

'use strict';

const V = require('../renderer/view');

/** 같은 알림인지 가리는 열쇠. 화면 모델에는 기기 식별자가 없으므로 무엇이 보였나와 어디인지로 가린다. */
function alertKey(alert) {
  return `${alert.explain.title}\u0000${alert.explain.place}`;
}

/** 같은 안내인지 가리는 열쇠. */
function noticeKey(notice) {
  return `${notice.place}\u0000${notice.problem}`;
}

/**
 * 이번에 새로 알릴 것을 고른다. `seen`은 열쇠마다 이번 세션에 한꺼번에 보인 가장 많은 개수다. 같은 열쇠가 그보다 많아질
 * 때만 새것으로 본다 — 신호가 잠깐 끊겼다 다시 잡혀도 알림이 거듭 오지 않고, 같은 종류 기기가 하나 더 나타나면 온다.
 */
function fresh(seen, items, keyOf) {
  const counts = new Map();
  const found = [];
  for (const item of items) {
    const key = keyOf(item);
    const n = (counts.get(key) || 0) + 1;
    counts.set(key, n);
    if (n > (seen[key] || 0)) found.push(item);
  }
  const next = { ...seen };
  for (const [key, n] of counts) next[key] = Math.max(next[key] || 0, n);
  return { found, seen: next };
}

/** 화면 모델(`UiModel.view`)에서 이 노트북 교실에 보일 알림 중 새것. 교실을 모르면(개발자 설정 파일) 모든 알림을 본다. */
function freshAlerts(seen, view, room) {
  return fresh(seen, V.visibleAlerts(view, room || null), alertKey);
}

/** 감지가 끊길 수 있는 문제 안내 중 새것. 주의·참고는 창에서만 보인다 — 창을 닫아 둔 동안 놓치면 안 되는 것만 알린다. */
function freshProblems(seen, view) {
  const problems = ((view && view.notices) || []).filter((n) => n.severity === 'problem');
  return fresh(seen, problems, noticeKey);
}

/** 알림 한 건의 운영체제 알림 문구. */
function alertNotification(alert) {
  return {
    title: `${V.CHECK_NEEDED} — ${alert.explain.place}`,
    body: `${alert.explain.title} · ${alert.explain.level}`,
  };
}

/** 문제 안내 한 건의 운영체제 알림 문구. */
function problemNotification(notice) {
  return {
    title: `감지 문제 — ${notice.place}`,
    body: notice.problem,
  };
}

/** 창을 처음 닫았을 때 한 번 알리는 문구. */
function backgroundNotification(platform) {
  const where = platform === 'darwin' ? '메뉴 막대' : '작업 표시줄 알림 영역';
  return {
    title: '창을 닫아도 감지는 계속됩니다',
    body: `새 알림은 이렇게 알려 드립니다. 끄려면 ${where}의 Tacet 아이콘에서 '종료'를 누르세요.`,
  };
}

module.exports = {
  alertKey,
  noticeKey,
  freshAlerts,
  freshProblems,
  alertNotification,
  problemNotification,
  backgroundNotification,
};
