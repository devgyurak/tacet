#!/usr/bin/env python3
"""편집 후 훅(그리고 CI 게이트): 아키텍처 규칙 (AGENTS.md §2).

Rust 소스:
  * `crates/core/src/**`는 순수 core다. IO, 시계, async, 난수, 출력, 시드가 프로세스마다
    다른 해시 컨테이너, 전역 가변 상태, OS별 `cfg`가 없다. 한 건은 그 줄이나 바로 윗줄의
    `// PURITY-OK: <이유>`로 면제할 수 있고, 리뷰어가 그 이유에 동의해야 한다.
  * `crates/net/src/**`, `crates/dashboard/src/**`도 OS별 `cfg`를 쓰지 않는다 — 플랫폼 차이는
    수집기와 앱(`crates/app/`)의 몫이다.
  * 작업 공간 어디서든 `unsafe`는 그 줄이나 위 세 줄 안에 `SAFETY:` 주석을 단다.

Cargo 매니페스트:
  * `<접두사>-core`는 작업 공간 크레이트에 의존하지 않고 CORE_FORBIDDEN_CRATES에도 의존하지 않는다.
  * 수집기(`<접두사>-collector-*`), `<접두사>-net`, `<접두사>-dashboard`는 작업 공간 안에서
    core에만 의존한다.
  * 아무것도 앱 패키지(`<접두사>`)와 `<접두사>-devtools`에 의존하지 않는다.
  * 패키지·lib·bin 이름은 `<접두사>` / `<접두사>-*`다.

프로젝트 이름은 Tacet이다(docs/STATUS.md). 접두사는 PREFIX 한 곳에만 있다.

모드: 기본(훅 페이로드의 경로), `--all`(저장소 전체, 발견하면 0이 아닌 값으로 종료 — CI).

판단 로직은 여기에만 있다. scripts/check_agent_config.py가 CORE_FORBIDDEN_CRATES를 가져와
AGENTS.md가 같은 목록을 적는지 확인한다.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from _hookio import has_flag, hook_root, rel_path, repo_files, run_all, run_post_hook
from _rustscan import line_of, strip

# 프로젝트 이름(Tacet). 바꾸려면 이 값과 Cargo.toml을 함께 바꾼다.
PREFIX = "tacet"
CORE_PACKAGE = f"{PREFIX}-core"
APP_PACKAGE = PREFIX
DEVTOOLS_PACKAGE = f"{PREFIX}-devtools"
COLLECTOR_PREFIX = f"{PREFIX}-collector-"
# 작업 공간 안에서 core에만 의존하는 패키지(수집기는 접두사로 따로 본다).
CORE_ONLY_PACKAGES = frozenset({f"{PREFIX}-net", f"{PREFIX}-dashboard"})
# 아무도 의존하면 안 되는 끝 패키지.
LEAF_PACKAGES = frozenset({APP_PACKAGE, DEVTOOLS_PACKAGE})
# OS별 `cfg`를 쓰면 안 되는 소스 경로.
NO_OS_CFG_ROOTS = ("crates/net/src/", "crates/dashboard/src/")

# core가 의존하면 안 되는 크레이트: 비동기 런타임, 무선 칩·USB·OS API, SDR·신호 처리, 네트워크·
# TLS·앱 프레임워크, 난수, 벽시계, 바이너리 쪽 오류 타입. AGENTS.md §2 규칙 1이 같은 목록을 적고, 둘이
# 다르면 scripts/check_agent_config.py가 실패한다.
CORE_FORBIDDEN_CRATES = frozenset(
    {
        "anyhow",
        "async-trait",
        "axum",
        "bluer",
        "btleplug",
        "chrono",
        "eframe",
        "egui",
        "getrandom",
        "libc",
        "mdns-sd",
        "nix",
        "objc2",
        "objc2-core-location",
        "objc2-core-wlan",
        "rand",
        "rusb",
        "rustfft",
        "rustls",
        "soapysdr",
        "tauri",
        "time",
        "tokio",
        "tokio-tungstenite",
        "windows",
        "windows-sys",
    }
)

CORE_FORBIDDEN_CODE: tuple[tuple[re.Pattern[str], str], ...] = (
    (re.compile(r"\bstd::(?:fs|net|process|thread|env|io)\b"), "std IO·프로세스·스레드·환경 접근"),
    (re.compile(r"\b(?:SystemTime|Instant|UNIX_EPOCH)\b"), "시계 — 시각은 관측값의 `Timestamp`로 받는다"),
    (re.compile(r"\b(?:print|println|eprint|eprintln|dbg)!"), "표준 출력/오류 출력"),
    (re.compile(r"\b(?:HashMap|HashSet|RandomState)\b"), "프로세스마다 시드가 다른 해시 컨테이너"),
    (re.compile(r"\bstatic\s+mut\b|\bthread_local!"), "전역 가변 상태"),
    (re.compile(r"\basync\b|\.await\b"), "async — core는 동기 순수 함수다"),
    (re.compile(r"\brand::|\bthread_rng\b"), "난수"),
    (re.compile(r"\bcfg\s*\(\s*(?:[^)]*\b)?(?:target_os|windows|unix)\b"), "OS별 분기 — 플랫폼 차이는 수집기와 앱의 몫"),
)
NO_OS_CFG_CODE: tuple[tuple[re.Pattern[str], str], ...] = (CORE_FORBIDDEN_CODE[-1],)
_STD_GROUP_RE = re.compile(r"\bstd::\{([^}]*)\}")
_STD_GROUP_BANNED = frozenset({"fs", "net", "process", "thread", "env", "io"})
_UNSAFE_RE = re.compile(r"\bunsafe\b")

WHY_CORE = (
    "core는 IO가 없고 결정적이다 (AGENTS.md §2): 입력은 관측값(`Observation`)과 설정, 출력은 "
    "적중·점수·위치 판정·알림 후보뿐이다. 같은 관측 열이면 언제나 같은 결과가 나와야 테스트와 "
    "재현이 된다. IO·async·OS API는 수집기·net·앱으로 옮기고, 컬렉션은 Vec·BTreeMap을 쓴다."
)
WHY_UNSAFE = "`unsafe`에는 건전성을 논증하는 `// SAFETY:` 주석을 단다 (AGENTS.md §2 규칙 9)."
WHY_MANIFEST = (
    "크레이트 의존은 core 쪽으로만 향한다 (AGENTS.md §2). 수집기·net·dashboard는 서로 모르고 core만 "
    f"안다. 앱과 devtools에는 아무도 의존하지 않는다. 이름은 `{PREFIX}` / `{PREFIX}-*`다."
)


def _waived(lines: list[str], line: int, marker: str) -> bool:
    here = lines[line - 1] if 0 < line <= len(lines) else ""
    above = lines[line - 2] if 1 < line <= len(lines) + 1 else ""
    return marker in here or marker in above


def check_rust_source(source: str, label: str) -> list[str]:
    """Rust 소스 한 개의 위반. 순수해서 selftest가 텍스트를 바로 먹일 수 있다."""
    stripped = strip(source)
    lines = source.splitlines()
    violations: list[str] = []

    rules: tuple[tuple[re.Pattern[str], str], ...] = ()
    scope = ""
    if label.startswith("crates/core/src/"):
        rules, scope = CORE_FORBIDDEN_CODE, "core"
    elif label.startswith(NO_OS_CFG_ROOTS):
        rules, scope = NO_OS_CFG_CODE, label.split("/")[1]
    for pattern, what in rules:
        for match in pattern.finditer(stripped):
            line = line_of(stripped, match.start())
            if not _waived(lines, line, "PURITY-OK:"):
                violations.append(f"  {label}:{line} — {scope}에 `{match.group(0).strip()}`: {what}.")
    if scope == "core":
        for group in _STD_GROUP_RE.finditer(stripped):
            names = {part.strip().split("::", 1)[0] for part in group.group(1).split(",")}
            line = line_of(stripped, group.start())
            banned = sorted(names & _STD_GROUP_BANNED)
            if banned and not _waived(lines, line, "PURITY-OK:"):
                violations.append(f"  {label}:{line} — core에 `std::{{{', '.join(banned)}}}`: std IO 접근.")

    for match in _UNSAFE_RE.finditer(stripped):
        line = line_of(stripped, match.start())
        context = lines[max(0, line - 4) : line]
        if not any("SAFETY:" in text or "# Safety" in text for text in context):
            violations.append(f"  {label}:{line} — `// SAFETY:` 주석 없는 `unsafe`.")
    return violations


# --- Cargo.toml (작은 줄 단위 파서: 훅은 Python 3.9에서 tomllib 없이 돌아야 한다) -----

_HEADER_RE = re.compile(r"^\s*\[\[?\s*([^\]]+?)\s*\]\]?\s*(?:#.*)?$")
_DEP_TABLE_RE = re.compile(
    r"""^(?:target\.(?:'[^']*'|"[^"]*"|[^.]+)\.)?((?:dev-|build-)?dependencies)(?:\.(.+))?$"""
)
_KEY_RE = re.compile(r'^\s*("([^"]+)"|[A-Za-z0-9_\-]+)\s*=\s*(.*)$')
_STR_VALUE_RE = re.compile(r"""^\s*["']([^"']*)["']""")
_PACKAGE_RE = re.compile(r"""\bpackage\s*=\s*["']([^"']+)["']""")
_WORKSPACE_TRUE_RE = re.compile(r"\bworkspace\s*=\s*true\b")


class Manifest:
    def __init__(self) -> None:
        self.package: str | None = None
        self.lib: str | None = None
        self.bins: list[str] = []
        # (의존 종류, 실제 패키지 이름, 줄, 작업 공간에서 상속했는지)
        self.deps: list[tuple[str, str, int, bool]] = []
        # [workspace.dependencies] 키 -> 실제 패키지 이름
        self.workspace_deps: dict[str, str] = {}


def parse_manifest(text: str) -> Manifest:
    manifest = Manifest()
    table = ""
    pending: tuple[str, str, int] | None = None  # 읽고 있는 [dependencies.foo] 테이블

    def close_pending() -> None:
        nonlocal pending
        if pending is not None:
            kind, name, line = pending
            manifest.deps.append((kind, name, line, False))
            pending = None

    for lineno, raw in enumerate(text.splitlines(), start=1):
        header = _HEADER_RE.match(raw)
        if header:
            close_pending()
            table = header.group(1).strip()
            dep_table = _DEP_TABLE_RE.match(table)
            if dep_table and dep_table.group(2):
                pending = (dep_table.group(1), dep_table.group(2).strip().strip("\"'"), lineno)
            if table == "bin":
                manifest.bins.append("")
            continue
        key = _KEY_RE.match(raw)
        if key is None:
            continue
        name = key.group(2) or key.group(1)
        value = key.group(3)
        if pending is not None:
            if name == "package":
                literal = _STR_VALUE_RE.match(value)
                if literal:
                    pending = (pending[0], literal.group(1), pending[2])
            continue
        if table in ("package", "lib") and name == "name":
            literal = _STR_VALUE_RE.match(value)
            if literal:
                if table == "package":
                    manifest.package = literal.group(1)
                else:
                    manifest.lib = literal.group(1)
        elif table == "bin" and name == "name" and manifest.bins:
            literal = _STR_VALUE_RE.match(value)
            if literal:
                manifest.bins[-1] = literal.group(1)
        elif table == "workspace.dependencies":
            renamed = _PACKAGE_RE.search(value)
            manifest.workspace_deps[name] = renamed.group(1) if renamed else name
        else:
            dep_table = _DEP_TABLE_RE.match(table)
            if dep_table and not dep_table.group(2):
                renamed = _PACKAGE_RE.search(value)
                inherited = bool(_WORKSPACE_TRUE_RE.search(value))
                manifest.deps.append((dep_table.group(1), renamed.group(1) if renamed else name, lineno, inherited))
    close_pending()
    manifest.bins = [b for b in manifest.bins if b]
    return manifest


def _workspace_member(name: str) -> bool:
    return name == PREFIX or name.startswith(PREFIX + "-")


def check_manifest_text(text: str, label: str, workspace_deps: dict[str, str] | None = None) -> list[str]:
    """Cargo.toml 한 개의 위반. 순수해서 selftest가 텍스트를 바로 먹일 수 있다."""
    manifest = parse_manifest(text)
    renames = dict(workspace_deps or {})
    renames.update(manifest.workspace_deps)
    violations: list[str] = []

    for kind, name in (("package", manifest.package), ("lib", manifest.lib), *(("bin", b) for b in manifest.bins)):
        if not name or name == "xtask":
            continue
        if not re.match(rf"^{PREFIX}(?:[-_].+)?$", name):
            violations.append(f"  {label} — {kind} 이름 `{name}`이 `{PREFIX}` / `{PREFIX}-*`가 아니다.")

    package = manifest.package
    if package is None:
        return violations
    for kind, dep, line, inherited in manifest.deps:
        real = renames.get(dep, dep) if inherited else dep
        if kind == "build-dependencies":
            continue
        if package == CORE_PACKAGE and kind == "dependencies" and real in CORE_FORBIDDEN_CRATES:
            violations.append(f"  {label}:{line} — core가 `{real}`에 의존한다 (core 금지 크레이트).")
        if not _workspace_member(real) or real == package:
            continue
        if real in LEAF_PACKAGES:
            violations.append(f"  {label}:{line} — `{package}`가 `{real}`에 의존한다. 앱과 devtools에는 아무것도 의존하지 않는다.")
        elif package == CORE_PACKAGE and kind == "dependencies":
            violations.append(f"  {label}:{line} — core가 작업 공간 크레이트 `{real}`에 의존한다.")
        elif (package.startswith(COLLECTOR_PREFIX) or package in CORE_ONLY_PACKAGES) and real != CORE_PACKAGE:
            violations.append(f"  {label}:{line} — `{package}`는 작업 공간 안에서 core에만 의존한다 (`{real}`).")
    return violations


def _workspace_deps(root: Path) -> dict[str, str]:
    top = root / "Cargo.toml"
    if not top.is_file():
        return {}
    try:
        return parse_manifest(top.read_text(encoding="utf-8")).workspace_deps
    except (OSError, UnicodeDecodeError):
        return {}


def check_file(path: Path) -> list[str]:
    root = hook_root()
    if not path.is_file():
        return []
    rel = rel_path(path, root)
    if rel is None:
        return []
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return []
    if path.name == "Cargo.toml":
        return check_manifest_text(text, rel, _workspace_deps(root))
    if path.suffix == ".rs":
        return check_rust_source(text, rel)
    return []


RULE = f"- {WHY_CORE}\n- {WHY_UNSAFE}\n- {WHY_MANIFEST}"


def main() -> None:
    if has_flag("--all"):
        root = hook_root()
        files = [*repo_files(root, "Cargo.toml"), *repo_files(root, ".rs")]
        run_all(check_file, files, "아키텍처 검사", RULE)
    run_post_hook(check_file, "아키텍처 규칙 위반 (AGENTS.md §2)", RULE)


if __name__ == "__main__":
    main()
