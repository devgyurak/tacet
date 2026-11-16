#!/usr/bin/env python3
"""에이전트 설정의 무결성 검사.

정본(AGENTS.md), 두 하네스(Claude Code · Codex)의 진입점, 서브에이전트, 스킬, 규칙, 훅이
서로 맞는지 확인한다. 끊어진 포인터나 형식·본문 불일치는 기여자의 컴퓨터가 아니라 여기와
CI에서 실패해야 한다.

표준 라이브러리만 쓴다. `tomllib` 때문에 Python 3.11 이상이 필요하다.

    python3 scripts/check_agent_config.py

검사:
  1. 정본 파일이 있다. CLAUDE.md가 `@AGENTS.md`를 가져온다.
  2. `.claude/skills/*/SKILL.md`에 `name`(== 디렉터리)과 `description`이 있다.
  3. `.claude/agents/*.md`에 `name`, `description`, `tools`가 있다.
  4. `.codex/agents/*.toml`이 Claude 집합과 같고, description과 본문이 글자 그대로 같다.
     쓰기 도구가 없는 서브에이전트는 Codex에서 `sandbox_mode = "read-only"`다.
  5. `.claude/rules/*.md`에 `paths`가 있고 본문이 AGENTS.md 절을 가리킨다.
  6. 두 훅 설정이 참조하는 스크립트가 있고, 모든 훅 스크립트가 두 설정에 다 연결돼 있다.
  7. `.agents/skills`가 `.claude/skills`를 가리킨다.
  8. 서브에이전트·스킬·규칙 이름이 CLAUDE.md와 .agents/README.md에 나온다.
  9. AGENTS.md가 적은 목록이 훅이 강제하는 목록과 같다: core 금지 크레이트(§2), 송신 도구
     (§3.1), 디코더 프로젝트(§3.2), 캡처 확장자(§3.3).
 10. docs/STATUS.md에 `▶`(현재 단계) 줄이 정확히 하나 있다.
 11. AGENTS.md가 Codex의 project_doc_max_bytes 안에 들어간다.
 12. AGENTS.md §5.9의 커버리지 하한이 scripts/verify.sh의 COVERAGE_FLOOR와 같다.
 13. 권한 정책이 하나다: .claude/settings.json의 ask/deny가 .codex/rules의 prompt/forbidden에
     모두 있고, 송신 도구는 두 곳 모두에서 막혀 있다.
"""

from __future__ import annotations

import json
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts" / "agent_hooks"))

FAILURES: list[str] = []

HOOK_SCRIPT_RE = re.compile(r"scripts/agent_hooks/([A-Za-z0-9_]+\.py)")
BACKTICKED_RE = re.compile(r"`([^`]+)`")
_RULE_RE = re.compile(r'prefix_rule\(\s*pattern\s*=\s*\[([^\]]*)\]\s*,\s*decision\s*=\s*"(\w+)"', re.DOTALL)
_QUOTED_RE = re.compile(r'"([^"]*)"')
_BASH_PERM_RE = re.compile(r"^Bash\((.+?)(?::\*)?\)$")

CORE_CRATES_MARKER = "**core 금지 크레이트:**"
TX_MARKER = "**송신 도구:**"
DECODER_MARKER = "**디코더 프로젝트:**"
CAPTURE_MARKER = "**캡처 확장자:**"
COVERAGE_MARKER = "**커버리지 하한:**"
_CANON_FLOOR_RE = re.compile(r"`(\d+)%`")
_SCRIPT_FLOOR_RE = re.compile(r"^COVERAGE_FLOOR=(\d+)\s*$", re.MULTILINE)

CANON_FILES = (
    "AGENTS.md",
    "CLAUDE.md",
    "README.md",
    ".agents/README.md",
    ".claude/settings.json",
    ".codex/config.toml",
    ".codex/hooks.json",
    ".codex/rules/default.rules",
    "docs/DESIGN.md",
    "docs/STATUS.md",
    "docs/rules/README.md",
    "docs/platform/README.md",
    "docs/field/README.md",
    ".github/ISSUE_TEMPLATE/decision.md",
    ".github/pull_request_template.md",
    "scripts/verify.sh",
    "deny.toml",
)

HOOK_CONFIGS = (".claude/settings.json", ".codex/hooks.json")
WRITE_TOOLS = frozenset({"Write", "Edit", "MultiEdit", "NotebookEdit"})


def fail(message: str) -> None:
    FAILURES.append(message)


def parse_frontmatter(text: str) -> tuple[dict[str, str], str]:
    """(최상위 프런트매터 키, 프런트매터 뒤 본문)."""
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return {}, text
    keys: dict[str, str] = {}
    end = None
    for index in range(1, len(lines)):
        line = lines[index]
        if line.strip() == "---":
            end = index
            break
        if line[:1] not in (" ", "\t", "-") and ":" in line:
            key, _, value = line.partition(":")
            keys[key.strip()] = value.strip()
    if end is None:
        return {}, text
    return keys, "\n".join(lines[end + 1 :]).strip()


def list_mismatch(text: str, marker: str, expected: set[str] | frozenset[str], label: str) -> str | None:
    """``marker``가 있는 한 줄의 백틱 이름들을 비교한다. 대소문자는 무시. 순수."""
    lines = [line for line in text.splitlines() if marker in line]
    if len(lines) != 1:
        return f"AGENTS.md는 {label}을(를) {marker!r}가 있는 한 줄에 정확히 한 번 적어야 한다 (찾은 수 {len(lines)})"
    stated = {name.lower() for name in BACKTICKED_RE.findall(lines[0].split(marker, 1)[1])}
    wanted = {name.lower() for name in expected}
    if stated != wanted:
        return f"AGENTS.md의 {label}이(가) 훅과 다르다: 빠짐 {sorted(wanted - stated)}, 더 있음 {sorted(stated - wanted)}"
    return None


def coverage_mismatch(agents_text: str, verify_text: str) -> str | None:
    """AGENTS.md가 적은 하한과 verify.sh가 강제하는 하한을 비교한다. 순수."""
    lines = [line for line in agents_text.splitlines() if COVERAGE_MARKER in line]
    canon = _CANON_FLOOR_RE.findall(lines[0].split(COVERAGE_MARKER, 1)[1]) if len(lines) == 1 else []
    script = _SCRIPT_FLOOR_RE.findall(verify_text)
    if len(canon) != 1:
        return f"AGENTS.md는 커버리지 하한을 {COVERAGE_MARKER} `NN%` 형식으로 한 번 적어야 한다"
    if len(script) != 1:
        return "scripts/verify.sh는 COVERAGE_FLOOR=NN을 정확히 한 번 정해야 한다"
    if canon != script:
        return f"커버리지 하한이 다르다: AGENTS.md는 {canon[0]}%, scripts/verify.sh는 {script[0]}%"
    return None


def codex_rules(rules_text: str) -> set[tuple[tuple[str, ...], str]]:
    """Codex 규칙 파일의 (명령 접두사, 결정) 집합. 순수."""
    return {
        (tuple(_QUOTED_RE.findall(pattern)), decision) for pattern, decision in _RULE_RE.findall(rules_text)
    }


def permission_mismatch(settings: dict, rules_text: str) -> list[str]:
    """Claude의 ask/deny 중 Codex 규칙에 prompt/forbidden으로 없는 것. 순수."""
    rules = codex_rules(rules_text)
    missing: list[str] = []
    perms = settings.get("permissions", {})
    for kind, decision in (("ask", "prompt"), ("deny", "forbidden")):
        for entry in perms.get(kind, []):
            match = _BASH_PERM_RE.match(entry)
            if not match:
                continue
            prefix = tuple(match.group(1).split())
            if (prefix, decision) not in rules:
                missing.append(f"Claude {kind} `{match.group(1)}`이(가) .codex/rules에 decision=\"{decision}\"로 없다")
    return missing


def check_canon() -> dict:
    for rel in CANON_FILES:
        if not (ROOT / rel).is_file():
            fail(f"정본 파일 없음: {rel}")
    claude = ROOT / "CLAUDE.md"
    if claude.is_file() and "@AGENTS.md" not in claude.read_text(encoding="utf-8"):
        fail("CLAUDE.md가 @AGENTS.md를 가져오지 않는다")

    config: dict = {}
    path = ROOT / ".codex" / "config.toml"
    if path.is_file():
        config = tomllib.loads(path.read_text(encoding="utf-8"))
    budget = int(config.get("project_doc_max_bytes", 32 * 1024))
    agents = ROOT / "AGENTS.md"
    if agents.is_file() and agents.stat().st_size > budget:
        fail(f"AGENTS.md가 {agents.stat().st_size}바이트다. Codex는 {budget}바이트까지만 읽는다")
    return config


def _skills() -> list[str]:
    names: list[str] = []
    dirs = sorted((ROOT / ".claude" / "skills").glob("*/SKILL.md"))
    if not dirs:
        fail(".claude/skills/에 스킬이 없다")
    for skill in dirs:
        keys, _ = parse_frontmatter(skill.read_text(encoding="utf-8"))
        if keys.get("name") != skill.parent.name:
            fail(f"스킬 이름 불일치: {skill}의 name={keys.get('name')!r}, 디렉터리={skill.parent.name!r}")
        if not keys.get("description"):
            fail(f"스킬에 description 없음: {skill}")
        names.append(skill.parent.name)
    return names


def _claude_agents() -> tuple[dict[str, dict[str, str]], dict[str, str]]:
    agents: dict[str, dict[str, str]] = {}
    bodies: dict[str, str] = {}
    for path in sorted((ROOT / ".claude" / "agents").glob("*.md")):
        keys, body = parse_frontmatter(path.read_text(encoding="utf-8"))
        name = keys.get("name")
        if not name:
            fail(f"Claude 서브에이전트에 name 없음: {path}")
            continue
        if name != path.stem:
            fail(f"Claude 서브에이전트 {path.name}의 name={name!r}. 파일 이름과 같아야 한다")
        for required in ("description", "tools"):
            if not keys.get(required):
                fail(f"Claude 서브에이전트 {name}에 {required} 없음")
        agents[name] = keys
        bodies[name] = body
    if not agents:
        fail(".claude/agents/에 서브에이전트가 없다")
    return agents, bodies


def check_codex_agents(claude: dict[str, dict[str, str]], bodies: dict[str, str]) -> None:
    expected = {name.replace("-", "_") for name in claude}
    seen: set[str] = set()
    for path in sorted((ROOT / ".codex" / "agents").glob("*.toml")):
        data = tomllib.loads(path.read_text(encoding="utf-8"))
        name = data.get("name")
        if not name:
            fail(f"Codex 서브에이전트에 name 없음: {path}")
            continue
        seen.add(name)
        claude_name = name.replace("_", "-")
        keys = claude.get(claude_name)
        if keys is None:
            continue
        if data.get("description", "").strip() != keys.get("description", ""):
            fail(f"Codex {name}의 description이 Claude와 다르다")
        if data.get("developer_instructions", "").strip() != bodies[claude_name]:
            fail(f"Codex {name}의 본문이 Claude 본문과 다르다 (글자 그대로 같아야 한다)")
        tools = {t.strip() for t in keys.get("tools", "").split(",")}
        if not tools & WRITE_TOOLS and data.get("sandbox_mode") != "read-only":
            fail(f"Codex {name}: Claude에서 읽기 전용이므로 sandbox_mode = \"read-only\"여야 한다")
    if seen != expected:
        fail(f"Codex 서브에이전트 집합 {sorted(seen)} != 기대 {sorted(expected)}")


def _claude_rules() -> list[str]:
    names: list[str] = []
    for path in sorted((ROOT / ".claude" / "rules").glob("*.md")):
        keys, body = parse_frontmatter(path.read_text(encoding="utf-8"))
        if "paths" not in keys:
            fail(f"Claude 규칙에 paths 없음: {path}")
        if "AGENTS.md §" not in body:
            fail(f"Claude 규칙 {path.name}이 정본(AGENTS.md §N)을 가리키지 않는다")
        names.append(path.name)
    if not names:
        fail(".claude/rules/에 규칙이 없다")
    return names


def _collect_hook_scripts(node: object, found: set[str]) -> None:
    if isinstance(node, dict):
        for value in node.values():
            _collect_hook_scripts(value, found)
    elif isinstance(node, list):
        for item in node:
            _collect_hook_scripts(item, found)
    elif isinstance(node, str):
        found.update(HOOK_SCRIPT_RE.findall(node))


def check_hooks() -> None:
    scripts = {
        p.name
        for p in (ROOT / "scripts" / "agent_hooks").glob("*.py")
        if not p.name.startswith("_") and p.name != "selftest.py"
    }
    for rel in HOOK_CONFIGS:
        path = ROOT / rel
        if not path.is_file():
            continue
        referenced: set[str] = set()
        _collect_hook_scripts(json.loads(path.read_text(encoding="utf-8")), referenced)
        for name in sorted(referenced - scripts):
            fail(f"{rel}이(가) 없는 스크립트를 참조한다: scripts/agent_hooks/{name}")
        for name in sorted(scripts - referenced):
            fail(f"훅 스크립트가 {rel}에 연결돼 있지 않다: scripts/agent_hooks/{name}")


def check_skills_symlink() -> None:
    link = ROOT / ".agents" / "skills"
    if not link.exists():
        fail(".agents/skills가 없다 (Codex가 읽는다. ../.claude/skills를 가리키는 심볼릭 링크여야 한다)")
        return
    if not link.is_symlink():
        fail(".agents/skills가 심볼릭 링크가 아니다. ../.claude/skills를 가리켜야 한다")
        return
    if link.resolve() != (ROOT / ".claude" / "skills").resolve():
        fail(f".agents/skills가 {link.resolve()}를 가리킨다. .claude/skills여야 한다")


def check_names_documented(names: list[str], kind: str, docs: tuple[str, ...]) -> None:
    for doc in docs:
        path = ROOT / doc
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for name in names:
            if f"`{name}`" not in text and f"**{name}**" not in text and name not in text:
                fail(f"{doc}에 {kind} {name!r}이(가) 나오지 않는다")


def check_gate_lists() -> None:
    agents = ROOT / "AGENTS.md"
    if not agents.is_file():
        return
    import check_architecture
    import guard_passive

    text = agents.read_text(encoding="utf-8")
    for message in (
        list_mismatch(text, CORE_CRATES_MARKER, check_architecture.CORE_FORBIDDEN_CRATES, "core 금지 크레이트 목록"),
        list_mismatch(text, TX_MARKER, guard_passive.TX_TOOLS, "송신 도구 목록"),
        list_mismatch(text, DECODER_MARKER, guard_passive.DECODER_PROJECTS, "디코더 프로젝트 목록"),
        list_mismatch(text, CAPTURE_MARKER, guard_passive.CAPTURE_SUFFIXES, "캡처 확장자 목록"),
    ):
        if message:
            fail(message)


def check_status() -> None:
    status = ROOT / "docs" / "STATUS.md"
    if not status.is_file():
        return
    import session_context

    if session_context.current_phase(status.read_text(encoding="utf-8")) is None:
        fail("docs/STATUS.md에 현재 단계를 적은 `▶` 줄이 정확히 하나 있어야 한다")


def check_coverage_floor() -> None:
    agents, verify = ROOT / "AGENTS.md", ROOT / "scripts" / "verify.sh"
    if agents.is_file() and verify.is_file():
        message = coverage_mismatch(agents.read_text(encoding="utf-8"), verify.read_text(encoding="utf-8"))
        if message:
            fail(message)


def check_permissions() -> None:
    settings_path = ROOT / ".claude" / "settings.json"
    rules_files = sorted((ROOT / ".codex" / "rules").glob("*.rules"))
    if not settings_path.is_file() or not rules_files:
        return
    import guard_passive

    settings = json.loads(settings_path.read_text(encoding="utf-8"))
    rules_text = "\n".join(p.read_text(encoding="utf-8") for p in rules_files)
    for message in permission_mismatch(settings, rules_text):
        fail(message)
    deny = set(settings.get("permissions", {}).get("deny", []))
    rules = codex_rules(rules_text)
    for tool in sorted(guard_passive.TX_TOOLS):
        if f"Bash({tool}:*)" not in deny:
            fail(f"송신 도구 {tool}이(가) .claude/settings.json deny에 없다")
        if ((tool,), "forbidden") not in rules:
            fail(f"송신 도구 {tool}이(가) .codex/rules에 forbidden으로 없다")


def main() -> int:
    check_canon()
    skills = _skills()
    claude, bodies = _claude_agents()
    check_codex_agents(claude, bodies)
    rules = _claude_rules()
    check_hooks()
    check_skills_symlink()
    docs = ("CLAUDE.md", ".agents/README.md")
    check_names_documented(sorted(claude), "서브에이전트", docs)
    check_names_documented(skills, "스킬", docs)
    check_names_documented(rules, "규칙", (".agents/README.md",))
    check_gate_lists()
    check_status()
    check_coverage_floor()
    check_permissions()

    if FAILURES:
        print("에이전트 설정 검사 실패:", file=sys.stderr)
        for failure in FAILURES:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    print("에이전트 설정 검사: 모두 통과")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
