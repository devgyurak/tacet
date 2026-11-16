#!/usr/bin/env python3
"""에이전트 훅과 에이전트 설정 검사기의 자체 테스트.

CI(`.github/workflows/agents.yml`)와 ./scripts/verify.sh가 돌린다. 로컬에서는:

    python3 scripts/agent_hooks/selftest.py

사례마다 훅 페이로드를 stdin으로 넣거나(또는 임시 트리를 만들어) 결정을 확인한다.
표준 라이브러리만 쓰고, 네트워크를 쓰지 않으며, 임시 트리 밖에는 쓰지 않는다.
실제 MAC처럼 보이는 값은 저장소에 남지 않도록 실행 중에 조립한다.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(REPO_ROOT / "scripts"))

FAILURES: list[str] = []

# 문서용 범위 밖의 MAC. 저장소 텍스트에 그대로 적히지 않게 조각으로 만든다.
REAL_MAC = ":".join(["3c", "22", "fb", "12", "34", "56"])
REAL_MAC_ARRAY = ", ".join("0x" + b for b in REAL_MAC.split(":"))
DOC_MAC = "00:00:5E:00:53:01"
DOC_RANDOM_MAC = "02:00:5E:00:53:2A"


def _run(
    script: str,
    tool: str,
    payload: dict[str, object] | None,
    *,
    root: Path | None = None,
    args: tuple[str, ...] = (),
    python: str = sys.executable,
) -> subprocess.CompletedProcess[str]:
    env = dict(os.environ)
    env["PYTHONDONTWRITEBYTECODE"] = "1"  # 훅 디렉터리에 __pycache__를 남기지 않는다
    if root is not None:
        env["TACET_HOOK_ROOT"] = str(root)
    return subprocess.run(
        [python, str(HERE / script), "--tool", tool, *args],
        input=json.dumps(payload) if payload is not None else "",
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
        env=env,
        cwd=str(root or REPO_ROOT),
    )


def _checked(name: str, condition: bool, detail: str = "") -> None:
    if not condition:
        FAILURES.append(f"{name}{': ' + detail.strip()[:400] if detail else ''}")


def _denied(result: subprocess.CompletedProcess[str]) -> bool:
    return '"permissionDecision": "deny"' in result.stdout


def _blocked(result: subprocess.CompletedProcess[str]) -> bool:
    return '"decision": "block"' in result.stdout


def _write(root: Path, rel: str, text: str = "") -> Path:
    target = root / rel
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text, encoding="utf-8")
    return target


def _read(path: str) -> dict[str, object]:
    return {"tool_input": {"file_path": path}}


def _bash(command: str) -> dict[str, object]:
    return {"tool_input": {"command": command}}


def _fetch(url: str) -> dict[str, object]:
    return {"tool_name": "WebFetch", "tool_input": {"url": url, "prompt": "x"}}


def _write_payload(path: str, content: str) -> dict[str, object]:
    return {"tool_input": {"file_path": path, "content": content}}


def _patch(path: str, body: str) -> dict[str, object]:
    return {"tool_name": "apply_patch", "tool_input": {"command": f"*** Begin Patch\n*** Add File: {path}\n+{body}\n*** End Patch"}}


# --- guard_passive ------------------------------------------------------------------------


def test_guard_passive() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp).resolve()
        synthetic = _write(root, "tests/fixtures/synthetic/hidden_ssid.pcap")
        allowed = (
            ("문서 읽기", _read(str(REPO_ROOT / "AGENTS.md"))),
            ("우리 바이너리로 캡처 재생", _bash("cargo run -p tacet -- replay ~/captures/room.pcapng")),
            ("수신 전용 SDR 녹음", _bash("hackrf_transfer -r room.cs8 -f 1950000000")),
            ("관리 모드 스캔", _bash("sudo iw dev wlan0 scan")),
            ("BLE 수신 스캔", _bash("bluetoothctl scan on")),
            ("이름 검색", _bash('rg -n "srsRAN" docs')),
            ("합성 픽스처 읽기", _read(str(synthetic))),
            ("데이터시트 열람", _fetch("https://www.bluetooth.com/specifications/assigned-numbers/")),
            ("문서용 MAC 쓰기", _write_payload(str(root / "docs/rules/R4.md"), f"예: {DOC_MAC}, {DOC_RANDOM_MAC}")),
            ("브로드캐스트 주소 쓰기", _write_payload(str(root / "x.rs"), 'const B: &str = "FF:FF:FF:FF:FF:FF";')),
            ("평범한 core 편집", _write_payload(str(root / "crates/core/src/lib.rs"), "pub fn f() {}")),
        )
        denied = (
            ("디어스 도구", _bash(f"sudo aireplay-ng --deauth 5 -a {DOC_MAC} wlan0mon")),
            ("비콘 플러딩 도구", _bash("mdk4 wlan0mon b")),
            ("SDR 송신", _bash("hackrf_transfer -t tone.cs8 -f 1950000000")),
            ("BLE 광고 송신", _bash("bluetoothctl advertise on")),
            ("핫스팟 켜기", _bash("nmcli device wifi hotspot ssid exam password 12345678")),
            ("Kismet DB 읽기", _read(str(Path.home() / "kismet" / "room.kismet"))),
            ("관측 녹화 덤프", _bash("cat ~/recordings/room.obslog")),
            ("LTE 스니퍼 클론", _bash("git clone https://github.com/SysSec-KAIST/LTESniffer")),
            ("디코더 설치", _bash("pip install srsran")),
            ("디코더 체크아웃 읽기", _read(str(Path.home() / "src" / "srsRAN" / "lib" / "phy.cc"))),
            ("디코더 코드 호스트", _fetch("https://github.com/srsran/srsRAN_Project")),
            ("실제 캡처 읽기", _read(str(Path.home() / "captures" / "exam.pcapng"))),
            ("tshark로 캡처 덤프", _bash("tshark -r ~/captures/exam.pcapng -V")),
            ("IQ 녹음 헥스덤프", _bash("xxd room.cu8 | head")),
            ("합성 캡처도 커밋 금지", _bash("git add tests/fixtures/synthetic/hidden_ssid.pcap")),
            ("실제 MAC 쓰기", _write_payload(str(root / "docs/field/2026-11-20.md"), f"관측: {REAL_MAC}")),
            ("실제 MAC 배열 쓰기", _write_payload(str(root / "crates/core/src/x.rs"), f"let addr = [{REAL_MAC_ARRAY}];")),
            ("Codex 패치로 캡처 추가", _patch("logs/run.pcap", "x")),
            ("Codex 패치로 실제 MAC 추가", _patch("docs/a.md", REAL_MAC)),
        )
        for name, payload in allowed:
            for tool in ("claude", "codex"):
                result = _run("guard_passive.py", tool, payload, root=root)
                _checked(f"guard[{tool}] 허용: {name}", not _denied(result) and result.returncode == 0, result.stdout + result.stderr)
        for name, payload in denied:
            for tool in ("claude", "codex"):
                result = _run("guard_passive.py", tool, payload, root=root)
                _checked(f"guard[{tool}] 차단: {name}", _denied(result), result.stdout + result.stderr)


def test_guard_scan_tracked() -> None:
    if shutil.which("git") is None:
        return
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp).resolve()
        subprocess.run(["git", "init", "-q", str(root)], check=True)
        _write(root, "README.md", "# 깨끗함\n")
        subprocess.run(["git", "-C", str(root), "add", "-A"], check=True)
        clean = _run("guard_passive.py", "claude", None, root=root, args=("--scan-tracked",))
        _checked("scan: 깨끗한 트리는 통과", clean.returncode == 0, clean.stderr)
        _write(root, "captures/room.pcapng", "x")
        _write(root, "vendor/LTESniffer/README.md", "x")
        subprocess.run(["git", "-C", str(root), "add", "-f", "-A"], check=True)
        dirty = _run("guard_passive.py", "claude", None, root=root, args=("--scan-tracked",))
        _checked("scan: 추적된 캡처를 잡는다", dirty.returncode == 1 and "room.pcapng" in dirty.stderr, dirty.stderr)
        _checked("scan: 디코더 디렉터리를 잡는다", "LTESniffer" in dirty.stderr, dirty.stderr)


# --- check_passive ------------------------------------------------------------------------


def test_check_passive() -> None:
    import check_passive as cp

    def hits(src: str, label: str) -> list[str]:
        return cp.check_rust_source(src, label)

    mon = "crates/collector-sdr/src/lib.rs"
    _checked("passive: pcap 주입", bool(hits("fn f(c: &mut Cap) { c.sendpacket(&buf); }", mon)))
    _checked("passive: SDR 송신 스트림", bool(hits("fn f(d: Device) { let s = d.tx_stream::<f32>(&[0]); }", "crates/collector-sdr/src/lib.rs")))
    _checked("passive: BLE 광고(bluer)", bool(hits("fn f() { let a = bluer::adv::Advertisement::default(); }", "crates/collector-ble/src/lib.rs")))
    _checked("passive: BLE 광고(WinRT)", bool(hits("fn f() { let p = BluetoothLEAdvertisementPublisher::new(); }", "crates/collector-ble/src/sys_windows.rs")))
    _checked("passive: WinRT 수신 네임스페이스는 허용", not hits("use windows::Devices::Bluetooth::Advertisement::BluetoothLEAdvertisementWatcher;", "crates/collector-ble/src/sys_windows.rs"))
    _checked("passive: BLE 연결", bool(hits("async fn f(p: P) { p.connect().await; }", "crates/collector-ble/src/scan.rs")))
    _checked("passive: net의 websocket connect는 허용", not hits("async fn f(c: C) { c.connect().await; }", "crates/net/src/ws.rs"))
    _checked("passive: HackRF 송신", bool(hits("fn f(d: *mut D) { hackrf_start_tx(d, cb, ctx); }", "crates/collector-sdr/src/sys.rs")))
    _checked("passive: 텔레메트리", bool(hits("fn f() { sentry::capture_message(m, l); }", "crates/app/src/main.rs")))
    _checked("passive: 셀룰러 식별자", bool(hits("fn f() { let imsi = 0; }", "crates/collector-sdr/src/lib.rs")))
    _checked("passive: 데이터 프레임 내용 사용", bool(hits("fn f(x: Frame) { match x { Frame::Data(d) => use_it(d), _ => {} } }", mon)))
    _checked("passive: 데이터 프레임은 버리면 허용", not hits("fn f(x: Frame) { match x { Frame::Data(_) => {}, _ => {} } }", mon))
    _checked("passive: 주석 안의 단어는 무시", not hits("// deauth 프레임은 다루지 않는다\nfn f() {}", mon))
    _checked("passive: 면제 표식", not hits("// PASSIVE-OK: 수신 경로의 함수 이름이다\nfn inject(x: u8) {}", mon))
    _checked("passive: 단정 문구", bool(hits('fn f() -> &str { "부정행위 적발!" }', "crates/app/src/text.rs")))
    _checked("passive: 영어 단정 문구", bool(hits('fn f() -> &str { "Cheating detected" }', "crates/dashboard/src/text.rs")))
    _checked("passive: 확인 필요 문구는 허용", not hits('fn f() -> &str { "확인 필요: 새 BLE 기기" }', "crates/app/src/text.rs"))
    _checked("passive: 프런트엔드 단정 문구", bool(cp.check_frontend("<h1>부정행위 적발</h1>", "crates/dashboard/ui/a.html")))
    _checked("passive: 프런트엔드 확인 필요", not cp.check_frontend("<h1>[확인 필요] 숨긴 Wi-Fi 핫스팟</h1>", "crates/dashboard/ui/a.html"))

    _checked("mac: 문서용 범위는 허용", cp.foreign_macs(f"{DOC_MAC} {DOC_RANDOM_MAC} ff:ff:ff:ff:ff:ff 01:00:5E:90:10:01 03:00:5E:90:10:01") == [])
    _checked("mac: 실제 MAC을 잡는다", cp.foreign_macs(f"주소 {REAL_MAC}.") == [REAL_MAC])
    _checked("mac: 대시 표기도 잡는다", cp.foreign_macs(REAL_MAC.replace(":", "-")) != [])
    _checked("mac: 6바이트 배열을 잡는다", cp.foreign_macs(f"let bssid = [{REAL_MAC_ARRAY}];") != [])
    _checked("mac: 문서용 배열은 허용", cp.foreign_macs("let addr = [0x00, 0x00, 0x5e, 0x00, 0x53, 0x01];") == [])
    _checked("mac: 시각·IPv6는 MAC이 아니다", cp.foreign_macs("12:34:56 fe80::1 2001:db8::1") == [])

    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp).resolve()
        bad = _write(root, "crates/collector-sdr/src/lib.rs", "fn f(c: &mut C) { c.sendpacket(&b); }\n")
        result = _run("check_passive.py", "claude", _read(str(bad)), root=root)
        _checked("passive 훅: 송신 코드를 막는다", _blocked(result), result.stdout + result.stderr)
        good = _write(root, "crates/collector-sdr/src/rx.rs", "fn f() {}\n")
        result = _run("check_passive.py", "codex", _read(str(good)), root=root)
        _checked("passive 훅: 깨끗한 코드는 통과", not _blocked(result) and result.returncode == 0, result.stdout)
        result = _run("check_passive.py", "claude", None, root=root, args=("--all",))
        _checked("passive --all: 위반이면 실패", result.returncode == 1 and "sendpacket" in result.stderr, result.stderr)


# --- check_architecture -------------------------------------------------------------------


def test_check_architecture_sources() -> None:
    import check_architecture as ca

    core = "crates/core/src/score.rs"
    cases_bad = (
        ("std::fs", "use std::fs;\n"),
        ("std 묶음", "use std::{fmt, io};\n"),
        ("Instant", "fn f() { let t = Instant::now(); }\n"),
        ("HashMap", "struct S { m: HashMap<u8, u8> }\n"),
        ("async", "async fn f() {}\n"),
        ("println", 'fn f() { println!("x"); }\n'),
        ("OS 분기", '#[cfg(target_os = "linux")]\nfn f() {}\n'),
        ("static mut", "static mut X: u8 = 0;\n"),
    )
    for name, src in cases_bad:
        _checked(f"arch: core의 {name}", bool(ca.check_rust_source(src, core)), name)
    _checked("arch: core 면제 표식", not ca.check_rust_source("// PURITY-OK: 테스트 도우미에서만 쓴다\nuse std::fs;\n", core))
    _checked("arch: 주석 속 단어는 무시", not ca.check_rust_source("// Instant 대신 Timestamp를 쓴다\nfn f() {}\n", core))
    _checked("arch: 수집기의 OS 분기는 허용", not ca.check_rust_source('#[cfg(target_os = "windows")]\nmod sys;\n', "crates/collector-wifi/src/lib.rs"))
    _checked("arch: net의 OS 분기", bool(ca.check_rust_source('#[cfg(windows)]\nfn f() {}\n', "crates/net/src/lib.rs")))
    _checked("arch: dashboard의 OS 분기", bool(ca.check_rust_source('#[cfg(target_os = "macos")]\nfn f() {}\n', "crates/dashboard/src/lib.rs")))
    _checked("arch: 앱의 OS 분기는 허용", not ca.check_rust_source('#[cfg(windows)]\nmod power;\n', "crates/app/src/main.rs"))
    _checked("arch: SAFETY 없는 unsafe", bool(ca.check_rust_source("fn f() { unsafe { g() } }\n", "crates/collector-wifi/src/sys/windows.rs")))
    _checked(
        "arch: SAFETY 있는 unsafe",
        not ca.check_rust_source("fn f() {\n    // SAFETY: 핸들은 위에서 연 것이다.\n    unsafe { g() }\n}\n", "crates/collector-wifi/src/sys/windows.rs"),
    )


def test_check_architecture_manifests() -> None:
    import check_architecture as ca

    def manifest(name: str, deps: str) -> str:
        return f'[package]\nname = "{name}"\n\n[dependencies]\n{deps}\n'

    bad = (
        ("core가 tokio", manifest("tacet-core", 'tokio = "1"')),
        ("core가 작업 공간 크레이트", manifest("tacet-core", 'tacet-net = { path = "../net" }')),
        ("수집기끼리 의존", manifest("tacet-collector-ble", 'tacet-collector-wifi = { path = "../collector-wifi" }')),
        ("dashboard가 net", manifest("tacet-dashboard", 'tacet-net = { workspace = true }')),
        ("net이 수집기", manifest("tacet-net", 'tacet-collector-sdr = { workspace = true }')),
        ("앱에 의존", manifest("tacet-dashboard", 'tacet = { path = "../app" }')),
        ("devtools에 의존", manifest("tacet-collector-ble", 'tacet-devtools = { path = "../devtools" }')),
        ("core가 tauri", manifest("tacet-core", 'tauri = "2"')),
        ("이름 접두사", manifest("exam-detector", "")),
        ("이름 바꾼 금지 크레이트", manifest("tacet-core", 'rt = { package = "tokio", version = "1" }')),
    )
    for name, text in bad:
        _checked(f"manifest 위반: {name}", bool(ca.check_manifest_text(text, "Cargo.toml")), name)
    good = (
        ("core는 serde", manifest("tacet-core", 'serde = { version = "1", features = ["derive"] }')),
        ("수집기는 core", manifest("tacet-collector-ble", 'tacet-core = { workspace = true }\nbtleplug = "0.11"')),
        ("앱은 전부", manifest("tacet", 'tacet-core = { workspace = true }\ntacet-net = { workspace = true }\ntacet-dashboard = { workspace = true }')),
        ("devtools는 수집기·net", manifest("tacet-devtools", 'tacet-collector-sdr = { workspace = true }\ntacet-net = { workspace = true }')),
        ("core의 dev 의존", '[package]\nname = "tacet-core"\n\n[dev-dependencies]\nproptest = "1"\n'),
    )
    for name, text in good:
        _checked(f"manifest 허용: {name}", not ca.check_manifest_text(text, "Cargo.toml"), str(ca.check_manifest_text(text, "Cargo.toml")))
    inherited = manifest("tacet-core", "rt = { workspace = true }")
    _checked("manifest: 작업 공간에서 이름 바꾼 의존", bool(ca.check_manifest_text(inherited, "Cargo.toml", {"rt": "tokio"})))


# --- check_tests --------------------------------------------------------------------------


def test_check_tests() -> None:
    import check_tests as ct

    label = "crates/core/src/score/tests.rs"
    good = (
        "#[test]\nfn given_hidden_ssid_beacon_when_scored_then_r5_hits() {\n"
        "    let hits = score(&beacon());\n    assert_eq!(hits, vec![RuleId::R5]);\n}\n"
    )
    _checked("tests: 규칙을 지키면 통과", ct.check_source(good, label) == [], str(ct.check_source(good, label)))
    bad_name = "#[test]\nfn hidden_ssid() { assert!(true); }\n"
    _checked("tests: GWT 아닌 이름", bool(ct.check_source(bad_name, label)))
    vague = "#[test]\nfn given_a_when_b_then_works() { assert!(true); }\n"
    _checked("tests: 모호한 then", bool(ct.check_source(vague, label)))
    flow = "#[test]\nfn given_a_when_b_then_c_is_d() { for x in xs() { assert!(x); } }\n"
    _checked("tests: 제어 흐름", bool(ct.check_source(flow, label)))
    no_assert = "#[test]\nfn given_a_when_b_then_c_is_d() { run(); }\n"
    _checked("tests: 단언 없음", bool(ct.check_source(no_assert, label)))
    inline = "pub fn f() {}\n#[cfg(test)]\nmod tests {\n}\n"
    _checked("tests: 인라인 모듈", bool(ct.check_source(inline, "crates/core/src/score/mod.rs")))
    _checked("tests: 형제 파일", bool(ct.check_source("", "crates/core/src/tests_rules.rs")))


# --- check_language -----------------------------------------------------------------------


def test_check_language() -> None:
    import check_language as cl
    # 프런트엔드: 클래스 목록·설정 줄·HTML 속성은 문구가 아니다. 영어 문장은 여전히 잡는다.
    assert cl.frontend_ui_findings("el.className = `chip phase ${on ? 'watching' : 'preparing'}`;", "desktop/a.js", ".js") == []
    assert cl.frontend_ui_findings("lines.push(`state_dir = ${x}`);", "desktop/a.js", ".js") == []
    assert cl.frontend_ui_findings('<meta name="viewport" content="width=device-width, initial-scale=1" />', "desktop/a.html", ".html") == []
    assert cl.frontend_ui_findings("h('p', {}, 'No alerts to check right now')", "desktop/a.js", ".js") != []
    assert cl.frontend_ui_findings("<p>No alerts to check right now</p>", "desktop/a.html", ".html") != []

    english = "This document explains how the detector scores every new device it observes in the room.\n"
    korean = "이 문서는 탐지기가 교실에서 새로 관측한 기기마다 점수를 매기는 방식을 설명한다. RSSI와 BLE는 그대로 쓴다.\n"
    _checked("lang: 영어 문단", bool(cl.check_text(english, "docs/a.md")))
    _checked("lang: 한국어 문단", not cl.check_text(korean, "docs/a.md"))
    fenced = "설명은 한국어다.\n\n```text\n" + english + "```\n"
    _checked("lang: 코드 블록은 제외", not cl.check_text(fenced, "docs/a.md"))
    _checked("lang: 번역본 이름", cl.is_translation("README-EN.md") and not cl.is_translation("R4-random-mac.md"))

    rust_en = "// This function scores a beacon against the baseline and returns hits.\nfn f() {}\n"
    rust_ko = "// 비콘을 기준선과 비교해 점수를 매기고 적중 목록을 돌려준다.\nfn f() {}\n"
    _checked("lang: 영어 Rust 주석", bool(cl.check_text(rust_en, "crates/core/src/a.rs")))
    _checked("lang: 한국어 Rust 주석", not cl.check_text(rust_ko, "crates/core/src/a.rs"))
    doc_code = "/// 예시:\n/// ```\n/// let score = engine.score(&obs); // returns the total score for this one\n/// ```\nfn f() {}\n"
    _checked("lang: 문서 주석 속 코드 블록은 제외", not cl.check_text(doc_code, "crates/core/src/a.rs"), str(cl.check_text(doc_code, "crates/core/src/a.rs")))
    _checked("lang: 문자열 속 // 는 주석이 아니다", not cl.check_text('fn f() -> &str { "http://example.com/a/b/c/d/e/f/g" }\n', "crates/core/src/a.rs"))
    ui_en = 'fn f() -> &str { "New device detected near the front" }\n'
    _checked("lang: 영어 사용자 문구", bool(cl.check_text(ui_en, "crates/app/src/text.rs")))
    _checked("lang: 한국어 사용자 문구", not cl.check_text('fn f() -> &str { "확인 필요: 앞쪽에 새 기기" }\n', "crates/dashboard/src/text.rs"))
    _checked("lang: 영어 프런트엔드 문구", bool(cl.check_text('const t = "New device detected near the front";\n', "crates/app/ui/alert.ts")))
    _checked("lang: 영어 HTML 텍스트", bool(cl.check_text("<p>New device detected near the front</p>\n", "crates/dashboard/ui/index.html")))
    _checked("lang: 한국어 HTML 텍스트", not cl.check_text("<p>[확인 필요] 3-2 교실 뒤쪽 · 신뢰도 중</p>\n", "crates/dashboard/ui/index.html"))
    _checked("lang: 영어 TS 주석", bool(cl.check_text("// render the alert list for the supervisor view here\nexport {};\n", "crates/app/ui/a.ts")))
    _checked("lang: core의 영어 문자열은 대상 아님", not cl.check_text(ui_en, "crates/core/src/a.rs"))
    cookie = 'fn f() -> String { format!("{COOKIE}={code}; HttpOnly; SameSite=Strict; Path=/") }\n'
    _checked("lang: 프로토콜 문자열은 사용자 문구가 아님", not cl.check_text(cookie, "crates/dashboard/src/lib.rs"))
    _checked("lang: 테스트 파일의 문자열은 대상 아님", not cl.check_text(ui_en, "crates/app/src/tests.rs"))
    # 이 줄 자체가 영어 주석으로 잡히지 않도록 주석 표식을 따로 붙인다.
    py_en = "x = 1  " + "# keep the value here so the next step can read it again\n"
    _checked("lang: 영어 Python 주석", bool(cl.check_text(py_en, "scripts/a.py")))
    marker = "fn f() {\n    // SAFETY: 핸들은 위에서 열었고 아직 닫지 않았다.\n    unsafe { g() }\n}\n"
    _checked("lang: 표식 뒤 한국어", not cl.check_text(marker, "crates/collector-wifi/src/a.rs"))

    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp).resolve()
        bad = _write(root, "docs/notes.md", english)
        result = _run("check_language.py", "claude", _read(str(bad)), root=root)
        _checked("lang 훅: 영어 문서를 막는다", _blocked(result), result.stdout + result.stderr)
        translation = _write(root, "README-EN.md", "x")
        result = _run("check_language.py", "codex", _read(str(translation)), root=root)
        _checked("lang 훅: 번역본을 막는다", _blocked(result), result.stdout + result.stderr)


# --- format_rust · session_context --------------------------------------------------------


def test_format_rust() -> None:
    result = _run("format_rust.py", "claude", _read(str(REPO_ROOT / "AGENTS.md")))
    _checked("format: .rs가 아니면 조용히 통과", result.returncode == 0 and result.stdout.strip() == "", result.stdout + result.stderr)


def test_session_context() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp).resolve()
        _write(root, "docs/STATUS.md", "# 상태\n\n▶ Phase 1 — 시험 단계\n")
        result = _run("session_context.py", "claude", {}, root=root)
        _checked("session: 현재 단계를 알린다", "Phase 1 — 시험 단계" in result.stdout and "additionalContext" in result.stdout, result.stdout)
        _write(root, "docs/STATUS.md", "▶ 하나\n▶ 둘\n")
        result = _run("session_context.py", "codex", {}, root=root)
        _checked("session: ▶가 둘이면 알 수 없음", "알 수 없음" in result.stdout, result.stdout)


def test_hooks_survive_old_python() -> None:
    """macOS의 /usr/bin/python3(3.9)에서도 훅이 문법 오류 없이 돈다."""
    old = Path("/usr/bin/python3")
    if not old.is_file():
        return
    for script in ("guard_passive.py", "check_passive.py", "check_architecture.py", "check_tests.py", "check_language.py", "format_rust.py", "session_context.py"):
        result = _run(script, "claude", {}, python=str(old))
        _checked(f"python3.9: {script}", result.returncode == 0, result.stderr)


# --- check_agent_config (순수 도우미) -------------------------------------------------------


def test_config_helpers() -> None:
    if sys.version_info < (3, 11):
        return  # check_agent_config는 tomllib이 필요하다. CI는 3.12에서 돌린다.
    import check_agent_config as cfg

    marker = "**core 금지 크레이트:**"
    canon = f"{marker} `tokio` · `rand`.\n"
    _checked("config: 같은 목록은 통과", cfg.list_mismatch(canon, marker, {"tokio", "rand"}, "x") is None)
    drifted = cfg.list_mismatch(canon, marker, {"tokio"}, "x")
    _checked("config: 어긋난 목록을 알린다", drifted is not None and "rand" in drifted, str(drifted))
    _checked("config: 목록 줄이 없으면 알린다", cfg.list_mismatch("없음", marker, {"tokio"}, "x") is not None)

    floor = "**커버리지 하한:** `85%` 줄 커버리지.\n"
    _checked("config: 같은 하한은 통과", cfg.coverage_mismatch(floor, "COVERAGE_FLOOR=85\n") is None)
    lowered = cfg.coverage_mismatch(floor, "COVERAGE_FLOOR=70\n")
    _checked("config: 낮춘 하한을 알린다", lowered is not None and "70" in lowered, str(lowered))

    settings = {"permissions": {"ask": ["Bash(cargo add:*)"], "deny": ["Bash(rm -rf:*)"]}}
    rules = (
        'prefix_rule(pattern = ["cargo", "add"], decision = "prompt")\n'
        'prefix_rule(pattern = ["rm", "-rf"], decision = "forbidden")\n'
    )
    _checked("config: 권한 정책이 같으면 통과", cfg.permission_mismatch(settings, rules) == [])
    missing = cfg.permission_mismatch(settings, rules.splitlines()[0])
    _checked("config: Codex 규칙에 없는 deny를 알린다", any("rm -rf" in m for m in missing), str(missing))


def main() -> int:
    test_guard_passive()
    test_guard_scan_tracked()
    test_check_passive()
    test_check_architecture_sources()
    test_check_architecture_manifests()
    test_check_tests()
    test_check_language()
    test_format_rust()
    test_session_context()
    test_hooks_survive_old_python()
    test_config_helpers()

    if FAILURES:
        print("훅 selftest 실패:", file=sys.stderr)
        for failure in FAILURES:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    print("훅 selftest: 모두 통과")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
