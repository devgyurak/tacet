// 화면 표시용 순수 함수. 데스크톱 앱과 웹 대시보드가 같이 쓰고, node:test로도 시험한다.
//
// 판단하지 않는다(AGENTS.md §2 규칙 11): 무엇이 문제이고 얼마나 무거운지는 core가 정해 보낸다(severity, rooms,
// everyone). 여기서는 그 값으로 보일 것을 고르고 숫자를 사람이 읽는 말로 바꿀 뿐이다.

(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.TacetView = api;
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  const SEVERITY_ORDER = Object.freeze({ ok: 0, info: 1, warning: 2, problem: 3 });
  const LEVEL_CLASS = Object.freeze({ High: 'high', Medium: 'medium', Low: 'low' });

  /** 고른 교실에 보일 알림. 교실을 고르지 않았으면 모두. 모든 교실 알림(everyone)은 늘 보인다. */
  function visibleAlerts(view, room) {
    const alerts = (view && view.alerts) || [];
    if (!room) return alerts.slice();
    return alerts.filter((a) => a.everyone || (a.rooms || []).includes(room));
  }

  /** 보일 안내. 감독관 화면에는 주의·문제만, 개발자 화면에는 참고까지. */
  function visibleNotices(view, developer) {
    const notices = (view && view.notices) || [];
    return notices.filter((n) => developer || SEVERITY_ORDER[n.severity] >= SEVERITY_ORDER.warning);
  }

  /** 받은 화면에 내용이 있는지(아직 아무것도 받지 못한 원격 화면과 구분한다). */
  function hasData(view) {
    return Boolean(view && view.phase && view.phase.title);
  }

  /** 머리글의 상태 요약. */
  function health(view, developer) {
    if (!hasData(view)) return { severity: 'info', text: '상태 정보 없음' };
    const notices = visibleNotices(view, developer);
    const problems = notices.filter((n) => n.severity === 'problem').length;
    const warnings = notices.filter((n) => n.severity === 'warning').length;
    if (problems > 0) {
      return { severity: 'problem', text: warnings > 0 ? `문제 ${problems}건 · 주의 ${warnings}건` : `문제 ${problems}건` };
    }
    if (warnings > 0) return { severity: 'warning', text: `주의 ${warnings}건` };
    return { severity: 'ok', text: '감지 장치 정상' };
  }

  /** 남은 시간(초)을 "2분 5초"처럼. */
  function duration(secs) {
    const total = Math.max(0, Math.floor(Number(secs) || 0));
    const minutes = Math.floor(total / 60);
    const seconds = total % 60;
    if (minutes === 0) return `${seconds}초`;
    if (seconds === 0) return `${minutes}분`;
    return `${minutes}분 ${seconds}초`;
  }

  /** 단계 배지 문구. */
  function phaseBadge(phase) {
    if (!phase || !phase.title) return { watching: false, text: '시작하는 중' };
    if (phase.watching) return { watching: true, text: phase.title };
    if (phase.remaining_secs > 0) return { watching: false, text: `준비 중 · ${duration(phase.remaining_secs)} 남음` };
    return { watching: false, text: phase.title };
  }

  /** 의심 정도 계기에서 채울 점 수(높음 3, 보통 2, 낮음 1). core가 정한 강도를 그림으로 옮길 뿐이다. */
  function meterCount(level) {
    return { High: 3, Medium: 2, Low: 1 }[level] || 1;
  }

  /** 두 자리 수(01, 02 …). 계기판 숫자 모양. */
  function count(n) {
    const value = Math.max(0, Math.floor(Number(n) || 0));
    return value < 10 ? `0${value}` : String(value);
  }

  /** 0부터 센 순번을 1부터 센 두 자리 번호로(01, 02 …). */
  function index(i) {
    return count(Number(i) + 1);
  }

  /** 남은 초를 시계 모양으로(1:05). */
  function clock(secs) {
    const total = Math.max(0, Math.floor(Number(secs) || 0));
    const seconds = total % 60;
    return `${Math.floor(total / 60)}:${seconds < 10 ? '0' : ''}${seconds}`;
  }

  function levelClass(level) {
    return LEVEL_CLASS[level] || 'low';
  }

  function severityClass(severity) {
    return Object.hasOwn(SEVERITY_ORDER, severity) ? severity : 'info';
  }

  /** 알림 제목 앞 배지. 언제나 "확인 필요"다(AGENTS.md §3.4). */
  const CHECK_NEEDED = '확인 필요';

  return { hasData, meterCount, count, index, clock, visibleAlerts, visibleNotices, health, duration, phaseBadge, levelClass, severityClass, CHECK_NEEDED };
});
