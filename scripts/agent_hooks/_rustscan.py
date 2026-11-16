"""훅들이 함께 쓰는 아주 작은 Rust 렉서.

원본 소스에 정규식을 바로 걸면 두 가지를 틀린다. 문자열 안의 `"std::fs"`나 주석 안의
`// if`가 코드로 잡히고, 문자열 안의 `}` 때문에 테스트 함수 본문이 일찍 끝난다. 그래서
주석과 리터럴을 지운 코드와, 테스트 함수마다의 본문을 따로 준다.

`strip`은 주석과 문자열/문자 리터럴을 **제자리에서** 공백으로 바꾼다. 줄바꿈은 모두 남고
나머지 글자는 공백이 되므로, 지운 텍스트의 오프셋과 줄 번호가 원본과 같다. 파서가 아니라
휴리스틱이며, 게이트 검사에 충분한 정도로만 맞고 애매하면 보고하는 쪽으로 기운다.
Python 3.9 호환, 표준 라이브러리만 쓴다.
"""

from __future__ import annotations

import re
from typing import NamedTuple

_RAW_RE = re.compile(r'(?:b|c)?r(#*)"')
_IDENT_CHAR_RE = re.compile(r"\w")

# 이 저장소가 인정하는 테스트 속성. `proptest!` 본문은 평범한 `#[test]`를 단다.
_TEST_ATTR_RE = re.compile(r"#\[\s*(?:(?:tokio|async_std)::test|test|rstest|test_case)\b[^\]]*\]")
_FN_RE = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)")
_ATTR_RE = re.compile(r"#\[[^\]]*\]")
_FN_PREFIX_RE = re.compile(r"^[\s\w()]*$")


def _blank(text: str) -> str:
    return "".join("\n" if ch == "\n" else " " for ch in text)


def _ident_before(src: str, i: int) -> bool:
    return i > 0 and bool(_IDENT_CHAR_RE.match(src[i - 1]))


def _char_literal_end(src: str, i: int) -> int:
    """``i``에서 시작하는 문자 리터럴('x', '\\n', '\\u{..}')의 끝 오프셋. 수명 표기면 0."""
    n = len(src)
    if i + 1 >= n:
        return 0
    if src[i + 1] == "\\":
        if src[i + 2 : i + 3] == "'":  # '\''
            return i + 4
        end = src.find("'", i + 2)
        return 0 if end == -1 else end + 1
    if i + 2 < n and src[i + 2] == "'":
        return i + 3
    return 0  # 수명('a)이나 루프 레이블('outer:)


def strip(src: str) -> str:
    """주석과 문자열/문자 리터럴을 공백으로 지운 ``src``. 오프셋은 그대로 유지한다."""
    out: list[str] = []
    i, n = 0, len(src)
    while i < n:
        ch = src[i]
        nxt = src[i + 1] if i + 1 < n else ""
        if ch == "/" and nxt == "/":
            end = src.find("\n", i)
            end = n if end == -1 else end
            out.append(_blank(src[i:end]))
            i = end
            continue
        if ch == "/" and nxt == "*":
            depth, j = 1, i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif src.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            out.append(_blank(src[i:j]))
            i = j
            continue
        raw = _RAW_RE.match(src, i)
        if raw and not _ident_before(src, i):
            close = src.find('"' + raw.group(1), raw.end())
            j = n if close == -1 else close + 1 + len(raw.group(1))
            out.append(_blank(src[i:j]))
            i = j
            continue
        if ch == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            j = min(j + 1, n)
            out.append(_blank(src[i:j]))
            i = j
            continue
        if ch == "'":
            end = _char_literal_end(src, i)
            if end:
                out.append(_blank(src[i:end]))
                i = end
                continue
        out.append(ch)
        i += 1
    return "".join(out)


def string_literals(src: str) -> list[tuple[int, str]]:
    """문자열 리터럴마다 (줄 번호, 내용). 주석 안의 따옴표는 무시한다."""
    found: list[tuple[int, str]] = []
    i, n = 0, len(src)
    while i < n:
        ch = src[i]
        nxt = src[i + 1] if i + 1 < n else ""
        if ch == "/" and nxt == "/":
            end = src.find("\n", i)
            i = n if end == -1 else end
            continue
        if ch == "/" and nxt == "*":
            close = src.find("*/", i + 2)
            i = n if close == -1 else close + 2
            continue
        raw = _RAW_RE.match(src, i)
        if raw and not _ident_before(src, i):
            close = src.find('"' + raw.group(1), raw.end())
            j = n if close == -1 else close
            found.append((line_of(src, i), src[raw.end() : j]))
            i = j + 1 + len(raw.group(1))
            continue
        if ch == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            found.append((line_of(src, i), src[i + 1 : j]))
            i = j + 1
            continue
        if ch == "'":
            end = _char_literal_end(src, i)
            if end:
                i = end
                continue
        i += 1
    return found


def comments(src: str) -> list[tuple[int, str]]:
    """주석 줄마다 (줄 번호, `//`·`///`·`/*`를 뗀 내용). 문자열 안의 `//`는 무시한다."""
    found: list[tuple[int, str]] = []
    i, n = 0, len(src)
    while i < n:
        ch = src[i]
        if src.startswith("//", i):
            end = src.find("\n", i)
            end = n if end == -1 else end
            found.append((line_of(src, i), src[i:end].lstrip("/!").strip()))
            i = end
            continue
        if src.startswith("/*", i):
            close = src.find("*/", i + 2)
            close = n if close == -1 else close
            first = line_of(src, i)
            for k, text in enumerate(src[i + 2 : close].split("\n")):
                found.append((first + k, text.strip().lstrip("*!").strip()))
            i = close + 2
            continue
        raw = _RAW_RE.match(src, i)
        if raw and not _ident_before(src, i):
            close = src.find('"' + raw.group(1), raw.end())
            i = n if close == -1 else close + 1 + len(raw.group(1))
            continue
        if ch == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            i = j + 1
            continue
        if ch == "'":
            end = _char_literal_end(src, i)
            if end:
                i = end
                continue
        i += 1
    return found


def line_of(text: str, offset: int) -> int:
    """``offset``의 1부터 세는 줄 번호."""
    return text.count("\n", 0, offset) + 1


def _matching_brace(text: str, open_at: int) -> int:
    depth = 0
    for j in range(open_at, len(text)):
        if text[j] == "{":
            depth += 1
        elif text[j] == "}":
            depth -= 1
            if depth == 0:
                return j
    return -1


class TestFn(NamedTuple):
    name: str
    line: int
    attrs: str  # fn 위의 속성 블록(지운 텍스트)
    body: str  # 바깥 중괄호 사이 본문(지운 텍스트)
    body_line: int  # 여는 중괄호의 줄


def test_functions(stripped: str) -> list[TestFn]:
    """이미 지운 소스 안의 `#[test]` 류 함수 전부."""
    found: list[TestFn] = []
    seen: set[int] = set()
    for attr in _TEST_ATTR_RE.finditer(stripped):
        fn = _FN_RE.search(stripped, attr.end())
        if fn is None or fn.start() in seen:
            continue
        # 테스트 속성과 `fn` 사이에는 다른 속성과 fn 한정자만 올 수 있다. `{`, `;`, 매크로
        # 메타변수 `fn $name` 같은 것이 끼어 있으면 이 속성은 찾은 fn의 것이 아니다.
        between = _ATTR_RE.sub(" ", stripped[attr.end() : fn.start()])
        if not _FN_PREFIX_RE.match(between):
            continue
        open_at = stripped.find("{", fn.end())
        close_at = _matching_brace(stripped, open_at) if open_at != -1 else -1
        if close_at == -1:
            continue
        seen.add(fn.start())
        block_start = max(stripped.rfind(c, 0, attr.start()) for c in ";{}") + 1
        found.append(
            TestFn(
                name=fn.group(1),
                line=line_of(stripped, fn.start()),
                attrs=stripped[block_start : fn.start()],
                body=stripped[open_at + 1 : close_at],
                body_line=line_of(stripped, open_at),
            )
        )
    return found
