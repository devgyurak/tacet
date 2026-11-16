#!/usr/bin/env python3
"""편집 후 훅(그리고 CI 게이트): 한국어 전용 규칙 (AGENTS.md §9).

이 저장소의 글은 한국어다. 식별자, 명령, 경로, 고유명사(BLE, RSSI, btleplug …)는 영어
그대로 둔다. 검사는 "영어로만 쓴 글"을 잡는다. 문장이 좋은 한국어인지는 판단하지 못한다.

  * 번역본 금지 — `<이름>-<언어코드>.md`(`README-EN.md` 등). 한국어 원본 하나만 둔다.
  * Markdown — 코드 블록·인라인 코드·URL을 뺀 문단 중, 영어 단어가 PROSE_WORDS개 이상인데
    한글이 하나도 없는 문단.
  * 코드 주석 — `.rs`의 `//`·`///`·`/* */`, `.py`·`.sh`·`.toml`·`.yml`·`.rules`의 `#` 주석과
    `.py` 독스트링. 연속된 주석 묶음에 영어 단어가 COMMENT_WORDS개 이상인데 한글이 없으면 걸린다.
  * 프런트엔드 주석 — `.ts`·`.tsx`·`.js`·`.jsx`·`.svelte`·`.vue`의 `//`·`/* */` 주석.
  * 사용자 문구 — `crates/app/`, `crates/dashboard/`, `desktop/`의 Rust 문자열과 프런트엔드 문자열·HTML
    텍스트 중 띄어쓴 영어 단어가 UI_WORDS개 이상인데 한글이 없는 것. 감독관이 읽는 알림·
    온보딩·대시보드 문구는 한국어다.

모드: 기본(훅 페이로드의 경로), `--all`(저장소의 모든 텍스트 파일, 발견하면 0이 아닌 값 — CI).
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from _hookio import has_flag, hook_root, rel_path, repo_files, run_all, run_post_hook
from _rustscan import comments, string_literals

PROSE_WORDS = 12
COMMENT_WORDS = 6
UI_WORDS = 4

LANG_CODES = frozenset({"en", "ko", "ja", "zh", "de", "fr", "es", "ru", "vi", "pt", "it"})
TRANSLATION_RE = re.compile(r"^(?P<base>.+)-(?P<code>[A-Za-z]{2})(?:-[A-Za-z]{2,4})?\.md$")

HASH_COMMENT_SUFFIXES = frozenset({".py", ".sh", ".toml", ".yml", ".yaml", ".rules"})
UI_ROOTS = ("crates/app/", "crates/dashboard/", "desktop/")
SLASH_COMMENT_SUFFIXES = frozenset({".ts", ".tsx", ".js", ".jsx", ".svelte", ".vue"})
MARKUP_SUFFIXES = frozenset({".html", ".svelte", ".vue"})
_JS_STRING_RE = re.compile(r"""(["'`])((?:\\.|(?!\1).)*)\1""")
_TAG_RE = re.compile(r"<[^>]*>|\{[^}]*\}")

_HANGUL_RE = re.compile(r"[가-힣ㄱ-ㆎ]")
_WORD_RE = re.compile(r"[A-Za-z]{2,}")
_URL_RE = re.compile(r"(?:https?://|git@)\S+")
_FENCE_RE = re.compile(r"^\s*(```|~~~)")
_INLINE_CODE_RE = re.compile(r"`[^`\n]*`")
_HTML_TAG_RE = re.compile(r"</?[A-Za-z][^>\n]*>")
# 줄 앞의 표식은 영어여도 된다(뒤에 오는 설명이 한국어다).
_MARKER_RE = re.compile(r"^(?:SAFETY|PURITY-OK|PASSIVE-OK|TODO\([^)]*\)|NOTE|SPDX-License-Identifier)\s*:?", re.IGNORECASE)

RULE = (
    "한국어 전용 (AGENTS.md §9): 문서·주석·독스트링·커밋·PR·감독관에게 보이는 문구는 한국어로 "
    "쓴다. 식별자·명령·경로·고유명사는 영어 그대로 둔다. 번역본(`-EN.md` 등)은 만들지 않는다."
)


def has_hangul(text: str) -> bool:
    return bool(_HANGUL_RE.search(text))


def english_words(text: str) -> int:
    text = _URL_RE.sub(" ", text)
    text = _INLINE_CODE_RE.sub(" ", text)
    text = _HTML_TAG_RE.sub(" ", text)
    return len(_WORD_RE.findall(text))


def is_translation(name: str) -> bool:
    match = TRANSLATION_RE.match(name)
    return bool(match) and match.group("code").lower() in LANG_CODES


def markdown_findings(text: str, label: str) -> list[str]:
    """한글 없이 영어로만 쓴 문단. 순수."""
    findings: list[str] = []
    in_fence = False
    para: list[tuple[int, str]] = []

    def flush() -> None:
        if not para:
            return
        joined = " ".join(t for _, t in para)
        if english_words(joined) >= PROSE_WORDS and not has_hangul(joined):
            findings.append(f"  {label}:{para[0][0]} — 영어로만 쓴 문단: {joined.strip()[:60]!r}")
        para.clear()

    for lineno, line in enumerate(text.splitlines(), start=1):
        if _FENCE_RE.match(line):
            flush()
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        if not line.strip():
            flush()
            continue
        para.append((lineno, line))
    flush()
    return findings


def _block_findings(lines: list[tuple[int, str]], label: str) -> list[str]:
    """연속된 주석 줄을 묶어 영어로만 쓴 묶음을 찾는다. 순수."""
    findings: list[str] = []
    block: list[tuple[int, str]] = []
    in_fence = False

    def flush() -> None:
        if not block:
            return
        joined = " ".join(t for _, t in block)
        if english_words(joined) >= COMMENT_WORDS and not has_hangul(joined):
            findings.append(f"  {label}:{block[0][0]} — 영어로만 쓴 주석: {joined.strip()[:60]!r}")
        block.clear()

    previous = -1
    for lineno, text in lines:
        if lineno != previous + 1:
            flush()
        previous = lineno
        if _FENCE_RE.match(text):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        text = _MARKER_RE.sub("", text.strip())
        if text:
            block.append((lineno, text))
    flush()
    return findings


def hash_comments(text: str, suffix: str) -> list[tuple[int, str]]:
    """`#` 주석 줄(셔뱅 제외)과, `.py`면 독스트링 줄. 순수."""
    found: list[tuple[int, str]] = []
    in_doc = False
    for lineno, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        if suffix == ".py" and ('"""' in stripped or "'''" in stripped):
            quotes = stripped.count('"""') + stripped.count("'''")
            body = stripped.replace('"""', " ").replace("'''", " ").strip()
            if in_doc or (quotes and stripped.startswith(('"""', "'''", 'r"""'))):
                if body:
                    found.append((lineno, body))
            if quotes % 2 == 1:
                in_doc = not in_doc
            continue
        if in_doc:
            found.append((lineno, stripped))
            continue
        if stripped.startswith("#!") and lineno == 1:
            continue
        if stripped.startswith("#"):
            found.append((lineno, stripped.lstrip("#").strip()))
        elif "  # " in line:
            found.append((lineno, line.split("  # ", 1)[1].strip()))
    return found


# 쿠키 속성, HTTP 요청줄, URL 같은 프로토콜 문자열은 감독관이 읽는 글이 아니다.
_PROTOCOL_MARKERS = ("=", ";", "\\r", "\\n", "://")


def is_test_file(label: str) -> bool:
    name = label.rsplit("/", 1)[-1]
    return name == "tests.rs" or "/tests/" in label


def looks_like_protocol(text: str) -> bool:
    return any(marker in text for marker in _PROTOCOL_MARKERS)


def ui_findings(source: str, label: str) -> list[str]:
    """감독관에게 보이는 Rust 문자열 중 영어로만 쓴 것. 테스트 파일과 프로토콜 문자열은 뺀다. 순수."""
    findings: list[str] = []
    if is_test_file(label):
        return findings
    for lineno, text in string_literals(source):
        if looks_like_protocol(text):
            continue
        if " " in text.strip() and english_words(text) >= UI_WORDS and not has_hangul(text):
            findings.append(f"  {label}:{lineno} — 영어로만 쓴 사용자 문구: {text[:60]!r}")
    return findings


_INTERPOLATION_RE = re.compile(r"\$\{[^}]*\}")
# 소문자·숫자·하이픈·밑줄뿐인 문자열은 CSS 클래스 목록이고, `=`가 있으면 설정·코드 줄이다 — 사람이 읽는 문구가 아니다.
_CLASS_LIST_RE = re.compile(r"[a-z0-9_\- .]*")


def looks_like_code(text: str) -> bool:
    stripped = _INTERPOLATION_RE.sub(" ", text).strip()
    return bool(_CLASS_LIST_RE.fullmatch(stripped)) or "=" in stripped


def frontend_ui_findings(text: str, label: str, suffix: str) -> list[str]:
    """프런트엔드의 문자열과 마크업 텍스트 중 영어로만 쓴 것. 순수.

    HTML 파일은 태그 사이 글만 본다(속성 값은 뷰포트·보안 정책 같은 설정이다). 문자열 안의 `${...}` 끼워 넣기는
    빼고 센다.
    """
    findings: list[str] = []
    for lineno, line in enumerate(text.splitlines(), start=1):
        candidates = [] if suffix == ".html" else [m.group(2) for m in _JS_STRING_RE.finditer(line)]
        candidates = [_INTERPOLATION_RE.sub(" ", c) for c in candidates if not looks_like_code(c)]
        if suffix in MARKUP_SUFFIXES and not line.lstrip().startswith(("<script", "<style", "import ", "//")):
            candidates.append(_TAG_RE.sub(" ", line))
        for text_part in candidates:
            if " " in text_part.strip() and english_words(text_part) >= UI_WORDS and not has_hangul(text_part):
                findings.append(f"  {label}:{lineno} — 영어로만 쓴 사용자 문구: {text_part.strip()[:60]!r}")
                break
    return findings


def check_text(text: str, rel: str) -> list[str]:
    """파일 하나(저장소 기준 경로 ``rel``)의 위반. 순수."""
    suffix = Path(rel).suffix
    if suffix in SLASH_COMMENT_SUFFIXES | {".html"}:
        found = _block_findings(comments(text), rel) if suffix in SLASH_COMMENT_SUFFIXES else []
        if rel.startswith(UI_ROOTS):
            found += frontend_ui_findings(text, rel, suffix)
        return found
    if suffix == ".md":
        return markdown_findings(text, rel)
    if suffix == ".rs":
        found = _block_findings(comments(text), rel)
        if rel.startswith(UI_ROOTS):
            found += ui_findings(text, rel)
        return found
    if suffix in HASH_COMMENT_SUFFIXES:
        return _block_findings(hash_comments(text, suffix), rel)
    return []


def check_file(path: Path) -> list[str]:
    root = hook_root()
    if not path.is_file():
        return []
    rel = rel_path(path, root)
    if rel is None:
        return []
    if is_translation(path.name):
        return [f"  {rel} — 번역본은 두지 않는다. 한국어 원본 하나만 유지한다."]
    if path.suffix not in HASH_COMMENT_SUFFIXES | SLASH_COMMENT_SUFFIXES | {".md", ".rs", ".html"}:
        return []
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return []
    return check_text(text, rel)


def main() -> None:
    if has_flag("--all"):
        root = hook_root()
        suffixes = (*HASH_COMMENT_SUFFIXES, *SLASH_COMMENT_SUFFIXES, ".md", ".rs", ".html")
        files = sorted({f for suffix in suffixes for f in repo_files(root, suffix)})
        run_all(check_file, files, "언어 검사", RULE)
    run_post_hook(check_file, "한국어 전용 규칙 위반", RULE)


if __name__ == "__main__":
    main()
