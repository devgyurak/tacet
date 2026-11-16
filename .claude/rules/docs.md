---
paths:
  - "docs/**"
  - "README.md"
---

# 문서 요약

정본은 AGENTS.md §3.3, §3.6, §9다.

- 한국어로 쓴다. 번역본(`-EN.md` 등)은 만들지 않는다.
- 규칙 노트는 `docs/rules/README.md`, 플랫폼 노트는 `docs/platform/README.md`, 실측 기록은
  `docs/field/README.md` 템플릿을 따른다.
- 실측 기록에는 요약 수치와 기기 라벨만 남긴다. 실제 MAC·SSID·기기 이름·장소를 특정할 정보는 쓰지 않는다.
- 근거에는 출처를 적고, 확인하지 못한 것은 "미검증"으로 남긴다.
- `docs/DESIGN.md`를 바꾸는 것은 설계 변경이다. 되돌리기 비싼 변경은 `decision` 이슈로 먼저 정한다.
- 문서만 바꾸는 커밋은 `DOCS:`.
