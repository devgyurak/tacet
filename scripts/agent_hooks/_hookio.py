"""에이전트 훅이 함께 쓰는 입출력 도우미.

훅 본문 하나가 Claude Code와 Codex CLI 두 하네스를 모두 섬긴다. stdin 페이로드의 키와
출력 JSON 형식이 조금씩 달라서, 그 차이는 이 파일에서만 흡수한다.

- Claude Code : https://code.claude.com/docs/en/hooks
- Codex CLI   : Claude Code와 호환되는 훅 stdin/출력 형식(.codex/hooks.json 참고).
                `apply_patch`는 `tool_input.command`에 패치 문자열을 넘기므로 경로는
                `*** Add|Update|Delete File:` / `*** Move to:` 머리글에서 읽는다.

차단 방식: 두 도구 모두 결정 JSON을 출력하고 0으로 끝낸다.

표준 라이브러리만 쓰고 Python 3.9에서 돌아야 한다. 훅이 PATH의 macOS /usr/bin/python3로
실행될 수 있고, 옛 문법에서 죽는 훅은 Claude/Codex에서 *열린 채로* 실패한다.
"""

from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path
from typing import Any

CLAUDE = "claude"
CODEX = "codex"
TOOLS = (CLAUDE, CODEX)

_PATCH_FILE_RE = re.compile(r"^\*\*\*\s+(?:Add|Update|Delete)\s+File:\s*(.+?)\s*$", re.MULTILINE)
_PATCH_MOVE_RE = re.compile(r"^\*\*\*\s+Move to:\s*(.+?)\s*$", re.MULTILINE)
_PATCH_MARKERS = ("*** Begin Patch", "*** Add File:", "*** Update File:", "*** Delete File:")
_URL_RE = re.compile(r"""(?:https?://|git@)[^\s'"<>`]+""")


def hook_root() -> Path:
    """훅이 판단 기준으로 삼는 저장소.

    보통은 이 스크립트가 들어 있는 체크아웃이다. selftest는 TACET_HOOK_ROOT로 임시
    트리를 가리킨다. macOS에서 임시 경로가 /var로 잡히고 파일은 /private/var로 풀릴 수
    있어 relative_to()가 실패하므로 resolve한다.
    """
    override = os.environ.get("TACET_HOOK_ROOT")
    return Path(override or Path(__file__).resolve().parents[2]).resolve()


def parse_args() -> str:
    """`--tool <이름>`을 읽는다. 없으면 Claude 형식."""
    tool = parse_option("--tool", CLAUDE) or CLAUDE
    return tool if tool in TOOLS else CLAUDE


def parse_option(name: str, default: str | None = None) -> str | None:
    """argv에서 `--name value` / `--name=value`를 읽고, 없으면 ``default``."""
    argv = sys.argv[1:]
    for i, arg in enumerate(argv):
        if arg == name and i + 1 < len(argv):
            return argv[i + 1]
        if arg.startswith(name + "="):
            return arg.split("=", 1)[1]
    return default


def has_flag(name: str) -> bool:
    return name in sys.argv[1:]


def read_payload() -> dict[str, Any]:
    """stdin의 훅 JSON. 비었거나 깨진 입력이면 빈 dict."""
    try:
        raw = sys.stdin.read()
    except Exception:
        return {}
    if not raw.strip():
        return {}
    try:
        data = json.loads(raw)
    except json.JSONDecodeError:
        return {}
    return data if isinstance(data, dict) else {}


def looks_like_patch(text: str) -> bool:
    """문자열이 셸 명령이 아니라 Codex apply_patch 본문이면 True."""
    return any(marker in text for marker in _PATCH_MARKERS)


def extract_patch_paths(text: str) -> list[str]:
    """Codex apply_patch 본문이 건드릴 파일 경로(`*** ... File:` 머리글)."""
    if not looks_like_patch(text):
        return []
    return [p for p in (*_PATCH_FILE_RE.findall(text), *_PATCH_MOVE_RE.findall(text)) if p]


def _walk(node: Any, keys: tuple[str, ...], out: list[str]) -> None:
    if isinstance(node, dict):
        for key, value in node.items():
            if key in keys and isinstance(value, str) and value:
                out.append(value)
            else:
                _walk(value, keys, out)
    elif isinstance(node, list):
        for item in node:
            _walk(item, keys, out)


def _dedupe(values: list[str]) -> list[str]:
    seen: set[str] = set()
    ordered: list[str] = []
    for value in values:
        if value not in seen:
            seen.add(value)
            ordered.append(value)
    return ordered


def extract_paths(payload: dict[str, Any]) -> list[str]:
    """도구 호출이 건드리는 파일 경로를 키 표기와 상관없이 모은다.

    Claude Code : tool_input.file_path / tool_input.edits[].file_path / tool_input.path (Grep, Glob)
    Codex CLI   : apply_patch 본문 → `*** Add|Update|Delete File: <경로>` 머리글
    """
    found: list[str] = []
    _walk(payload, ("file_path", "filePath", "path", "notebook_path"), found)
    ordered = _dedupe(found)
    if ordered:
        return ordered
    return _dedupe(extract_patch_paths(extract_command(payload)))


def extract_command(payload: dict[str, Any]) -> str:
    """도구가 실행하려는 명령/패치 문자열(Bash / apply_patch / shell)."""
    found: list[str] = []
    _walk(payload, ("command",), found)
    return "\n".join(found)


def extract_urls(payload: dict[str, Any]) -> list[str]:
    """도구 호출이 가져오는 URL: WebFetch의 `url`, 또는 셸 명령 안의 URL."""
    found: list[str] = []
    _walk(payload, ("url",), found)
    command = extract_command(payload)
    if command and not looks_like_patch(command):
        found.extend(_URL_RE.findall(command))
    return _dedupe(found)


def extract_written_text(payload: dict[str, Any]) -> str:
    """쓰기/편집/패치 호출이 디스크에 쓰려는 텍스트(최선 노력)."""
    found: list[str] = []
    _walk(payload, ("content", "new_string", "newString", "patch"), found)
    command = extract_command(payload)
    if command and looks_like_patch(command):
        found.append(command)
    return "\n".join(found)


def event_name(payload: dict[str, Any]) -> str:
    return str(payload.get("hook_event_name") or payload.get("hookEventName") or "")


def emit_allow(tool: str) -> None:
    """아무 조치 없이 통과."""
    sys.exit(0)


def emit_deny(tool: str, reason: str, payload: dict[str, Any]) -> None:
    """시도하려는 동작을 막고 이유를 모델에게 돌려준다(PreToolUse)."""
    print(
        json.dumps(
            {
                "hookSpecificOutput": {
                    "hookEventName": event_name(payload) or "PreToolUse",
                    "permissionDecision": "deny",
                    "permissionDecisionReason": reason,
                },
                "systemMessage": reason,
            },
            ensure_ascii=False,
        )
    )
    sys.exit(0)


def emit_block(tool: str, reason: str) -> None:
    """이미 실행된 동작을 고치라고 모델에게 알린다(PostToolUse)."""
    print(json.dumps({"decision": "block", "reason": reason, "systemMessage": reason}, ensure_ascii=False))
    sys.exit(0)


def emit_note(tool: str, message: str) -> None:
    """막지 않고 정보만 전달한다."""
    print(json.dumps({"suppressOutput": True, "systemMessage": message}, ensure_ascii=False))
    sys.exit(0)


def emit_context(tool: str, text: str) -> None:
    """세션 컨텍스트에 텍스트를 더한다(SessionStart)."""
    print(
        json.dumps(
            {"hookSpecificOutput": {"hookEventName": "SessionStart", "additionalContext": text}},
            ensure_ascii=False,
        )
    )
    sys.exit(0)


def rel_path(path: Path, root: Path) -> str | None:
    """저장소 기준 상대 경로. 저장소 밖이면 None."""
    try:
        return path.resolve().relative_to(root).as_posix()
    except (ValueError, OSError):
        return None


def repo_files(root: Path, suffix: str) -> list[Path]:
    """``root`` 아래에서 ``suffix``로 끝나는 파일 전부. 빌드 산출물과 VCS 메타데이터는 뺀다."""
    skip = {".git", "target", "node_modules", "build", "__pycache__"}
    found: list[Path] = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if d not in skip)
        for name in sorted(filenames):
            if name.endswith(suffix):
                found.append(Path(dirpath) / name)
    return found


def run_post_hook(check_file: Any, label: str, rule: str) -> None:
    """PostToolUse 훅 공통 흐름: 페이로드의 경로를 검사하고, 위반이 있으면 block."""
    tool = parse_args()
    payload = read_payload()
    violations = [v for raw in extract_paths(payload) for v in check_file(Path(raw).expanduser())]
    if not violations:
        emit_allow(tool)
    emit_block(tool, f"{label}:\n" + "\n".join(violations) + f"\n\n{rule}")


def run_all(check_file: Any, files: list[Path], label: str, rule: str) -> None:
    """`--all` 모드 공통 흐름: 파일 전부를 검사하고, 위반이 있으면 0이 아닌 값으로 끝낸다."""
    violations = [v for path in files for v in check_file(path)]
    if violations:
        print(f"{label} 실패:\n" + "\n".join(violations) + f"\n\n{rule}", file=sys.stderr)
        raise SystemExit(1)
    print(f"{label}: 모두 통과")
    raise SystemExit(0)
