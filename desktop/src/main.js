// Electron 메인 프로세스: 창을 띄우고, Rust 백엔드를 자식 프로세스로 돌리고, 둘 사이를 잇는다.
//
// 감독관 기본 흐름: 처음 켜면 교실과 노트북 자리를 고르고, 이 노트북 혼자 감지한다(단독 모드). 서버·센서·본부 화면
// 같은 건물 단위 구성은 개발자 모드(Ctrl/Cmd+Shift+D 또는 `--dev`)에서 설정 파일을 골라 쓴다.
//
// 창을 닫아도 감지는 계속된다: 창만 숨기고 백엔드는 그대로 둔다. 끄는 것은 메뉴 막대(Windows는 작업 표시줄 알림 영역)
// 아이콘의 '종료'나 Cmd+Q다. 창이 안 보이는 동안 새 알림·감지 문제는 운영체제 알림으로 알린다(src/notify.js).
//
// 화면은 판단하지 않는다(AGENTS.md §2 규칙 11). 판정·문구는 백엔드(core)가 만들고, 여기는 전달만 한다. 기술 정보(식별자
// 포함)는 개발자 모드일 때만 화면에 넘긴다(AGENTS.md §3.3).

'use strict';

const { app, BrowserWindow, Menu, Notification, Tray, dialog, ipcMain, nativeImage, nativeTheme, shell } = require('electron');
const { spawn } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const backend = require('./backend');
const demo = require('./demo');
const notify = require('./notify');

const argv = process.argv.slice(1);
const hasFlag = (name) => argv.includes(name);
const optionValue = (name) => {
  const i = argv.indexOf(name);
  return i >= 0 ? argv[i + 1] : undefined;
};

const DEMO = hasFlag('--demo');
// 창 캡처는 시연 모델일 때만 한다 — 실제 세션 화면(식별자가 보일 수 있다)을 파일로 남기지 않는다.
const SCREENSHOT = DEMO ? optionValue('--screenshot') : undefined;
const SCREEN = optionValue('--screen');
const THEME = optionValue('--theme');

const state = {
  setup: null,
  // 개발자 모드는 저장하지 않는다 — 켠 노트북이 다음 시험에서도 기술 정보를 보이지 않게, 실행할 때마다 다시 켠다.
  developer: hasFlag('--dev'),
  devConfigPath: null,
  model: null,
  error: null,
  stderr: [],
  /** 교실 보정 중이면 진행 상황·블루투스 문제·보정 프로그램 종료 여부를 담고, 아니면 null. */
  calibration: null,
  /** 이번 감지 세션에 운영체제 알림으로 알린 것(열쇠마다 가장 많이 보인 개수). 백엔드를 새로 띄우면 비운다. */
  notified: { alerts: {}, problems: {} },
};

/** 참이면 정말 끝내는 중이다 — 창 닫기를 숨기기로 바꾸지 않는다. */
let quitting = false;
/** 창을 닫아도 감지가 계속된다는 안내를 이번 실행에 이미 했는지. */
let backgroundHintShown = false;
/** 메뉴 막대·작업 표시줄 아이콘. 가비지 수집되면 사라지므로 붙잡아 둔다. */
let tray = null;

let win = null;
/** 지금 돌고 있는 백엔드. 멈추는 중인 옛 백엔드는 여기 없다. */
let child = null;
/** 교실 보정 중인 백엔드(`tacet calibrate`). */
let calibrator = null;
/** 멈추는 중인 옛 보정 백엔드가 끝나기를 기다리는 약속. 감지 백엔드와 보정 백엔드는 블루투스를 함께 쓰지 않는다. */
let calibrationStopped = Promise.resolve();
/** 보정을 띄우는 중이면 참(두 번 눌러 둘이 뜨는 것을 막는다). */
let calibrationStarting = false;
/** 멈추는 중인 옛 백엔드가 끝나기를 기다리는 약속. 새 백엔드는 이것이 끝난 뒤 띄운다(두 개가 겹쳐 돌지 않게). */
let stopped = Promise.resolve();

function userFile(name) {
  return path.join(app.getPath('userData'), name);
}

/** 이 사용자만 읽고 쓰게 쓴다. 이미 있던 파일도 권한을 다시 좁힌다(Windows는 권한 비트를 무시한다). */
function writePrivate(file, text) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, text, { mode: 0o600 });
  try {
    fs.chmodSync(file, 0o600);
  } catch {
    // 권한을 바꿀 수 없는 파일 시스템이다.
  }
}

function loadSettings() {
  try {
    const saved = JSON.parse(fs.readFileSync(userFile('settings.json'), 'utf8'));
    // 예전 3×2 자리 설정은 5×5 칸으로 옮긴다 — 보정값을 잃지 않게.
    const setup = saved && backend.migrateSetup(saved.setup);
    if (setup && backend.setupProblems(setup).length === 0) state.setup = setup;
    if (typeof saved.devConfigPath === 'string') state.devConfigPath = saved.devConfigPath;
  } catch {
    // 처음 실행이거나 읽을 수 없다 — 처음 설정부터 한다.
  }
}

function saveSettings() {
  writePrivate(userFile('settings.json'), JSON.stringify({ setup: state.setup, devConfigPath: state.devConfigPath }, null, 2));
}

function send(channel, payload) {
  if (win && !win.isDestroyed()) win.webContents.send(channel, payload);
}

function shownModel() {
  return backend.forAudience(state.model, state.developer);
}

function publicState() {
  return {
    demo: DEMO,
    version: app.getVersion(),
    screen: SCREEN || null,
    developer: state.developer,
    setup: state.setup,
    placeLabel: state.setup ? backend.placeLabel(state.setup) : null,
    // 교실 보정에서 휴대폰을 둘 자리 추천(이 노트북에서 가장 먼 구석).
    calibrationSpot: state.setup ? backend.calibrationSpot(state.setup) : null,
    devConfigPath: state.devConfigPath,
    calibration: state.calibration,
    model: shownModel(),
    error: state.error,
    stderr: state.developer ? state.stderr.slice(-20) : [],
    options: {
      grid: backend.GRID,
      cellLabels: Array.from({ length: backend.GRID }, (_, row) =>
        Array.from({ length: backend.GRID }, (_, column) => backend.cellLabel(row, column)),
      ),
      rows: backend.ROWS,
      columns: backend.COLUMNS,
      baselineMinutes: backend.BASELINE_MINUTES,
      defaultBaselineMinutes: backend.DEFAULT_BASELINE_MINUTES,
    },
  };
}

function fail(message) {
  state.error = message;
  send('state', publicState());
}

/** 백엔드에 한 줄을 쓴다. 이미 끝났으면 쓰지 않는다(끝난 프로세스에 쓰면 파이프 오류가 난다). */
function writeLine(proc, line) {
  if (!proc || proc.exitCode !== null || proc.signalCode !== null || !proc.stdin.writable) return false;
  proc.stdin.write(line);
  return true;
}

function stopBackend() {
  if (!child) return;
  const old = child;
  child = null;
  old.stopping = true;
  stopped = new Promise((resolve) => {
    if (old.exitCode !== null || old.signalCode !== null) {
      resolve();
      return;
    }
    old.once('exit', () => resolve());
    writeLine(old, backend.commandLine('shutdown'));
    setTimeout(() => {
      if (old.exitCode === null && old.signalCode === null) old.kill();
    }, 1500);
  });
}

/** 쓸 백엔드 실행 파일. 없으면 화면에 알리고 null. */
function backendBinary() {
  const candidates = backend.backendCandidates({
    packaged: app.isPackaged,
    resourcesPath: process.resourcesPath,
    appRoot: app.getAppPath(),
    platform: process.platform,
    env: process.env,
  });
  const bin = backend.pickBackend(candidates, (candidate) => {
    try {
      return fs.statSync(candidate).mtimeMs;
    } catch {
      return null;
    }
  });
  if (!bin) fail(`감지 프로그램을 찾지 못했습니다. 앱을 다시 설치해 주세요. (찾은 곳: ${candidates.join(', ')})`);
  return bin;
}

async function startBackend(configPath) {
  stopBackend();
  await Promise.all([stopped, calibrationStopped]);
  // 기다리는 동안 보정이 시작됐으면 감지를 띄우지 않는다 — 블루투스를 함께 쓰지 않게(보정이 끝나면 endCalibration이 다시 부른다).
  if (state.calibration) return;
  state.model = null;
  state.error = null;
  state.stderr = [];
  const bin = backendBinary();
  if (!bin) return;
  state.notified = { alerts: {}, problems: {} };
  const proc = spawn(bin, ['--config', configPath], { stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true });
  proc.stopping = false;
  child = proc;
  // 끝난 백엔드에 쓰다 난 파이프 오류는 비동기로 온다. 여기서 받아야 메인 프로세스가 죽지 않는다.
  proc.stdin.on('error', () => {});
  let buffer = '';
  proc.stdout.setEncoding('utf8');
  proc.stdout.on('data', (chunk) => {
    if (child !== proc) return;
    const { messages, rest } = backend.splitMessages(buffer, chunk);
    buffer = rest;
    for (const message of messages) {
      if (message.type === 'model') {
        state.model = message.model;
        send('model', shownModel());
        notifyNew();
      } else if (message.type === 'error') {
        fail(message.message);
      }
    }
  });
  proc.stderr.setEncoding('utf8');
  proc.stderr.on('data', (chunk) => {
    if (child !== proc) return;
    state.stderr.push(...chunk.split('\n').filter(Boolean));
    state.stderr = state.stderr.slice(-50);
  });
  proc.on('error', (e) => {
    if (child === proc) fail(`감지 프로그램을 시작하지 못했습니다: ${e.message}`);
  });
  proc.on('exit', (code) => {
    if (child !== proc) return;
    child = null;
    if (!proc.stopping && code !== 0 && !state.error) {
      fail(`감지 프로그램이 멈췄습니다(코드 ${code}). 앱을 다시 시작해 주세요.`);
      if (!windowInView()) osNotify({ title: '감지가 멈췄습니다', body: '앱을 열어 다시 시작해 주세요.' });
    }
  });
}

function stopCalibration() {
  if (!calibrator) return;
  const old = calibrator;
  calibrator = null;
  old.stopping = true;
  calibrationStopped = new Promise((resolve) => {
    if (old.exitCode !== null || old.signalCode !== null) {
      resolve();
      return;
    }
    old.once('exit', () => resolve());
    writeLine(old, backend.commandLine('shutdown'));
    setTimeout(() => {
      if (old.exitCode === null && old.signalCode === null) old.kill();
    }, 1500);
  });
}

/**
 * 교실 보정을 시작한다(docs/rules/L-room-calibration.md). 감지를 멈추고 블루투스만 켠 보정 백엔드를 띄운다. 감독관 기본
 * 모드(처음 설정으로 만든 단독 모드)에서만 한다 — 건물 단위 구성의 보정은 설치 담당자가 설정 파일에 넣는다.
 */
async function startCalibration() {
  if (DEMO || !state.setup || state.devConfigPath || calibrationStarting) return;
  calibrationStarting = true;
  try {
    state.model = null;
    state.error = null;
    state.calibration = { progress: null, bluetooth_problem: null, wifi_problem: null, ended: false };
    send('state', publicState());
    stopCalibration();
    stopBackend();
    await Promise.all([stopped, calibrationStopped]);
    // 기다리는 동안 그만두기를 눌렀으면 띄우지 않는다.
    if (!state.calibration) return;
    const bin = backendBinary();
    if (!bin) {
      // 보정을 못 하면 감지라도 다시 돌린다 — 감지가 꺼진 채 남지 않게.
      state.calibration = null;
      startFromSettings();
      return;
    }
    spawnCalibrator(bin);
  } finally {
    calibrationStarting = false;
  }
}

function spawnCalibrator(bin) {
  const proc = spawn(bin, ['calibrate'], { stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true });
  proc.stopping = false;
  calibrator = proc;
  proc.stdin.on('error', () => {});
  proc.stderr.resume();
  let buffer = '';
  proc.stdout.setEncoding('utf8');
  proc.stdout.on('data', (chunk) => {
    if (calibrator !== proc) return;
    const { messages, rest } = backend.splitMessages(buffer, chunk);
    buffer = rest;
    for (const message of messages) {
      if (message.type === 'calibration') {
        state.calibration = {
          progress: message.progress,
          bluetooth_problem: message.bluetooth_problem,
          wifi_problem: message.wifi_problem,
          ended: false,
        };
        send('state', publicState());
      } else if (message.type === 'error') {
        fail(message.message);
      }
    }
  });
  proc.on('error', (e) => {
    if (calibrator === proc) fail(`교실 보정을 시작하지 못했습니다: ${e.message}`);
  });
  proc.on('exit', () => {
    if (calibrator !== proc) return;
    calibrator = null;
    if (!proc.stopping && state.calibration) {
      state.calibration = { ...state.calibration, ended: true };
      send('state', publicState());
    }
  });
}

/** 보정을 마치거나 그만두고 감지를 다시 시작한다. `save`이면 백엔드가 잰 값을 저장한다(화면이 보낸 숫자는 쓰지 않는다). */
function endCalibration(save) {
  const progress = state.calibration && state.calibration.progress;
  const done = progress && progress.stage === 'done';
  const bluetooth = done ? progress.bluetooth_dbm : null;
  const wifi = done ? progress.wifi_dbm : null;
  // 블루투스·와이파이를 둘 다 잰 경우에만 저장한다(사용자 결정 2026-11-16: 둘 다 필수).
  if (save && state.setup && backend.validBoundary(bluetooth) && backend.validBoundary(wifi)) {
    state.setup = { ...state.setup, boundaryDbm: bluetooth, wifiBoundaryDbm: wifi };
    saveSettings();
  }
  stopCalibration();
  state.calibration = null;
  startFromSettings();
  send('state', publicState());
}

/** 예외 기기 목록(설치 담당자가 적는 TOML 조각). 없으면 빈 문자열. */
function readWhitelist() {
  try {
    return fs.readFileSync(userFile(backend.WHITELIST_FILE), 'utf8');
  } catch {
    return '';
  }
}

/** 지금 설정으로 백엔드를 띄운다: 개발자가 고른 설정 파일이 있으면 그것, 아니면 처음 설정으로 만든 단독 모드. */
function startFromSettings() {
  // 보정 중에는 감지를 띄우지 않는다(블루투스를 함께 쓰지 않게). 보정이 끝나면 endCalibration이 다시 부른다.
  if (DEMO || state.calibration) return;
  if (state.devConfigPath) {
    startBackend(state.devConfigPath);
    return;
  }
  if (!state.setup) {
    stopBackend();
    return;
  }
  const configPath = userFile('standalone.toml');
  writePrivate(configPath, backend.standaloneToml(state.setup, userFile('state'), readWhitelist()));
  startBackend(configPath);
}

function setDeveloper(on) {
  state.developer = Boolean(on);
  send('state', publicState());
}

function registerIpc() {
  ipcMain.handle('state:get', () => publicState());
  ipcMain.handle('setup:save', (_event, setup) => {
    const cleaned = {
      room: typeof setup?.room === 'string' ? setup.room.trim() : '',
      row: setup?.row,
      column: setup?.column,
      baselineMinutes: Number(setup?.baselineMinutes),
    };
    const problems = backend.setupProblems(cleaned);
    if (problems.length > 0) return { ok: false, problems };
    state.setup = cleaned;
    saveSettings();
    send('state', publicState());
    // 자리를 새로 정했으면 보정값이 없다 — 감지보다 교실 보정을 먼저 한다. 보정을 그만두면 endCalibration이 감지를 띄운다.
    // 개발자 설정 파일을 쓰는 중이면 그 파일이 앞서므로(보정도 하지 않는다) 감지를 그대로 다시 띄운다.
    if (state.devConfigPath) startFromSettings();
    else startCalibration();
    return { ok: true, problems: [] };
  });
  ipcMain.handle('setup:reset', () => {
    state.setup = null;
    state.model = null;
    state.calibration = null;
    saveSettings();
    stopCalibration();
    stopBackend();
    send('state', publicState());
    return publicState();
  });
  // 설정 초기화: 처음 설정·교실 보정·개발자 설정 파일 선택과 그것으로 만든 설정 파일을 지운다. 예외 기기 목록(설치 담당자가
  // 적음)과 저장한 보고서는 남긴다 — 실수로 지우면 되살릴 수 없고, 초기화의 목적(교실·자리를 다시 정하기)과 관계없다.
  ipcMain.handle('settings:reset', () => {
    stopCalibration();
    stopBackend();
    state.setup = null;
    state.devConfigPath = null;
    state.calibration = null;
    state.model = null;
    state.error = null;
    try {
      for (const name of ['settings.json', 'standalone.toml']) {
        fs.rmSync(userFile(name), { force: true });
      }
    } catch (e) {
      // 파일을 지우지 못하면(다른 프로그램이 잡고 있음 등) 다시 켰을 때 설정이 되살아난다 — 그렇다고 알린다.
      fail(`설정 파일을 지우지 못했습니다(${e.message}). 앱을 다시 켜면 이전 설정이 돌아올 수 있습니다.`);
    }
    send('state', publicState());
    return publicState();
  });
  ipcMain.handle('backend:command', (_event, type) => {
    const line = backend.commandLine(type);
    return Boolean(line) && writeLine(child, line);
  });
  ipcMain.handle('dev:set', (_event, on) => {
    setDeveloper(on);
    return publicState();
  });
  ipcMain.handle('dev:choose-config', async () => {
    if (!state.developer || !win) return publicState();
    const picked = await dialog.showOpenDialog(win, {
      title: '실행할 설정 파일 고르기',
      filters: [{ name: '설정 파일', extensions: ['toml'] }],
      properties: ['openFile'],
    });
    if (!picked.canceled && picked.filePaths[0]) {
      state.devConfigPath = picked.filePaths[0];
      saveSettings();
      startFromSettings();
    }
    return publicState();
  });
  ipcMain.handle('dev:clear-config', () => {
    if (!state.developer) return publicState();
    state.devConfigPath = null;
    saveSettings();
    startFromSettings();
    return publicState();
  });
  ipcMain.handle('calibration:start', async () => {
    await startCalibration();
    return publicState();
  });
  ipcMain.handle('calibration:command', (_event, type) => {
    if (!['calibrate_far', 'calibrate_restart', 'open_location_settings'].includes(type)) return false;
    return writeLine(calibrator, backend.commandLine(type));
  });
  ipcMain.handle('calibration:select', (_event, bluetooth, wifi) => {
    const line = backend.selectLine(bluetooth, wifi);
    return Boolean(line) && writeLine(calibrator, line);
  });
  ipcMain.handle('calibration:finish', (_event, save) => {
    endCalibration(save === true);
    return publicState();
  });
  ipcMain.handle('dev:open-data-folder', () => (state.developer ? shell.openPath(app.getPath('userData')) : ''));
  ipcMain.handle('dev:open-whitelist', () => {
    if (!state.developer) return '';
    const file = userFile(backend.WHITELIST_FILE);
    if (!fs.existsSync(file)) writePrivate(file, backend.WHITELIST_TEMPLATE);
    return shell.openPath(file);
  });
  ipcMain.handle('dev:restart', () => {
    if (state.developer) startFromSettings();
    return publicState();
  });
}

/** 감독관이 지금 창을 보고 있는지. 보고 있으면 화면에 이미 보이므로 운영체제 알림을 내지 않는다. */
function windowInView() {
  return Boolean(win && !win.isDestroyed() && win.isVisible() && !win.isMinimized() && win.isFocused());
}

function showWindow() {
  if (!win || win.isDestroyed()) {
    createWindow();
    return;
  }
  if (win.isMinimized()) win.restore();
  win.show();
  win.focus();
}

/** 운영체제 알림. 누르면 창을 연다. 알림을 지원하지 않는 환경이면 조용히 넘어간다(창에는 그대로 보인다). */
function osNotify({ title, body }) {
  if (!Notification.isSupported()) return;
  const n = new Notification({ title, body });
  n.on('click', showWindow);
  n.show();
}

/** 새 알림과 감지 문제를 운영체제 알림으로 알린다. 무엇이 새것인지는 열쇠로만 가린다(판정은 core가 이미 했다). */
function notifyNew() {
  if (DEMO || !state.model) return;
  const view = state.model.view;
  // 교실을 정한 감독관 화면이면 그 교실에 보일 알림만, 개발자 설정 파일이면 모든 알림.
  const room = state.devConfigPath || !state.setup ? null : state.setup.room.trim();
  const alerts = notify.freshAlerts(state.notified.alerts, view, room);
  const problems = notify.freshProblems(state.notified.problems, view);
  state.notified = { alerts: alerts.seen, problems: problems.seen };
  if (windowInView()) return;
  for (const a of alerts.found) osNotify(notify.alertNotification(a));
  for (const p of problems.found) osNotify(notify.problemNotification(p));
}

/** 감지가 도는 동안에만 창 닫기를 숨기기로 바꾼다. 처음 설정 중이면 돌고 있는 것이 없으니 그냥 끝낸다. */
function keepsRunning() {
  return !DEMO && !quitting && Boolean(child || calibrator);
}

function createTray() {
  const icon =
    process.platform === 'darwin'
      ? nativeImage.createFromPath(path.join(__dirname, '..', 'assets', 'trayTemplate.png'))
      : nativeImage.createFromPath(path.join(__dirname, '..', 'assets', 'tray.png'));
  if (process.platform === 'darwin') icon.setTemplateImage(true);
  tray = new Tray(icon);
  tray.setToolTip('Tacet — 시험장 무선 기기 탐지기');
  tray.setContextMenu(
    Menu.buildFromTemplate([
      { label: 'Tacet 열기', click: showWindow },
      { type: 'separator' },
      { label: '종료 (감지 멈춤)', click: () => app.quit() },
    ]),
  );
  // Windows는 아이콘을 한 번 누르면 창을 연다(macOS는 메뉴가 열린다).
  tray.on('click', () => {
    if (process.platform !== 'darwin') showWindow();
  });
}

function buildMenu() {
  const template = [
    ...(process.platform === 'darwin' ? [{ role: 'appMenu' }] : []),
    { role: 'editMenu' },
    { role: 'windowMenu' },
  ];
  Menu.setApplicationMenu(Menu.buildFromTemplate(template));
}

function createWindow() {
  win = new BrowserWindow({
    width: 1240,
    height: 820,
    minWidth: 920,
    minHeight: 640,
    title: 'Tacet — 시험장 무선 기기 탐지기',
    backgroundColor: '#f5f6f8',
    show: !SCREENSHOT,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
      spellcheck: false,
    },
  });
  // 화면은 우리 파일만 연다. 다른 주소로 가거나 새 창을 여는 것을 막는다.
  win.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  win.webContents.on('will-navigate', (event) => event.preventDefault());
  // 개발자 모드는 메뉴에 두지 않고 단축키로만 켠다: Ctrl/Cmd + Shift + D.
  win.webContents.on('before-input-event', (_event, input) => {
    if (input.type === 'keyDown' && input.shift && (input.control || input.meta) && input.key.toLowerCase() === 'd') {
      setDeveloper(!state.developer);
    }
  });
  win.loadFile(path.join(__dirname, '..', 'renderer', 'index.html'));
  win.on('close', (event) => {
    if (!keepsRunning()) return;
    event.preventDefault();
    // 운영체제 알림을 못 쓰면 숨긴 창의 알림을 놓친다(탐지 공백) — 그때는 숨기지 않고 최소화해 Dock·작업 표시줄에 남긴다.
    if (!Notification.isSupported()) {
      win.minimize();
      return;
    }
    win.hide();
    if (!backgroundHintShown) {
      backgroundHintShown = true;
      osNotify(notify.backgroundNotification(process.platform));
    }
  });
  win.on('closed', () => {
    win = null;
  });
  if (DEMO) startDemo();
  if (SCREENSHOT) {
    win.webContents.once('did-finish-load', () => {
      setTimeout(async () => {
        const image = await win.webContents.capturePage();
        fs.writeFileSync(SCREENSHOT, image.toPNG());
        app.quit();
      }, 1500);
    });
  }
}

function startDemo() {
  if (SCREEN !== 'onboarding') state.setup = { ...demo.SETUP };
  if (SCREEN && SCREEN.startsWith('calibration-')) state.calibration = demo.calibration(SCREEN.slice('calibration-'.length));
  const watching = SCREEN !== 'preparing';
  let remaining = 95;
  state.model = demo.model({ watching, remainingSecs: remaining });
  if (!watching) {
    setInterval(() => {
      remaining = Math.max(0, remaining - 1);
      state.model = demo.model({ watching: false, remainingSecs: remaining });
      send('model', shownModel());
    }, 1000);
  }
}

// 시험·자동 점검용으로 앱 데이터 폴더를 바꿀 수 있다(실제 감독관 설정을 건드리지 않게). 한 번에 하나만 뜨게 하는 잠금이
// 이 폴더를 기준으로 하므로 잠금보다 먼저 정한다.
if (process.env.TACET_USER_DATA) app.setPath('userData', process.env.TACET_USER_DATA);

if (!app.requestSingleInstanceLock()) {
  app.quit();
} else {
  app.on('second-instance', () => {
    if (app.isReady()) showWindow();
  });
  // macOS Dock 아이콘을 누르면 숨긴 창을 다시 연다.
  app.on('activate', () => {
    if (app.isReady()) showWindow();
  });
  // Windows 알림은 앱 식별자가 있어야 앱 이름으로 뜬다(package.json의 appId와 같다).
  if (process.platform === 'win32') app.setAppUserModelId('app.tacet.desktop');
  app.whenReady().then(() => {
    // 화면 확인용으로 밝은·어두운 모양을 고정할 수 있다(`--theme light|dark`). 기본은 운영체제 설정을 따른다.
    if (THEME === 'light' || THEME === 'dark') nativeTheme.themeSource = THEME;
    if (!DEMO) loadSettings();
    registerIpc();
    buildMenu();
    if (!SCREENSHOT) createTray();
    createWindow();
    startFromSettings();
  });
  app.on('window-all-closed', () => app.quit());
  app.on('before-quit', () => {
    quitting = true;
    stopCalibration();
    stopBackend();
  });
}
