#!/usr/bin/env python3
"""방금 편집한 Rust 파일에 `rustfmt`를 적용하는 편집 후 훅.

저장소는 늘 `cargo fmt`가 깨끗한 상태이므로, 편집한 파일을 포맷하면 실제로 바뀐 곳에만
diff가 남는다. rustfmt는 파일의 상위 디렉터리에서 `rustfmt.toml`을 찾고, `--edition 2024`는
작업 공간 에디션과 맞춘다. rustfmt가 없으면(또는 Cargo 작업 공간이 아직 없으면) 조용히 아무
일도 하지 않는다 — 진짜 게이트는 ./scripts/verify.sh의 `cargo fmt --all --check`다.

사용법:
    format_rust.py --tool claude|codex
"""

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from _hookio import emit_allow, emit_note, extract_paths, hook_root, parse_args, read_payload, rel_path

EDITION = "2024"


def _eligible(raw: str, root: Path) -> str | None:
    """포맷 대상이면 저장소 기준 상대 경로, 아니면 None."""
    path = Path(raw).expanduser()
    if path.suffix != ".rs" or not path.is_file():
        return None
    return rel_path(path, root)


def main() -> None:
    tool = parse_args()
    payload = read_payload()
    root = hook_root()

    targets = [rel for rel in (_eligible(p, root) for p in extract_paths(payload)) if rel]
    rustfmt = shutil.which("rustfmt")
    if not targets or rustfmt is None:
        emit_allow(tool)

    try:
        result = subprocess.run(
            [str(rustfmt), "--edition", EDITION, *targets],
            cwd=root,
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        emit_allow(tool)
        return

    names = ", ".join(Path(t).name for t in targets)
    if result.returncode != 0:
        # 구문 오류: verify 게이트까지 기다리지 않고 지금 알린다. 편집 중이므로 막지는 않는다.
        emit_note(tool, f"rustfmt가 {names}을(를) 포맷하지 못했다:\n{result.stderr.strip()[:2000]}")
    emit_note(tool, f"rustfmt 적용: {names}")


if __name__ == "__main__":
    main()
