#!/usr/bin/env python3
"""도구 실행 전 훅(그리고 CI 스캔): 이 도구를 수동 감지 도구로 남긴다 (AGENTS.md §3).

여기서 실수하면 되돌릴 수 없다. 전파를 한 번 쏘면 그걸로 전파법 위반이고, 남의 MAC이
담긴 캡처가 git 기록에 들어가면 모든 클론에 남는다. 그래서 도구가 실행되기 *전에* 막는다.

  1. **송신 도구** — 재밍·주입·디어스·가짜 AP·광고·SDR 송신 도구 실행. 전파법 (§3.1).
  2. **셀룰러/패킷 디코더** — LTE 스니퍼, IMSI 캐처 같은 프로젝트의 체크아웃, 설치, 코드
     호스트 페이지. 통신비밀보호법 (§3.2).
  3. **실제 캡처** — pcap·IQ 녹음·Kismet DB·관측 녹화(.obslog) 같은 캡처 파일의 바이트를 대화로 읽기(Read 도구,
     cat/xxd/tshark…), 쓰기, `git`에 올리기. 우리 바이너리로 캡처를 *재생*하는 것은 된다.
     `git`은 어디서든 막는다 — 합성 픽스처도 파일이 아니라 코드로 만든다 (§3.3).
  4. **실제 MAC 쓰기** — 쓰려는 텍스트에 문서용 범위 밖의 MAC 주소가 있으면 막는다 (§3.3).

**최선 노력** 휴리스틱이지 샌드박스가 아니다. 판단하지 못하면 닫힌 쪽으로 실패한다.
`--scan-tracked`는 같은 규칙을 `git ls-files`에 적용하고(서명 인증서·키도 본다), ./scripts/verify.sh와 CI가 돌린다.

사용법:
    guard_passive.py --tool claude|codex   (훅 페이로드 JSON은 stdin)
    guard_passive.py --scan-tracked
"""

from __future__ import annotations

import os
import re
import shlex
import subprocess
import sys
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

sys.path.insert(0, str(Path(__file__).resolve().parent))

from _hookio import (
    emit_allow,
    emit_deny,
    extract_command,
    extract_paths,
    extract_urls,
    extract_written_text,
    has_flag,
    hook_root,
    looks_like_patch,
    parse_args,
    read_payload,
)
from check_passive import foreign_macs

# 실행 자체가 송신인 도구. AGENTS.md §3.1이 같은 목록을 적고, 둘이 다르면
# scripts/check_agent_config.py가 실패한다.
TX_TOOLS = frozenset(
    {
        "aireplay-ng",
        "airbase-ng",
        "mdk3",
        "mdk4",
        "wifiphisher",
        "hostapd",
        "create_ap",
        "rpitx",
        "sendiq",
    }
)

# 인자에 따라 송신이 되는 도구: (명령, 송신을 뜻하는 인자들).
TX_WHEN = {
    "hackrf_transfer": frozenset({"-t", "-c"}),
    "bluetoothctl": frozenset({"advertise"}),
    "btmgmt": frozenset({"advertising", "add-adv"}),
    "hcitool": frozenset({"cmd", "lecc", "cc"}),
    "nmcli": frozenset({"hotspot"}),
}

# 셀룰러 제어 정보·식별자나 패킷 내용을 디코딩하는 프로젝트. AGENTS.md §3.2가 같은 목록을
# 적고, 둘이 다르면 scripts/check_agent_config.py가 실패한다.
DECODER_PROJECTS = frozenset(
    {
        "ltesniffer",
        "srsran",
        "srslte",
        "imsi-catcher",
        "gr-gsm",
        "openbts",
        "lte-cell-scanner",
    }
)
_DECODER_COMPONENT_RE = re.compile(
    r"^(?:%s)(?:[-_. ].*)?$" % "|".join(re.escape(name) for name in sorted(DECODER_PROJECTS)), re.IGNORECASE
)

# 무선 캡처·IQ 녹음 파일. AGENTS.md §3.3이 같은 목록을 적고, 둘이 다르면
# scripts/check_agent_config.py가 실패한다.
CAPTURE_SUFFIXES = frozenset(
    {
        ".pcap",
        ".pcapng",
        ".cap",
        ".btsnoop",
        ".iq",
        ".cfile",
        ".cu8",
        ".cs8",
        ".cs16",
        ".cf32",
        ".sigmf-data",
        ".sigmf-meta",
        ".kismet",
        ".kismet-journal",
        ".obslog",
    }
)

# 코드 서명 인증서·키. 서명은 CI 비밀값으로만 한다 (AGENTS.md §7).
SECRET_SUFFIXES = frozenset({".p12", ".pfx", ".p8", ".keystore", ".mobileprovision", ".provisionprofile"})

# 우리 코드가 만든 합성 데이터만 있는 디렉터리. 여기서는 캡처 확장자 파일을 읽고 쓸 수 있다.
SYNTHETIC_DIRS = (("tests", "fixtures", "synthetic"),)

# 파일 내용을 읽거나 복사하거나 해석하는 명령.
FILE_READ_VERBS = frozenset(
    {
        "cat",
        "bat",
        "less",
        "more",
        "head",
        "tail",
        "strings",
        "xxd",
        "hexdump",
        "hd",
        "od",
        "dd",
        "cp",
        "mv",
        "ln",
        "scp",
        "rsync",
        "tar",
        "zip",
        "base64",
        "python",
        "python3",
        "perl",
        "ruby",
        "node",
        "tshark",
        "tcpdump",
        "editcap",
        "capinfos",
        "mergecap",
        "git",
    }
)
INSTALL_VERBS = frozenset({"pip", "pip3", "pipx", "brew", "apt", "apt-get", "dnf", "pacman", "port", "snap", "cargo", "npm"})
PREFIXES = ("sudo", "time", "env", "nice", "nohup", "command", "exec")
_SEGMENT_SPLIT_RE = re.compile(r"\s*(?:&&|\|\||;|\||\n)\s*")

REASON_TX = (
    "차단: 이 도구는 전파를 쏘지 않는다 (AGENTS.md §3.1). 재밍·주입·"
    "디어스·가짜 AP·광고 송신·SDR 송신 도구는 에이전트가 실행하지 않는다. 시험 시나리오는 합성 관측값으로 "
    "만든다 (test-author 서브에이전트)."
)
REASON_DECODER = (
    "차단: 셀룰러 제어 정보·식별자나 패킷 내용을 디코딩하는 프로젝트는 열지도, 받지도, 설치하지도 "
    "않는다 (AGENTS.md §3.2, 통신비밀보호법). 이 도구는 에너지·비콘 헤더·광고 메타데이터만 쓴다."
)
REASON_CAPTURE = (
    "차단: 실제 무선 캡처(pcap·IQ 녹음 등)에는 남의 MAC과 통신 흔적이 들어 있다. 그 바이트를 "
    "대화로 읽거나 복사하거나 저장소에 쓰지 않는다 (AGENTS.md §3.3). 우리 바이너리로 재생하는 "
    "것은 된다. 테스트 데이터는 코드로 만든 합성 관측값을 쓴다."
)
REASON_COMMIT = (
    "차단: 캡처 파일은 어떤 것도 git에 올리지 않는다 — 합성 데이터도 파일이 아니라 코드로 "
    "만든다 (AGENTS.md §3.3)."
)
REASON_MAC = (
    "차단: 쓰려는 내용에 문서용 범위 밖의 MAC 주소가 있다 (AGENTS.md §3.3, 개인정보). 예시와 "
    "테스트는 `00:00:5E:00:53:xx`(RFC 7042 문서용) 또는 랜덤 MAC 예시용 `02:00:5E:00:53:xx`만 쓴다. "
    "실측에서 나온 주소는 저장소·문서·커밋 어디에도 적지 않는다: {macs}"
)


def _parts(raw: str) -> tuple[str, ...]:
    return Path(os.path.normpath(os.path.expanduser(raw))).parts


def capture_suffix(raw: str) -> bool:
    name = Path(raw).name.lower()
    return any(name.endswith(suffix) for suffix in CAPTURE_SUFFIXES)


def _in_synthetic_dir(raw: str, root: Path) -> bool:
    path = Path(os.path.expanduser(raw))
    if not path.is_absolute():
        path = root / path
    try:
        rel = path.resolve().relative_to(root).parts
    except (ValueError, OSError):
        return False
    return any(rel[: len(prefix)] == prefix for prefix in SYNTHETIC_DIRS)


def is_foreign_capture(raw: str, root: Path) -> bool:
    """합성 디렉터리 밖의 캡처 파일."""
    return capture_suffix(raw) and not _in_synthetic_dir(raw, root)


def is_decoder_path(raw: str) -> bool:
    return any(_DECODER_COMPONENT_RE.match(part) for part in _parts(raw))


def is_decoder_url(url: str) -> bool:
    if url.startswith("git@"):
        _, _, rest = url[4:].partition(":")
    else:
        rest = urlparse(url).path
    return any(_DECODER_COMPONENT_RE.match(seg) for seg in rest.split("/") if seg)


def _tokens(segment: str) -> list[str]:
    try:
        toks = shlex.split(segment)
    except ValueError:
        toks = segment.split()
    while toks and (re.match(r"^\w+=", toks[0]) or toks[0] in PREFIXES):
        toks = toks[1:]
    return toks


def _arguments(toks: list[str]) -> list[str]:
    """인자 값. `--flag=value`의 값 부분도 포함한다."""
    values: list[str] = []
    for tok in toks[1:]:
        if tok.startswith("-"):
            if "=" in tok:
                values.append(tok.split("=", 1)[1])
            continue
        values.append(tok)
    return values


def blocked_command(command: str, root: Path) -> str | None:
    """셸 명령을 막는 이유, 없으면 None."""
    for segment in _SEGMENT_SPLIT_RE.split(command):
        toks = _tokens(segment)
        if not toks:
            continue
        verb = os.path.basename(toks[0])
        args = _arguments(toks)
        if verb in TX_TOOLS:
            return REASON_TX
        if verb in TX_WHEN and set(toks[1:]) & TX_WHEN[verb]:
            return REASON_TX
        # 설치 명령은 이름만으로, 그 밖에는 경로나 URL로 쓰였을 때만 본다 — `rg srsRAN docs`
        # 같은 검색까지 막지 않으려고.
        installs = verb in INSTALL_VERBS
        if any(
            ((installs or "/" in arg or verb in ("cd", "pushd")) and is_decoder_path(arg))
            or ("://" in arg and is_decoder_url(arg))
            for arg in args
        ):
            return REASON_DECODER
        if verb == "gh" and any("/" in arg and is_decoder_url("https://github.com/" + arg) for arg in args):
            return REASON_DECODER
        if verb == "git" and any(capture_suffix(arg) for arg in args):
            return REASON_COMMIT
        if verb in FILE_READ_VERBS and any(is_foreign_capture(arg, root) for arg in args):
            return REASON_CAPTURE
    return None


def judge(payload: dict[str, Any], root: Path) -> str | None:
    """도구 호출을 막는 이유, 없으면 None. 페이로드에 대해 순수하다."""
    for raw in extract_paths(payload):
        if is_foreign_capture(raw, root):
            return REASON_CAPTURE
        if is_decoder_path(raw):
            return REASON_DECODER
    for url in extract_urls(payload):
        if is_decoder_url(url):
            return REASON_DECODER

    written = extract_written_text(payload)
    if written:
        macs = foreign_macs(written)
        if macs:
            return REASON_MAC.format(macs=", ".join(macs[:5]))

    command = extract_command(payload)
    # Codex apply_patch 본문은 셸 명령이 아니다. 경로는 위에서 이미 검사했다.
    if command and not looks_like_patch(command):
        return blocked_command(command, root)
    return None


def scan_tracked(root: Path) -> list[str]:
    """§3을 어기는 추적 파일. 발견마다 한 줄."""
    result = subprocess.run(
        ["git", "-C", str(root), "ls-files", "-z"],
        capture_output=True,
        timeout=30,
        check=False,
    )
    if result.returncode != 0:
        return [f"  git ls-files 실패: {result.stderr.decode('utf-8', 'replace').strip()}"]
    findings: list[str] = []
    for rel in (p for p in result.stdout.decode("utf-8", "surrogateescape").split("\0") if p):
        if capture_suffix(rel):
            findings.append(f"  {rel} — 캡처 파일이 추적되고 있다 (§3.3)")
        if any(_DECODER_COMPONENT_RE.match(part) for part in Path(rel).parts[:-1]):
            findings.append(f"  {rel} — 디코더 프로젝트 이름의 디렉터리 안에 있다 (§3.2)")
        if Path(rel).suffix.lower() in SECRET_SUFFIXES:
            findings.append(f"  {rel} — 서명 인증서·키가 추적되고 있다 (§7)")
    return findings


def main() -> None:
    root = hook_root()
    if has_flag("--scan-tracked"):
        findings = scan_tracked(root)
        if findings:
            print("수동성 스캔 실패:\n" + "\n".join(findings), file=sys.stderr)
            raise SystemExit(1)
        print("수동성 스캔: 모두 통과")
        raise SystemExit(0)

    tool = parse_args()
    payload = read_payload()
    try:
        reason = judge(payload, root)
    except Exception as exc:
        emit_deny(tool, f"차단: 수동성 가드 오류로 닫힌 쪽으로 실패함: {exc!r}", payload)
        return
    if reason:
        emit_deny(tool, reason, payload)
    emit_allow(tool)


if __name__ == "__main__":
    main()
