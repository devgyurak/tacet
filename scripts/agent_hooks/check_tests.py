#!/usr/bin/env python3
"""편집 후 훅(그리고 CI 게이트): Rust 테스트 규칙 (AGENTS.md §5).

편집한 `.rs` 파일의 `#[test]` 류 함수마다:

  * 이름이 `given_<상태>_when_<동작>_then_<관찰값>`이고, 마지막 `_then_` 뒤가 `works` /
    `ok` / `passes`가 아니라 관찰할 수 있는 결과다.
  * 본문에 제어 흐름이 없다 — `if` / `else` / `for` / `while` / `loop` / `match`(그래서
    `let … else`도) / `for_each`. 여러 사례는 실패 목록을 돌려주는 하네스 함수로 돌리고,
    테스트는 그 목록이 비었음을 단언한다.
  * 무언가를 단언한다(`assert*!`, `prop_assert*!`, insta `assert_*_snapshot!`, 이름이
    `assert_*`인 도우미). 아니면 `#[should_panic(expected = …)]`.
  * `crates/` 아래에서 테스트는 모듈의 `tests.rs`(또는 `tests/mod.rs` 옆의
    `tests/<주제>.rs`)에 있다. 인라인 `mod tests { … }`와 `tests_<주제>.rs` 형제 파일은 안 된다.

모드: 기본(훅 페이로드의 경로), `--all`(모든 `.rs`, 발견하면 0이 아닌 값으로 종료 — CI).
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from _hookio import has_flag, hook_root, rel_path, repo_files, run_all, run_post_hook
from _rustscan import strip, test_functions

GWT_RE = re.compile(r"^given_[a-z0-9_]+_when_[a-z0-9_]+_then_[a-z0-9_]+$")

# 관찰할 결과를 말하지 않는 `then_`은 읽는 사람이 확인할 것이 없다.
VAGUE_THEN = frozenset(
    {"works", "work", "ok", "okay", "success", "succeeds", "pass", "passes", "good", "correct", "fine", "valid", "done"}
)

CONTROL_FLOW_RE = re.compile(r"\b(if|else|for|while|loop|match|for_each|try_for_each)\b")
ASSERT_RE = re.compile(r"\b(?:prop_)?assert\w*!|\bassert_\w+\s*\(|\binsta::assert_\w+!")

# 인라인 테스트 모듈: `mod tests {` 또는 `mod tests_<주제> {`. `mod tests;`(파일)는 괜찮다.
INLINE_TESTS_RE = re.compile(r"^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?mod[ \t]+tests\w*[ \t]*\{", re.MULTILINE)
SIBLING_TESTS_FILE_RE = re.compile(r"^tests_\w+\.rs$")

RULE = (
    "테스트 규칙 (AGENTS.md §5): 이름은 given_<상태>_when_<동작>_then_<관찰값>; 테스트 본문에 "
    "제어 흐름을 두지 않는다(반복은 실패 목록을 돌려주는 하네스 함수로 옮기고 목록이 비었음을 "
    "단언); 모든 테스트는 무언가를 단언한다; 모듈의 테스트는 tests.rs(또는 tests/<주제>.rs)에 "
    "두고 인라인이나 tests_<주제>.rs 형제 파일로 두지 않는다."
)


def check_source(source: str, label: str) -> list[str]:
    """Rust 소스 한 개의 규칙 위반. 순수해서 selftest가 텍스트를 바로 먹일 수 있다."""
    violations: list[str] = []
    for test in test_functions(strip(source)):
        where = f"  {label}:{test.line} `{test.name}`"
        if not GWT_RE.match(test.name):
            violations.append(f"{where} — given_<상태>_when_<동작>_then_<관찰값> 형식이 아니다.")
        else:
            then_part = test.name.rsplit("_then_", 1)[1]
            if then_part in VAGUE_THEN:
                violations.append(f"{where} — `then_{then_part}`은 관찰할 수 있는 결과가 아니다.")
        flow = sorted({m.group(1) for m in CONTROL_FLOW_RE.finditer(test.body)})
        if flow:
            violations.append(f"{where} — 테스트 본문에 제어 흐름: {', '.join(flow)}.")
        if not ASSERT_RE.search(test.body) and "should_panic" not in test.attrs:
            violations.append(f"{where} — 아무것도 단언하지 않는다.")
    violations.extend(check_placement(source, label))
    return violations


def check_placement(source: str, label: str) -> list[str]:
    """`crates/` 아래 파일의 테스트 위치. check_source처럼 순수하다."""
    if not label.startswith("crates/"):
        return []
    violations: list[str] = []
    if SIBLING_TESTS_FILE_RE.match(label.rsplit("/", 1)[-1]):
        violations.append(f"  {label} — `tests_<주제>.rs` 형제 파일. 모듈의 tests.rs나 tests/<주제>.rs를 쓴다.")
    for match in INLINE_TESTS_RE.finditer(strip(source)):
        line = source.count("\n", 0, match.start()) + 1
        violations.append(f"  {label}:{line} — 인라인 테스트 모듈. 모듈의 tests.rs로 옮긴다.")
    return violations


def check_file(path: Path) -> list[str]:
    if path.suffix != ".rs" or not path.is_file():
        return []
    try:
        source = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return []
    label = rel_path(path, hook_root())
    if label is None:
        return []  # 저장소 밖
    return check_source(source, label)


def main() -> None:
    if has_flag("--all"):
        run_all(check_file, repo_files(hook_root(), ".rs"), "테스트 규칙 검사", RULE)
    run_post_hook(check_file, "테스트 규칙 위반", RULE)


if __name__ == "__main__":
    main()
