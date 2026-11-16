#!/usr/bin/env python3
"""SessionStart 훅: 현재 단계와 절대 규칙을 에이전트 앞에 놓는다.

단계마다 완료 기준이 있고 다음 단계의 기능을 미리 만들지 않으므로(AGENTS.md §6), 첫
편집 전에 현재 단계를 알아야 한다. 현재 단계는 docs/STATUS.md의 `▶` 줄 하나다
(scripts/check_agent_config.py는 정확히 하나가 아니면 실패한다).

사용법:
    session_context.py --tool claude|codex
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from _hookio import emit_context, hook_root, parse_args

MARKER = "▶"


def current_phase(status_text: str) -> str | None:
    lines = [line.strip() for line in status_text.splitlines() if line.strip().startswith(MARKER)]
    if len(lines) != 1:
        return None
    return lines[0].lstrip(MARKER).strip()


def build_context(root: Path) -> str:
    try:
        status = (root / "docs" / "STATUS.md").read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        status = ""
    phase = current_phase(status) or "알 수 없음 — docs/STATUS.md에 `▶` 줄이 정확히 하나 있어야 한다"
    return "\n".join(
        (
            "## 세션 컨텍스트 (Tacet — 시험장 무선 기기 탐지기)",
            "",
            f"**현재 단계:** {phase}",
            "이 단계 안에서만 작업한다. 이후 단계의 기능은 스텁 + `TODO(phaseN)`으로 남긴다 (AGENTS.md §6).",
            "",
            "**절대 금지:** 전파 송신·재밍·주입·기기 연결 (전파법) / 패킷 내용·"
            "셀룰러 식별자 디코딩 (통신비밀보호법) / 실제 캡처·MAC·SSID를 저장소에 넣거나 대화로 읽기, "
            "관측 데이터를 내부망 밖으로 보내기 (개인정보) / 알림에서 부정행위를 단정하는 문구 / "
            "고정 RSSI 임계치 / 라이선스·출처 없는 제3자 코드 / "
            "core에 IO·시계·async·난수 / 테스트 없는 core 변경 (AGENTS.md §2, §3).",
            "",
            "**언어:** 문서·주석·커밋·PR·사용자 문구는 한국어. 식별자만 영어 (AGENTS.md §9).",
            "",
            "**게이트:** `./scripts/verify.sh` — 도구 훅은 빠른 피드백일 뿐이다.",
        )
    )


def main() -> None:
    emit_context(parse_args(), build_context(hook_root()))


if __name__ == "__main__":
    main()
