#!/usr/bin/env python3
"""편집 후 훅(그리고 CI 게이트): 코드가 수동 감지·비디코딩·개인정보·알림 문구 규칙을 지키는지 본다.

guard_passive.py가 *실행*을 막는다면, 이 검사는 *코드에 들어간 것*을 본다 (AGENTS.md §3).

`crates/` 아래 Rust 소스 (주석과 문자열을 지운 코드 기준):
  * 송신 API — pcap 주입(`sendpacket`, `inject`), SDR 송신(`tx_stream`, `Direction::Tx`,
    `hackrf_start_tx`, `hackrf_set_txvga_gain`), BLE 광고(WinRT `BluetoothLEAdvertisementPublisher`,
    bluer `adv::Advertisement`, `advertise`),
    Wi-Fi 연결·호스팅(`WlanConnect`, `WlanHostedNetwork*`), 디어스. 전파법 (§3.1).
  * BLE 수집기에서 상대 기기와의 연결(`connect`, `discover_services`, `subscribe`) —
    연결 요청은 송신이고 광고만으로 충분하다 (§3.1).
  * 셀룰러 식별자·제어 채널 이름(`imsi`, `tmsi`, `guti`, `rnti`, `pdcch`, `pdsch`, `pusch`)과
    Wi-Fi 데이터 프레임 내용을 변수로 묶는 패턴. 통신비밀보호법 (§3.2).
  * 원격 수집·텔레메트리 크레이트(`sentry`, `opentelemetry` …) — 관측 데이터는 시험장
    내부망을 벗어나지 않는다 (§3.3).
  * 한 건은 그 줄이나 바로 윗줄의 `// PASSIVE-OK: <이유>`로 면제할 수 있다. 리뷰어가 그
    이유에 동의해야 한다.

감독관에게 보이는 문구 (Rust 문자열 리터럴 전부, `crates/app/`·`crates/dashboard/`·`desktop/`의 프런트엔드 파일):
  * 부정행위를 단정하는 문구(`부정행위 적발`, `cheating detected` …). 알림은 언제나
    "확인 필요"다 (§3.4).

저장소의 모든 텍스트 파일:
  * 문서용 범위 밖의 MAC 주소 (§3.3). 허용: `00:00:5E:00:53:xx`(RFC 7042), 랜덤 MAC 예시용
    `02:00:5E:00:53:xx`, 브로드캐스트, 전부 0.

모드: 기본(훅 페이로드의 경로), `--all`(저장소 전체, 발견하면 0이 아닌 값으로 종료 — CI).
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from _hookio import has_flag, hook_root, rel_path, repo_files, run_all, run_post_hook
from _rustscan import line_of, string_literals, strip

WAIVER = "PASSIVE-OK:"

# (지운 코드에 거는 패턴, 무엇인지)
TX_CODE: tuple[tuple[re.Pattern[str], str], ...] = (
    (re.compile(r"\.sendpacket\s*\(|\binject\s*\("), "pcap 프레임 주입"),
    (
        re.compile(r"\.tx_stream\w*\b|\bDirection::Tx\b|\bTxStream\b|\bhackrf_start_tx\b|\bhackrf_set_txvga_gain\b"),
        "SDR 송신",
    ),
    (re.compile(r"\bBluetoothLEAdvertisementPublisher\w*|\badv::Advertisement\b|\.advertise\s*\("), "BLE 광고 송신"),
    (re.compile(r"\bWlanConnect\w*\b|\bWlanHostedNetwork\w*"), "Wi-Fi 연결·호스팅"),
    (re.compile(r"\bdeauth\w*", re.IGNORECASE), "디어스 프레임"),
)
BLE_CONNECT = re.compile(r"\.(?:connect|discover_services|subscribe)\s*\(")
TELEMETRY = re.compile(r"\b(?:sentry|opentelemetry|posthog|datadog|bugsnag|rollbar)(?:_\w+)?::")
DECODE_CODE: tuple[tuple[re.Pattern[str], str], ...] = (
    (
        re.compile(r"\b\w*(?:imsi|tmsi|guti|rnti|pdcch|pdsch|pusch)\w*\b", re.IGNORECASE),
        "셀룰러 식별자·제어 채널 디코딩",
    ),
    (
        re.compile(r"\bFrame::(?:Qos)?(?:Null)?Data\s*(?:\(\s*[A-Za-z]|\{(?!\s*\.\.\s*\}))"),
        "Wi-Fi 데이터 프레임 내용 사용",
    ),
)

VERDICT_RE = re.compile(
    r"부정\s*행위\s*(?:적발|판정|확정|감지|확인|발견|자)|부정\s*행위가\s*(?:있|확인|감지)|"
    r"cheat(?:ing|er)?\s*(?:detected|found|caught|confirmed)|\bcheaters?\b",
    re.IGNORECASE,
)

_MAC_RE = re.compile(r"(?<![0-9A-Fa-f:\-])((?:[0-9A-Fa-f]{2}[:\-]){5}[0-9A-Fa-f]{2})(?![0-9A-Fa-f:\-])")
_MAC_ARRAY_RE = re.compile(
    r"(?i)(?:addr|mac|bssid)[^\n]{0,60}?\[\s*((?:0x[0-9a-f]{1,2}\s*,\s*){5}0x[0-9a-f]{1,2})\s*,?\s*\]"
)
# RFC 7042 문서용 유니캐스트·멀티캐스트 범위와, 그 locally administered 비트를 켠 예시.
DOC_PREFIXES = ("00005e0053", "02005e0053", "01005e9010", "03005e9010")
ALLOWED_MACS = frozenset({"ffffffffffff", "000000000000"})

FRONTEND_SUFFIXES = frozenset({".ts", ".tsx", ".js", ".jsx", ".html", ".svelte", ".vue"})
UI_ROOTS = ("crates/app/", "crates/dashboard/", "desktop/")
TEXT_SUFFIXES = frozenset(
    {".md", ".txt", ".rs", ".py", ".toml", ".json", ".yml", ".yaml", ".sh", ".ron", ".csv", ".rules"}
) | FRONTEND_SUFFIXES

RULE = (
    "수동 감지 규칙 (AGENTS.md §3): 송신·연결 코드 없음, 셀룰러 식별자·데이터 "
    "프레임 내용 디코딩 없음, 관측 데이터의 원격 전송 없음, 실제 MAC 없음(문서용 `00:00:5E:00:53:xx` / `02:00:5E:00:53:xx`만), 알림은 "
    "부정행위를 단정하지 않고 \"확인 필요\"로 쓴다. 정당한 예외는 `// PASSIVE-OK: <이유>`."
)


def _norm(mac: str) -> str:
    return re.sub(r"[^0-9a-f]", "", mac.lower())


def _allowed(hex12: str) -> bool:
    return hex12 in ALLOWED_MACS or hex12.startswith(DOC_PREFIXES)


def foreign_macs(text: str) -> list[str]:
    """문서용 범위 밖의 MAC 주소(문자열 표기와 addr/mac/bssid 옆의 6바이트 배열). 순수."""
    found: list[str] = []
    for match in _MAC_RE.finditer(text):
        if not _allowed(_norm(match.group(1))):
            found.append(match.group(1))
    for match in _MAC_ARRAY_RE.finditer(text):
        octets = [int(b, 16) for b in re.findall(r"0x([0-9a-fA-F]{1,2})", match.group(1))]
        hex12 = "".join(f"{b:02x}" for b in octets)
        if not _allowed(hex12):
            found.append(":".join(f"{b:02X}" for b in octets))
    return sorted(set(found))


def _waived(lines: list[str], line: int) -> bool:
    here = lines[line - 1] if 0 < line <= len(lines) else ""
    above = lines[line - 2] if 1 < line <= len(lines) + 1 else ""
    return WAIVER in here or WAIVER in above


def check_rust_source(source: str, label: str) -> list[str]:
    """Rust 소스 한 개의 위반. 순수해서 selftest가 텍스트를 바로 먹일 수 있다."""
    stripped = strip(source)
    lines = source.splitlines()
    violations: list[str] = []
    checks = list(TX_CODE) + list(DECODE_CODE)
    checks.append((TELEMETRY, "원격 수집·텔레메트리 — 관측 데이터는 내부망 밖으로 나가지 않는다"))
    if label.startswith("crates/collector-ble/"):
        checks.append((BLE_CONNECT, "BLE 상대 기기와 연결"))
    for pattern, what in checks:
        for match in pattern.finditer(stripped):
            line = line_of(stripped, match.start())
            if not _waived(lines, line):
                violations.append(f"  {label}:{line} — `{match.group(0).strip()}`: {what}.")
    for line, text in string_literals(source):
        if VERDICT_RE.search(text):
            violations.append(f"  {label}:{line} — 부정행위를 단정하는 문구: {text[:60]!r}. \"확인 필요\"로 쓴다.")
    return violations


def check_text(text: str, label: str) -> list[str]:
    macs = foreign_macs(text)
    if not macs:
        return []
    return [f"  {label} — 문서용 범위 밖의 MAC 주소: {', '.join(macs[:5])}."]


def check_frontend(text: str, label: str) -> list[str]:
    """앱·대시보드 프런트엔드 파일의 단정 문구. 순수."""
    found: list[str] = []
    for lineno, line in enumerate(text.splitlines(), start=1):
        if VERDICT_RE.search(line):
            found.append(f"  {label}:{lineno} — 부정행위를 단정하는 문구: {line.strip()[:60]!r}. \"확인 필요\"로 쓴다.")
    return found


def check_file(path: Path) -> list[str]:
    root = hook_root()
    if not path.is_file():
        return []
    rel = rel_path(path, root)
    if rel is None or path.suffix not in TEXT_SUFFIXES:
        return []
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return []
    violations = check_text(text, rel)
    if path.suffix == ".rs" and rel.startswith("crates/"):
        violations += check_rust_source(text, rel)
    if path.suffix in FRONTEND_SUFFIXES and rel.startswith(UI_ROOTS):
        violations += check_frontend(text, rel)
    return violations


def main() -> None:
    if has_flag("--all"):
        root = hook_root()
        files = sorted({f for suffix in TEXT_SUFFIXES for f in repo_files(root, suffix)})
        run_all(check_file, files, "수동 감지 검사", RULE)
    run_post_hook(check_file, "수동 감지 규칙 위반 (AGENTS.md §3)", RULE)


if __name__ == "__main__":
    main()
