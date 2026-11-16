// 감독관 화면. 데스크톱 앱(Electron, window.tacet)과 웹 대시보드(브라우저, /events) 두 곳에서 돈다.
//
// 판단하지 않는다(AGENTS.md §2 규칙 11): 받은 모델을 그리고, 사용자의 요청을 전달할 뿐이다. 모든 글은 textContent로
// 넣는다 — 와이파이 이름 같은 남의 문자열이 화면 코드가 되지 않게 한다.

(function () {
  'use strict';

  const V = window.TacetView;
  const bridge = window.tacet || null;
  const root = document.getElementById('app');

  const ui = {
    state: null,
    model: null,
    webError: null,
    room: readStored('tacet.room'),
    webDeveloper: readStored('tacet.developer') === 'on',
    draft: null,
    problems: [],
    saving: false,
    open: new Set(),
    /** 설정 메뉴가 열려 있는지. 화면은 모델이 올 때마다 다시 그려지므로 열림 상태를 여기 둔다. */
    menuOpen: false,
    /** 교실 보정에서 고른 목록 번호. */
    calSel: { bluetooth: null, wifi: null },
  };

  // ---------------------------------------------------------------- 도우미

  function readStored(key) {
    try {
      return window.localStorage.getItem(key);
    } catch {
      return null;
    }
  }

  function writeStored(key, value) {
    try {
      if (value === null) window.localStorage.removeItem(key);
      else window.localStorage.setItem(key, value);
    } catch {
      // 저장하지 못해도 화면은 돈다.
    }
  }

  /** 요소를 만든다. 글은 언제나 텍스트 노드로 넣는다. */
  function h(tag, props, ...children) {
    const el = document.createElement(tag);
    for (const [key, value] of Object.entries(props || {})) {
      if (value === null || value === undefined || value === false) continue;
      if (key === 'class') el.className = value;
      else if (key.startsWith('on') && typeof value === 'function') el.addEventListener(key.slice(2), value);
      else if (key === 'value') el.value = value;
      else el.setAttribute(key, value === true ? '' : String(value));
    }
    for (const child of children.flat(Infinity)) {
      if (child === null || child === undefined || child === false) continue;
      el.append(child instanceof Node ? child : document.createTextNode(String(child)));
    }
    return el;
  }

  const ICONS = {
    shield: 'M12 2 4 5v6c0 5 3.4 9.3 8 11 4.6-1.7 8-6 8-11V5l-8-3Z',
    pin: 'M12 22s7-6.2 7-12a7 7 0 1 0-14 0c0 5.8 7 12 7 12Zm0-9.5a2.5 2.5 0 1 1 0-5 2.5 2.5 0 0 1 0 5Z',
    check: 'M20 6 9 17l-5-5',
    alert: 'M12 9v4m0 4h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z',
    info: 'M12 16v-4m0-4h.01M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0Z',
    clock: 'M12 7v5l3 2m7-2a10 10 0 1 1-20 0 10 10 0 0 1 20 0Z',
    laptop: 'M4 5h16v10H4zM2 19h20',
    phone: 'M7 2h10v20H7zM11 18h2',
    wrench: 'M14.7 6.3a4 4 0 0 0-5.4 5.4L3 18l3 3 6.3-6.3a4 4 0 0 0 5.4-5.4l-2.6 2.6-2.4-.6-.6-2.4 2.6-2.6Z',
  };

  function icon(name, cls) {
    const ns = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(ns, 'svg');
    svg.setAttribute('viewBox', '0 0 24 24');
    svg.setAttribute('aria-hidden', 'true');
    svg.setAttribute('class', `icon ${cls || ''}`);
    const path = document.createElementNS(ns, 'path');
    path.setAttribute('d', ICONS[name]);
    svg.append(path);
    return svg;
  }

  function developer() {
    return bridge ? Boolean(ui.state && ui.state.developer) : ui.webDeveloper;
  }

  function send(type) {
    if (bridge) bridge.command(type);
  }

  // ---------------------------------------------------------------- 교실 그림

  /**
   * 칠판을 남쪽에 둔 5×5 교실 그림. 위가 맨 뒤(북쪽)다. `onPick`이 있으면 칸을 눌러 고르고, 없으면 노트북과 보정 자리
   * 추천(`spot`)만 그린다. 감독관 노트북·보정용 휴대폰 자리만 그린다 — 알림 위치를 칸으로 그리지 않는다(좌석 단위로
   * 특정하지 않는다, AGENTS.md §3.4).
   */
  function roomMap({ laptop, spot, onPick }) {
    const options = ui.state.options;
    const same = (a, row, column) => Boolean(a) && a.row === row && a.column === column;
    const cell = (row, column) => {
      const isLaptop = same(laptop, row, column);
      const isSpot = same(spot, row, column);
      const label = options.cellLabels[row][column];
      const content = isLaptop
        ? [icon('laptop', 'seat-icon'), h('span', { class: 'seat-label' }, '이 노트북')]
        : isSpot
          ? [icon('phone', 'seat-icon'), h('span', { class: 'seat-label' }, '휴대폰')]
          : h('span', { class: 'desk' });
      const cls = `seat${isLaptop ? ' selected' : ''}${isSpot ? ' spot' : ''}${onPick ? '' : ' static'}`;
      return onPick
        ? h(
            'button',
            {
              type: 'button',
              class: cls,
              'aria-pressed': isLaptop ? 'true' : 'false',
              'aria-label': label,
              onclick: () => onPick(row, column),
            },
            content,
          )
        : h('div', { class: cls, 'aria-label': label }, content);
    };
    const rows = [];
    for (let row = options.grid - 1; row >= 0; row -= 1) {
      rows.push(h('div', { class: 'row-label' }, options.rows[row].label));
      for (let column = 0; column < options.grid; column += 1) rows.push(cell(row, column));
    }
    return h(
      'div',
      { class: `room-map${onPick ? '' : ' compact'}` },
      h('div', { class: 'compass north' }, '북'),
      h(
        'div',
        { class: 'map-body' },
        h('div', { class: 'compass west' }, '서'),
        h('div', { class: 'map-grid' }, rows),
        h('div', { class: 'compass east' }, '동'),
      ),
      h('div', { class: 'blackboard' }, '칠판 (남쪽)'),
    );
  }

  /** 교실 보정에서 휴대폰을 둘 자리 추천: 그림과 한 줄 설명. */
  function spotAdvice() {
    const spot = ui.state.calibrationSpot;
    const setup = ui.state.setup;
    if (!spot || !setup) return null;
    return h(
      'div',
      { class: 'cal-spot' },
      roomMap({ laptop: { row: setup.row, column: setup.column }, spot }),
      h(
        'div',
        { class: 'cal-spot-text' },
        h('span', { class: 'quiet' }, '휴대폰을 둘 자리'),
        h('strong', {}, spot.label),
        h(
          'p',
          { class: 'hint' },
          '이 노트북에서 가장 먼 구석입니다. 교실 끝에서 잰 세기가 교실 안팎을 가르는 기준이 되므로, 교실 안에서 가장 먼 곳에서 재야 교실 안의 기기를 빠뜨리지 않습니다.',
        ),
        h('p', { class: 'hint' }, '구석 책상 위에 두되 벽에 바짝 붙이지 말고 한 걸음 안쪽에 두세요. 벽에 붙이면 복도·옆 교실 기기까지 교실 안으로 볼 수 있습니다.'),
        h('p', { class: 'hint' }, '휴대폰에서 손을 뗀 채 2–3걸음 떨어져 주세요. 몸이 신호를 가려 값이 틀어집니다.'),
        h('p', { class: 'hint subtle' }, '이 그림은 노트북과 보정용 휴대폰 자리만 보여 줍니다.'),
      ),
    );
  }

  // ---------------------------------------------------------------- 처음 설정

  function defaultDraft() {
    const options = ui.state.options;
    return { room: '', row: null, column: null, baselineMinutes: options.defaultBaselineMinutes };
  }

  function onboarding() {
    const options = ui.state.options;
    const draft = ui.draft || (ui.draft = defaultDraft());

    const roomInput = h('input', {
      id: 'room',
      class: 'text-input',
      type: 'text',
      maxlength: '20',
      placeholder: '예: 1-1반',
      autocomplete: 'off',
      value: draft.room,
      oninput: (e) => {
        draft.room = e.target.value;
      },
    });

    const map = roomMap({
      laptop: draft.row === null ? null : { row: draft.row, column: draft.column },
      onPick: (row, column) => {
        draft.row = row;
        draft.column = column;
        render();
      },
    });

    const chosen =
      draft.row !== null && draft.column !== null
        ? `선택한 자리: ${options.cellLabels[draft.row][draft.column]}`
        : '노트북이 놓인 칸을 눌러 주세요.';

    const minutes = h(
      'div',
      { class: 'segmented', role: 'radiogroup', 'aria-label': '준비 시간' },
      options.baselineMinutes.map((m) =>
        h(
          'button',
          {
            type: 'button',
            role: 'radio',
            'aria-checked': draft.baselineMinutes === m ? 'true' : 'false',
            class: draft.baselineMinutes === m ? 'active' : '',
            onclick: () => {
              draft.baselineMinutes = m;
              render();
            },
          },
          `${m}분`,
        ),
      ),
    );

    const submit = async () => {
      ui.saving = true;
      render();
      const result = await bridge.saveSetup({ ...draft });
      ui.saving = false;
      ui.problems = result.ok ? [] : result.problems;
      if (result.ok) ui.draft = null;
      render();
    };

    return shell(
      h(
        'div',
        { class: 'onboarding-card' },
        cardHead('처음 설정', '세 가지를 정하고 교실 보정으로 넘어갑니다'),
        h('p', { class: 'lead' }, '이 노트북을 둔 교실과 자리를 알려 주세요. 시험 중 교실에 새로 나타난 휴대폰·스마트 안경의 무선 신호를 찾아 알려 드립니다.'),
        h(
          'section',
          { class: 'step' },
          h('div', { class: 'step-head' }, h('span', { class: 'step-no' }, '1'), h('label', { for: 'room' }, '어느 교실인가요?')),
          roomInput,
          h('p', { class: 'hint' }, '감독관이 알아보기 쉬운 이름이면 됩니다. 알림에 이 이름이 나옵니다.'),
        ),
        h(
          'section',
          { class: 'step' },
          h('div', { class: 'step-head' }, h('span', { class: 'step-no' }, '2'), h('span', {}, '이 노트북은 교실 어디에 있나요?')),
          h('p', { class: 'hint' }, '칠판이 아래(남쪽)에 있는 5×5 교실 그림입니다. 노트북을 둔 칸을 누르세요.'),
          map,
          h('p', { class: 'chosen' }, chosen),
          h('p', { class: 'hint subtle' }, '고른 칸은 안내 문구의 "어디"와 교실 보정 때 휴대폰을 둘 자리 추천에 씁니다.'),
        ),
        h(
          'section',
          { class: 'step' },
          h('div', { class: 'step-head' }, h('span', { class: 'step-no' }, '3'), h('span', {}, '시험 전 준비 시간')),
          minutes,
          h('p', { class: 'hint' }, '켜 둔 감독관 장비·교실 설비처럼 시험 전부터 있던 신호를 익히는 시간입니다. 수험생 휴대폰을 걷은 뒤, 시험 시작 전에 켜 주세요.'),
        ),
        ui.problems.length > 0 ? h('ul', { class: 'problems' }, ui.problems.map((p) => h('li', {}, p))) : null,
        h(
          'div',
          { class: 'actions' },
          developer()
            ? h('button', { type: 'button', class: 'button ghost', onclick: () => bridge.chooseConfig() }, '개발자: 설정 파일로 실행…')
            : null,
          h('button', { type: 'button', class: 'button primary', disabled: ui.saving, onclick: submit }, ui.saving ? '넘어가는 중…' : '다음: 교실 보정'),
        ),
      ),
    );
  }

  // ---------------------------------------------------------------- 교실 보정

  const CALIBRATION_STEPS = [
    { stages: ['listing', 'failed'], label: '휴대폰·핫스팟 고르기' },
    { stages: ['ready_for_far', 'far'], label: '교실 끝에서 재기' },
    { stages: ['done'], label: '저장' },
  ];

  // 보안 정책(CSP)이 style 속성을 막으므로 막대는 기본 progress 요소로 그린다.
  function progressBar(p) {
    return h('progress', { class: 'progress', max: String(Math.max(1, p.total_secs)), value: String(Math.min(p.elapsed_secs, p.total_secs)) });
  }

  /** 고르는 목록 한 칸(블루투스 또는 와이파이). */
  function candidateList(title, hint, radio, list, problem) {
    const chosen = ui.calSel[radio];
    const rows =
      list.length > 0
        ? h(
            'ul',
            { class: 'cal-list', role: 'listbox', 'aria-label': title },
            list.map((c, index) =>
              h(
                'li',
                {},
                h(
                  'button',
                  {
                    type: 'button',
                    role: 'option',
                    'aria-selected': chosen === c.id ? 'true' : 'false',
                    class: `cal-row${chosen === c.id ? ' selected' : ''}`,
                    onclick: () => {
                      ui.calSel[radio] = c.id;
                      render();
                    },
                  },
                  h('span', { class: 'cal-row-name' }, c.label),
                  c.detail ? h('span', { class: 'cal-row-detail' }, c.detail) : null,
                  index === 0 ? h('span', { class: 'pill ok' }, '가장 가까움') : null,
                  h('span', { class: 'cal-row-dbm' }, `${c.rssi}dBm`),
                ),
              ),
            ),
          )
        : h('p', { class: 'quiet cal-empty' }, problem ? '' : '아직 잡힌 것이 없습니다. 잠시 기다려 주세요.');
    return h(
      'section',
      { class: 'cal-column' },
      h('h3', {}, title),
      h('p', { class: 'hint' }, hint),
      problem
        ? h(
            'div',
            { class: 'banner problem inline' },
            icon('alert'),
            h('span', {}, problem),
            radio === 'wifi' ? h('button', { type: 'button', class: 'button small', onclick: () => bridge.calibrationCommand('open_location_settings') }, '위치 설정 열기') : null,
          )
        : null,
      rows,
    );
  }

  function chosenSummary(p) {
    const card = (title, c) =>
      h('div', { class: 'cal-chosen' }, h('span', { class: 'quiet' }, title), h('strong', {}, c ? c.label : '—'), c && c.detail ? h('span', { class: 'quiet' }, c.detail) : null);
    return h('div', { class: 'cal-chosen-row' }, card('내 휴대폰(블루투스)', p.selected_bluetooth), card('내 핫스팟(와이파이)', p.selected_wifi));
  }

  function calibrationScreen() {
    const c = ui.state.calibration;
    const p = c.progress;
    const stage = p ? p.stage : 'starting';
    const command = (type) => () => bridge.calibrationCommand(type);
    const restart = () => {
      ui.calSel = { bluetooth: null, wifi: null };
      bridge.calibrationCommand('calibrate_restart');
    };
    const steps = h(
      'ol',
      { class: 'cal-steps' },
      CALIBRATION_STEPS.map((step, i) =>
        h('li', { class: step.stages.includes(stage) ? 'active' : '' }, h('span', { class: 'step-no' }, String(i + 1)), step.label),
      ),
    );
    let body;
    if (!p) {
      body = [h('div', { class: 'spinner' }), h('p', { class: 'cal-message' }, '블루투스와 와이파이를 켜는 중입니다…')];
    } else if (stage === 'listing' || stage === 'failed') {
      const ready = ui.calSel.bluetooth && ui.calSel.wifi;
      body = [
        stage === 'failed' ? h('div', { class: 'banner problem inline' }, icon('alert'), h('span', {}, p.message)) : h('p', { class: 'cal-message' }, p.message),
        h(
          'div',
          { class: 'cal-columns' },
          candidateList('블루투스 기기 — 내 휴대폰 고르기', '휴대폰을 노트북 가까이 대면 맨 위로 올라옵니다.', 'bluetooth', p.bluetooth, c.bluetooth_problem),
          candidateList('와이파이 — 내 핫스팟 고르기', '휴대폰의 개인 핫스팟 이름을 고르세요. 목록은 몇 초마다 바뀝니다.', 'wifi', p.wifi, c.wifi_problem),
        ),
        h(
          'button',
          {
            type: 'button',
            class: 'button primary',
            disabled: !ready,
            onclick: () => bridge.calibrationSelect(ui.calSel.bluetooth, ui.calSel.wifi),
          },
          ready ? '이 두 개로 재기' : '휴대폰과 핫스팟을 하나씩 골라 주세요',
        ),
      ];
    } else if (stage === 'ready_for_far') {
      body = [
        chosenSummary(p),
        h('p', { class: 'cal-message' }, p.message),
        spotAdvice(),
        h(
          'div',
          { class: 'actions' },
          h('button', { type: 'button', class: 'button ghost', onclick: restart }, '다시 고르기'),
          h('button', { type: 'button', class: 'button primary', onclick: command('calibrate_far') }, '교실 끝에 두었어요'),
        ),
      ];
    } else if (stage === 'far') {
      body = [
        chosenSummary(p),
        h('p', { class: 'cal-message' }, p.message),
        progressBar(p),
        h('p', { class: 'quiet' }, `${p.elapsed_secs}/${p.total_secs}초 이상 · 블루투스 ${Math.min(p.bluetooth_samples, p.needed_samples)}/${p.needed_samples} · 와이파이 ${Math.min(p.wifi_samples, p.needed_samples)}/${p.needed_samples}`),
      ];
    } else {
      body = [
        chosenSummary(p),
        h(
          'div',
          { class: 'cal-results' },
          h('div', { class: 'cal-result' }, h('span', { class: 'quiet' }, '교실 끝 블루투스 세기'), h('strong', {}, `${p.bluetooth_dbm}dBm`)),
          h('div', { class: 'cal-result' }, h('span', { class: 'quiet' }, '교실 끝 와이파이 세기'), h('strong', {}, `${p.wifi_dbm}dBm`)),
        ),
        h('p', { class: 'cal-message' }, p.message),
        h(
          'div',
          { class: 'actions' },
          h('button', { type: 'button', class: 'button ghost', onclick: restart }, '다시 재기'),
          h('button', { type: 'button', class: 'button primary', onclick: () => bridge.finishCalibration(true) }, '저장하고 감지 다시 시작'),
        ),
      ];
    }
    return shell(
      h(
        'div',
        { class: 'onboarding-card calibration' },
        cardHead('교실 보정', '1–2분 · 감지는 잠시 멈춥니다'),
        h('p', { class: 'lead' }, '내 휴대폰 하나로 이 교실의 크기를 잽니다. 휴대폰의 블루투스와 핫스팟이 교실 끝에서 이 노트북에 얼마나 세게 잡히는지 재 두면, 새로 나타난 기기가 교실 안인지 밖인지 가리는 데 참고합니다.'),
        steps,
        c.ended ? h('div', { class: 'banner problem inline' }, icon('alert'), h('span', {}, '보정 프로그램이 멈췄습니다. 그만두고 다시 시작해 주세요.')) : null,
        ui.state.error ? h('div', { class: 'banner problem inline' }, icon('alert'), h('span', {}, ui.state.error)) : null,
        h('section', { class: 'cal-body' }, body),
        h(
          'div',
          { class: 'cal-footer' },
          h('p', { class: 'hint' }, '목록의 기기 이름은 고르는 데만 쓰고 저장하지 않습니다. 보정하는 동안에는 감지가 멈추고, 끝나면 준비 시간부터 다시 시작합니다.'),
          h(
            'button',
            {
              type: 'button',
              class: 'button ghost small',
              onclick: () => {
                ui.calSel = { bluetooth: null, wifi: null };
                bridge.finishCalibration(false);
              },
            },
            '그만두기',
          ),
        ),
      ),
    );
  }

  /** 처음 설정·교실 보정 화면의 틀: 앱 바(작업 단추 없음) · 가운데 판 · 상태 표시줄. */
  function shell(card) {
    return h(
      'div',
      { class: 'screen' },
      appBar(null, { toolbar: false }),
      h('main', { class: 'onboarding' }, card),
      developer() ? devPanel() : null,
      statusBar(null),
    );
  }

  /** 판 머리: 제목과 짧은 설명. */
  function cardHead(title, sub) {
    return h('div', { class: 'card-head' }, h('h1', {}, title), h('span', { class: 'card-sub' }, sub));
  }

  // ---------------------------------------------------------------- 감독관 화면

  /** 신호등 점. 무게는 core가 정한 값(severity)이다. */
  function signal(severity) {
    return h('span', { class: `sig ${V.severityClass(severity)}`, 'aria-hidden': 'true' });
  }

  /** 계기판 칸: 이름표 + 값. */
  function readout(label, value, extra) {
    return h('div', { class: `readout ${extra || ''}` }, h('span', { class: 'readout-label' }, label), h('span', { class: 'readout-value' }, value));
  }

  /** 의심 정도 계기(점 세 칸). */
  function meter(level, text) {
    const filled = V.meterCount(level);
    return h(
      'span',
      { class: 'meter', 'aria-label': text },
      [0, 1, 2].map((i) => h('span', { class: `pip${i < filled ? ' on' : ''}`, 'aria-hidden': 'true' })),
      h('span', { class: 'meter-text' }, text),
    );
  }

  /** 앱 바: 제품 표식·이름·버전, 오른쪽에 작업 단추. */
  function appBar(model, options) {
    const showToolbar = !options || options.toolbar !== false;
    const canCalibrate = bridge && ui.state.setup && !ui.state.devConfigPath;
    const calibrated = canCalibrate && ui.state.setup.boundaryDbm !== undefined && ui.state.setup.boundaryDbm !== null;
    return h(
      'header',
      { class: 'appbar' },
      h(
        'div',
        { class: 'app-id' },
        h('span', { class: 'brandmark', 'aria-hidden': 'true' }, h('span', { class: 'brandmark-dot' })),
        h('span', { class: 'app-title' }, 'Tacet'),
        h('span', { class: 'app-tagline' }, '시험장 무선 기기 탐지기'),
        bridge && ui.state.version ? h('span', { class: 'version' }, `v${ui.state.version}`) : null,
        developer() ? h('span', { class: 'dev-tag' }, '개발자 모드') : null,
      ),
      showToolbar
        ? h(
        'nav',
        { class: 'toolbar', 'aria-label': '작업' },
        bridge && model && model.can_save_report ? h('button', { type: 'button', class: 'button ghost small', onclick: () => send('save_report') }, '보고서 저장') : null,
        canCalibrate ? h('button', { type: 'button', class: 'button small', onclick: startCalibration }, calibrated ? '교실 보정 다시 하기' : '교실 보정') : null,
        bridge
          ? h(
              'details',
              // 열림 상태는 ui.menuOpen이 정한다 — 모델이 올 때마다 다시 그려도 사용자가 닫기 전에는 열려 있다.
              { class: 'menu', open: ui.menuOpen },
              h(
                'summary',
                {
                  class: 'button ghost small',
                  'aria-expanded': ui.menuOpen ? 'true' : 'false',
                  onclick: (e) => {
                    e.preventDefault();
                    ui.menuOpen = !ui.menuOpen;
                    render();
                  },
                },
                '설정',
              ),
              h(
                'div',
                { class: 'menu-list', role: 'menu' },
                h(
                  'button',
                  {
                    type: 'button',
                    role: 'menuitem',
                    onclick: () => {
                      closeMenu();
                      if (window.confirm(RESET_CONFIRM)) bridge.resetAll();
                    },
                  },
                  h('span', {}, '설정 초기화'),
                  h('span', { class: 'menu-hint' }, '교실·자리·보정을 지우고 처음 설정으로'),
                ),
              ),
            )
          : null,
      )
        : null,
    );
  }

  function closeMenu() {
    if (!ui.menuOpen) return;
    ui.menuOpen = false;
    render();
  }

  // 설정 메뉴는 메뉴 밖을 누르거나 Esc를 눌러야 닫힌다(설정 단추를 다시 눌러도 닫힌다).
  document.addEventListener('click', (e) => {
    if (ui.menuOpen && !(e.target instanceof Element && e.target.closest('.menu'))) closeMenu();
  });
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') closeMenu();
  });

  /** 상태 줄: 교실·감지·확인 필요·장치 상태. */
  function statusStrip(model) {
    const view = model ? model.view : null;
    const phase = V.phaseBadge(view && view.phase);
    const status = V.health(view, developer());
    const alerts = view ? V.visibleAlerts(view, ui.room).length : 0;
    const place = bridge && ui.state.placeLabel && !ui.state.devConfigPath ? ui.state.placeLabel : model ? model.mode_label : '—';
    return h(
      'section',
      { class: 'status-strip', 'aria-label': '지금 상태' },
      readout('교실', place),
      readout('감지', h('span', { class: 'inline' }, signal(phase.watching ? 'ok' : 'warning'), phase.text)),
      readout('확인 필요', h('span', { class: `figure${alerts > 0 ? ' hot' : ''}` }, V.count(alerts)), 'numeric'),
      readout('장치', h('span', { class: 'inline' }, signal(status.severity), status.text)),
    );
  }

  function alertRow(alert, index) {
    const e = alert.explain;
    const key = alert.text || e.title;
    const technical = developer()
      ? h(
          'details',
          {
            class: 'technical',
            open: ui.open.has(key),
            ontoggle: (ev) => (ev.target.open ? ui.open.add(key) : ui.open.delete(key)),
          },
          h('summary', {}, '기술 정보 (개발자)'),
          h('ul', {}, e.technical.map((t) => h('li', {}, t))),
        )
      : null;
    return h(
      'article',
      { class: `alert-row level-${V.levelClass(alert.level)}` },
      h('div', { class: 'alert-index' }, V.index(index)),
      h(
        'div',
        { class: 'alert-main' },
        h('div', { class: 'alert-top' }, h('span', { class: 'check-label' }, V.CHECK_NEEDED), meter(alert.level, e.level), h('span', { class: 'audience' }, `알린 곳 · ${alert.audience}`)),
        h('h3', { class: 'alert-title' }, e.title),
        h('dl', { class: 'facts' }, h('dt', {}, '위치'), h('dd', {}, h('strong', {}, e.place)), h('dt', {}, '확실도'), h('dd', {}, e.certainty)),
        h('div', { class: 'why' }, h('h4', {}, '왜 알렸나요'), h('ul', {}, e.reasons.map((r) => h('li', {}, r)))),
        e.caveats.length > 0 ? h('ul', { class: 'caveats' }, e.caveats.map((c) => h('li', {}, c))) : null,
        h('p', { class: 'guide' }, h('span', { class: 'guide-label' }, '할 일'), e.guide),
        technical,
      ),
    );
  }

  function emptyAlerts(view) {
    if (!V.hasData(view)) {
      return h('div', { class: 'empty' }, h('span', { class: 'big-figure' }, '—'), h('h3', {}, '아직 받은 화면이 없습니다'), h('p', {}, '연결되면 알림이 여기에 나옵니다. 오른쪽의 연결 상태를 확인해 주세요.'));
    }
    const phase = view && view.phase;
    if (phase && !phase.watching && phase.remaining_secs > 0) {
      return h(
        'div',
        { class: 'empty preparing' },
        h('span', { class: 'big-figure' }, V.clock(phase.remaining_secs)),
        h('h3', {}, '준비 시간 남음'),
        h('p', {}, phase.detail),
      );
    }
    return h(
      'div',
      { class: 'empty' },
      h('span', { class: 'big-figure' }, '00'),
      h('h3', {}, '확인할 알림이 없습니다'),
      h('p', {}, phase && phase.detail ? phase.detail : '새로 나타난 무선 신호를 지켜보고 있습니다.'),
    );
  }

  function roomPicker(view) {
    if (!view || !view.rooms || view.rooms.length < 2) return null;
    return h(
      'label',
      { class: 'room-picker' },
      '보일 교실 ',
      h(
        'select',
        {
          onchange: (e) => {
            ui.room = e.target.value || null;
            writeStored('tacet.room', ui.room);
            render();
          },
        },
        h('option', { value: '' }, '모든 교실'),
        view.rooms.map((r) => h('option', { value: r, selected: ui.room === r }, r)),
      ),
    );
  }

  function noticeRow(notice) {
    let fix = null;
    if (bridge && notice.fix === 'open_location_settings') {
      fix = h('button', { type: 'button', class: 'button small', onclick: () => send('open_location_settings') }, '위치 설정 열기');
    } else if (bridge && notice.fix === 'calibrate' && ui.state.setup && !ui.state.devConfigPath) {
      fix = h('button', { type: 'button', class: 'button small primary', onclick: startCalibration }, '교실 보정하기');
    }
    return h(
      'article',
      { class: `notice-row ${V.severityClass(notice.severity)}` },
      signal(notice.severity),
      h(
        'div',
        { class: 'notice-main' },
        h('strong', { class: 'notice-problem' }, notice.problem),
        h('dl', { class: 'facts' }, h('dt', {}, '어디'), h('dd', {}, notice.place), h('dt', {}, '영향'), h('dd', {}, notice.effect), h('dt', {}, '할 일'), h('dd', {}, notice.action)),
        fix,
        developer() ? h('p', { class: 'tech-line' }, notice.technical) : null,
      ),
    );
  }

  function sensorTable(sensor) {
    const collectors = sensor.collectors.filter((c) => developer() || c.severity !== 'info');
    return h(
      'div',
      { class: 'spec' },
      h('div', { class: 'spec-head' }, h('strong', {}, sensor.place), h('span', { class: 'inline spec-summary' }, signal(sensor.severity), sensor.summary)),
      collectors.length > 0
        ? h(
            'table',
            {},
            h(
              'tbody',
              {},
              collectors.map((c) =>
                h(
                  'tr',
                  {},
                  h('th', { scope: 'row' }, c.name),
                  h('td', {}, h('span', { class: 'inline' }, signal(c.severity), developer() ? `${c.state} · ${c.technical}` : c.state)),
                ),
              ),
            ),
          )
        : null,
    );
  }

  function statusPanel(model) {
    const view = model.view;
    const notices = V.visibleNotices(view, developer());
    return h(
      'aside',
      { class: 'side' },
      model.connection ? h('div', { class: 'connection' }, signal(model.connection.severity), h('span', {}, model.connection.text)) : null,
      V.hasData(view)
        ? h(
            'section',
            { class: 'panel' },
            h('div', { class: 'panel-head' }, h('h2', {}, '확인할 안내'), h('span', { class: 'count' }, `${notices.length}건`)),
            notices.length > 0 ? notices.map(noticeRow) : h('p', { class: 'quiet pad' }, '감지 장치가 모두 정상입니다.'),
          )
        : null,
      view.sensors.length > 0
        ? h('section', { class: 'panel' }, h('div', { class: 'panel-head' }, h('h2', {}, view.sensors.length > 1 ? '교실 노트북 상태' : '이 노트북 상태')), view.sensors.map(sensorTable))
        : null,
    );
  }

  const CALIBRATION_CONFIRM =
    '교실 보정을 시작할까요?\n\n보정하는 동안 감지가 멈추고, 끝나면 준비 시간(주변 신호 익히기)부터 다시 시작합니다. 그때 켜져 있는 기기는 원래 있던 기기로 보고 알림에서 빠집니다.\n\n시험 중에는 누르지 말고, 수험생 휴대폰을 걷기 전에 보정해 주세요.';

  /** 보정 시작. 감지가 돌고 있으면 확인을 받는다 — 보정 뒤 기준선을 새로 익히므로 시험 중에 누르면 탐지에 빈틈이 생긴다. */
  function startCalibration() {
    const running = ui.model && V.hasData(ui.model.view);
    if (running && !window.confirm(CALIBRATION_CONFIRM)) return;
    ui.calSel = { bluetooth: null, wifi: null };
    bridge.startCalibration();
  }

  const RESET_CONFIRM =
    '설정을 초기화할까요?\n\n교실·노트북 자리·교실 보정·준비 시간(개발자가 고른 설정 파일 포함)을 지우고 처음 설정 화면으로 돌아갑니다. 지금 감지는 멈춥니다.\n\n예외 기기 목록과 저장한 보고서는 지우지 않습니다.';

  /** 바닥 상태 표시줄: 처리 위치와 안내, 마지막 작업 결과. */
  function statusBar(model) {
    return h(
      'footer',
      { class: 'statusbar' },
      h('span', { class: 'inline' }, signal('ok'), '내부망 전용 · 이 기기 안에서 처리'),
      h('span', { class: 'statusbar-sep', 'aria-hidden': 'true' }),
      h('span', { class: 'disclaimer' }, '알림은 "확인 필요"입니다. 자동 판단은 참고용이며 특정 수험생을 가리키지 않습니다. 최종 확인은 감독관이 합니다.'),
      model && model.notice ? h('span', { class: 'notice-line' }, model.notice) : null,
    );
  }

  function devPanel() {
    const model = ui.model;
    const s = ui.state;
    return h(
      'section',
      { class: 'dev-panel' },
      h('div', { class: 'dev-head' }, icon('wrench'), h('strong', {}, '개발자 모드'), h('span', { class: 'quiet' }, 'Ctrl/⌘ + Shift + D로 닫기')),
      bridge
        ? h(
            'div',
            { class: 'dev-row' },
            h('span', {}, '실행 중: ', h('strong', {}, model ? model.mode_label : '없음')),
            h('span', { class: 'quiet' }, s.devConfigPath ? `설정 파일 ${s.devConfigPath}` : '처음 설정으로 만든 감독관 모드'),
          )
        : null,
      bridge
        ? h(
            'div',
            { class: 'dev-actions' },
            h('button', { type: 'button', class: 'button small', onclick: () => bridge.chooseConfig() }, '설정 파일로 실행 (단독·센서·서버·본부 화면)…'),
            s.devConfigPath ? h('button', { type: 'button', class: 'button small ghost', onclick: () => bridge.clearConfig() }, '감독관 기본 모드로 돌아가기') : null,
            h('button', { type: 'button', class: 'button small ghost', onclick: () => bridge.openWhitelist() }, '예외 기기 목록 열기'),
            h('button', { type: 'button', class: 'button small ghost', onclick: () => bridge.restart() }, '다시 시작'),
            h('button', { type: 'button', class: 'button small ghost', onclick: () => bridge.openDataFolder() }, '앱 데이터 폴더 열기'),
          )
        : null,
      model && model.header.length > 0 ? h('div', {}, h('h4', {}, '서버·연결 정보'), h('ul', { class: 'mono' }, model.header.map((l) => h('li', {}, l)))) : null,
      model && model.log.length > 0 ? h('div', {}, h('h4', {}, '알림 기록'), h('ul', { class: 'mono' }, model.log.slice(-10).reverse().map((l) => h('li', {}, l)))) : null,
      bridge && s.stderr.length > 0 ? h('div', {}, h('h4', {}, '백엔드 오류 출력'), h('ul', { class: 'mono' }, s.stderr.map((l) => h('li', {}, l)))) : null,
    );
  }

  function errorBanner() {
    const message = bridge ? ui.state && ui.state.error : ui.webError;
    if (!message) return null;
    return h('div', { class: 'banner problem' }, icon('alert'), h('span', {}, message));
  }

  function mainScreen() {
    const model = ui.model;
    if (!model) {
      return h(
        'div',
        { class: 'screen' },
        appBar(null),
        statusStrip(null),
        errorBanner(),
        h('main', { class: 'loading' }, ui.state && ui.state.error ? null : h('div', { class: 'spinner' }), h('p', {}, ui.state && ui.state.error ? '감지를 시작하지 못했습니다.' : '감지를 시작하는 중…')),
        statusBar(null),
        developer() ? devPanel() : null,
      );
    }
    const view = model.view;
    const alerts = V.visibleAlerts(view, ui.room);
    return h(
      'div',
      { class: 'screen' },
      appBar(model),
      statusStrip(model),
      errorBanner(),
      h(
        'main',
        { class: 'content' },
        h(
          'section',
          { class: 'alerts' },
          h('div', { class: 'panel-head' }, h('h2', {}, '확인 필요 알림'), roomPicker(view), h('span', { class: 'count' }, `${alerts.length}건`)),
          alerts.length > 0 ? alerts.map(alertRow) : emptyAlerts(view),
        ),
        statusPanel(model),
      ),
      developer() ? devPanel() : null,
      statusBar(model),
    );
  }

  // ---------------------------------------------------------------- 그리기와 데이터

  function render() {
    // 다시 그려도 입력 칸의 초점과 커서를 지킨다.
    const focused = document.activeElement && document.activeElement.id;
    const caret = focused === 'room' ? document.activeElement.selectionStart : null;
    let screen;
    if (bridge && !ui.state) screen = h('main', { class: 'loading' }, h('div', { class: 'spinner' }));
    else if (bridge && ui.state.calibration) screen = calibrationScreen();
    else if (bridge && !ui.state.setup && !ui.state.devConfigPath) screen = onboarding();
    else screen = mainScreen();
    root.replaceChildren(screen);
    if (focused) {
      const el = document.getElementById(focused);
      if (el) {
        el.focus();
        if (caret !== null && el.setSelectionRange) el.setSelectionRange(caret, caret);
      }
    }
  }

  function startDesktop() {
    bridge.on('model', (model) => {
      ui.model = model;
      render();
    });
    bridge.on('state', (state) => {
      ui.state = state;
      ui.model = state.model;
      render();
    });
    bridge.getState().then((state) => {
      ui.state = state;
      ui.model = state.model;
      render();
    });
  }

  function startWeb() {
    // 웹 대시보드: 서버가 보내는 건물 화면(SSE)을 그린다. 개발자 표시는 이 브라우저에만 저장한다.
    document.addEventListener('keydown', (e) => {
      if (e.shiftKey && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'd') {
        ui.webDeveloper = !ui.webDeveloper;
        writeStored('tacet.developer', ui.webDeveloper ? 'on' : null);
        render();
      }
    });
    const events = new EventSource('events');
    events.onmessage = (e) => {
      try {
        const view = JSON.parse(e.data);
        ui.webError = null;
        ui.model = { mode: 'server', mode_label: '시험본부 대시보드', view, header: [], connection: null, log: [], notice: null, can_save_report: false };
      } catch {
        return;
      }
      render();
    };
    events.onerror = () => {
      ui.webError = '서버와 연결이 끊겼습니다 — 다시 연결하는 중입니다.';
      render();
    };
    render();
  }

  if (bridge) startDesktop();
  else startWeb();
})();
