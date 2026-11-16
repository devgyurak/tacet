# 에이전트 설정 지도

이 저장소는 코딩 에이전트 두 개 — **Claude Code**와 **Codex** — 를 지원한다. **정본은 루트 `AGENTS.md`
하나다.** 두 도구 모두 각자의 진입점에서 여기로 모인다. 규칙을 바꾸려면 `AGENTS.md`를 고친다 — 아래
파일들은 도구별 배선이다.

| 도구 | 읽는 것 | 비고 |
| --- | --- | --- |
| **Claude Code** | `CLAUDE.md` → `@AGENTS.md` + `.claude/rules/` + `.claude/skills/` + `.claude/settings.json` | 서브에이전트는 `.claude/agents/` |
| **Codex** | `AGENTS.md` + `.agents/skills/`<sup>※</sup> + `.codex/` | 서브에이전트 `.codex/agents/*.toml`, 훅 `.codex/hooks.json`<sup>※</sup>, 명령 정책 `.codex/rules/`<sup>※</sup> |

<sup>※</sup> **미검증.** Codex의 `SKILL.md` 탐색, 훅, 프로젝트 `.codex/rules/`는 Codex 문서를 따랐지만 이
저장소에서 동작하는 것을 아직 관찰하지 못했다. 정본이 두 도구 모두 읽는 `AGENTS.md`에 있으므로, 도구가
스킬을 열거나 훅을 돌리지 못해도 규칙은 에이전트에 닿고, `./scripts/verify.sh`와 CI가 어쨌든 선을 지킨다.

스킬은 도구마다 **복제하지 않는다.** 복제하는 것은 서브에이전트뿐인데, `.codex/agents/`가 다른 형식(TOML)을
쓰기 때문이다.

## 파일 배치

```
AGENTS.md                       ← 정본 (Codex가 직접 읽는다)
CLAUDE.md                       ← @AGENTS.md를 가져오고 Claude 전용 내용을 더한다
docs/DESIGN.md                  ← 설계 문서, "설계 §N"으로 인용
docs/STATUS.md                  ← 현재 단계(▶ 한 줄) + 결정됨 + 열린 결정
docs/rules/README.md            ← 탐지 규칙 노트 템플릿
docs/platform/README.md         ← 수집기·플랫폼 노트 템플릿
docs/field/README.md            ← 실측·교실 보정 기록 템플릿 (요약만)

.claude/
├── agents/                     ← Claude Code 서브에이전트
│   ├── architecture-guard.md
│   ├── compliance-auditor.md
│   ├── detection-reviewer.md
│   └── test-author.md
├── rules/                      ← 경로별 요약 (paths 프런트매터, 정본 절을 가리킴)
│   ├── core.md                 ← crates/core/**
│   ├── collectors.md           ← crates/collector-*/**
│   ├── net.md                  ← crates/net/**
│   ├── app-ui.md               ← crates/app/**, crates/dashboard/**
│   ├── devtools.md             ← crates/devtools/**
│   ├── tests.md                ← tests/**, crates/**/tests*
│   └── docs.md                 ← docs/**, README.md
├── settings.json               ← Claude 훅 + 권한
└── skills/                     ← 스킬 원본 (두 도구가 공유, SKILL.md 표준)
    ├── verify/SKILL.md
    ├── add-rule/SKILL.md
    ├── add-collector/SKILL.md
    ├── triage-detection/SKILL.md
    ├── field-test/SKILL.md
    └── commit/SKILL.md

.agents/
├── README.md                   ← 이 파일
└── skills → ../.claude/skills  ← 심볼릭 링크 (Codex가 .agents/skills를 읽는다)

.codex/
├── config.toml                 ← project_doc_max_bytes = 64KiB (한국어 정본)
├── hooks.json
├── rules/default.rules         ← 명령 정책 (prompt / forbidden)
└── agents/                     ← 같은 서브에이전트, Codex 형식
    ├── architecture_guard.toml
    ├── compliance_auditor.toml
    ├── detection_reviewer.toml
    └── test_author.toml

scripts/
├── verify.sh                   ← 유일한 게이트 (로컬과 CI)
├── check_agent_config.py       ← 무결성 검사 (CI 게이트)
└── agent_hooks/                ← 훅 본문 (Claude · Codex 공유)
    ├── _hookio.py              ← 도구별 stdin/stdout 형식
    ├── _rustscan.py            ← 작은 Rust 렉서 (주석·리터럴, 테스트 fn 본문)
    ├── session_context.py
    ├── guard_passive.py
    ├── format_rust.py
    ├── check_architecture.py
    ├── check_passive.py
    ├── check_tests.py
    ├── check_language.py
    └── selftest.py
```

## 서브에이전트

네 개 모두 `.claude/agents/`와 `.codex/agents/`에 있다. 본문과 description은 같고 형식만 다르다.

- **architecture-guard** (읽기 전용) — 크레이트 그래프, core 순수성, 판정의 자리(core만), 결정성·중앙값, 고정 임계치,
  macOS UUID 상관, 조용히 꺼지는 수집기, 외부 입력·센서 보고서 패닉, `unsafe`, 단계 범위. `crates/`나 `Cargo.toml`을 바꾼 뒤.
- **compliance-auditor** (읽기 전용) — 송신·HackRF 송신(전파법), 내용·식별자 디코딩(통신비밀보호법), 실제
  식별자·캡처·녹화·내부망 밖 전송·보존(개인정보), 단정적 알림 문구, 의료기기·보청기 화이트리스트, 범위 확장,
  의존성 라이선스, 서명 키. 수집기 · net · dashboard · app · devtools · 테스트 데이터 · `docs/field/` · README ·
  의존성 · 바이너리 파일을 바꾼 뒤 **필수**.
- **detection-reviewer** (읽기 전용) — 위협 T1–T7 커버리지와 신호 간 보완, 오탐 요인, 교실 재배정, 보정·점수·신뢰도,
  기준선과 첫 관측 지연, 노트·코드·테스트 일치. 탐지 로직이나 `docs/rules/`를 바꾼 뒤.
- **test-author** (쓰기 가능) — 규칙대로 테스트를 쓴다: GWT 이름, 제어 흐름 없음, 하네스 함수, 위협·오탐·다교실 합성
  시나리오, 결정성, 합성 식별자. 구현 전(실패하는 테스트) 또는 커버리지가 모자랄 때.

## 스킬

- **verify** — `./scripts/verify.sh`를 돌리고 실패를 진단한다
- **add-rule** — 규칙·위치 판정·보정 방식: 단계 확인 → 위협 매핑 → `docs/rules/` 노트 → 실패하는 시나리오 → core 구현 → 오탐 검토 → verify → `decision` 이슈
- **add-collector** — 단계 확인 → 플랫폼 노트(송신 여부·라이선스 먼저) → 순수 변환 + 합성 테스트 → 얇은 OS·USB 어댑터 → 상태 보고 → 실측 → verify
- **triage-detection** — 익명화한 재현 → 점수·위치 분해로 첫 갈림점 → 노트 대조 → 실패 테스트 → 수정
- **field-test** — 교실 보정·신뢰도·기기 특성을 사람이 자기 기기로, 송신 없이 실측하고 요약만 `docs/field/`에 기록
- **commit** — 커밋·PR·브랜치 규칙과 접두사 경계

## 규칙 파일

`.claude/rules/`의 `core.md`, `collectors.md`, `net.md`, `app-ui.md`, `devtools.md`, `tests.md`, `docs.md`는 Claude Code가 해당 경로의
파일을 다룰 때 불러오는 **요약**이다. 새 규칙을 만들지 않고 AGENTS.md의 절을 가리킨다. Codex는 같은 내용을
정본 `AGENTS.md`에서 얻으므로 따로 복제하지 않는다.

## 훅

본문은 `scripts/agent_hooks/`에 한 번만 있다. 각 도구의 훅 설정이 같은 스크립트를 `--tool` 플래그와 함께
부르므로 동작은 한곳에서 바뀐다.

| 훅 | 언제 | 하는 일 |
| --- | --- | --- |
| `session_context.py` | SessionStart | 현재 단계와 절대 규칙을 알린다 |
| `guard_passive.py` | Pre | 송신 도구, 디코더 프로젝트, 캡처·Kismet DB·관측 녹화 읽기·커밋, 문서용 범위 밖 MAC 쓰기를 막는다. `--scan-tracked`는 `git ls-files`를 검사(서명 키 포함) |
| `format_rust.py` | Post | 편집한 `*.rs`에 `rustfmt --edition 2024`. rustfmt가 없으면 아무 일도 안 함 |
| `check_architecture.py` | Post | core 순수성, core·net·dashboard의 OS 분기, `SAFETY:` 없는 `unsafe`, 크레이트 그래프, 패키지 이름. `--all`은 저장소 전체 |
| `check_passive.py` | Post | 송신·연결·HackRF 송신·디코딩·텔레메트리 코드, 문서용 범위 밖 MAC, 단정적 알림 문구(Rust와 프런트엔드). `--all`은 저장소 전체 |
| `check_tests.py` | Post | GWT 이름, 제어 흐름 없음, 단언 있음, 테스트 위치. `--all`은 저장소 전체 |
| `check_language.py` | Post | 영어로만 쓴 문서·주석·사용자 문구(Rust와 프런트엔드), 번역본 파일. `--all`은 저장소 전체 |

강제는 **최선 노력**이다: 연결된 도구로 편집한 파일에만 걸린다. `Bash`/`sed`로 바꾼 파일이나 훅이 로드되지
않은 하네스는 건너뛴다 — 그래서 중요한 규칙은 모두 `./scripts/verify.sh`와 CI에서도 돈다(AGENTS.md §7).
수동성 가드는 **닫힌 쪽으로** 실패한다(오류가 나면 거부).

## 유지보수

- **규칙을 바꾸려면 `AGENTS.md`를 고친다.** 두 도구가 따라온다.
- **서브에이전트는 두 사본 모두 고친다**(`.claude/` · `.codex/`). Codex의 `description`과
  `developer_instructions`는 Claude의 description과 본문과 글자 그대로 같아야 한다.
  `scripts/check_agent_config.py`가 확인한다.
- **훅이 강제하는 목록은 AGENTS.md에 한 번 적는다** — core 금지 크레이트(§2), 송신 도구(§3.1), 디코더 프로젝트
  (§3.2), 캡처 확장자(§3.3). 훅의 집합과 AGENTS.md 줄을 함께 바꾼다. 다르면 설정 검사가 실패한다.
- **권한 정책은 두 곳에 같게 둔다** — `.claude/settings.json`의 ask/deny와 `.codex/rules/default.rules`의
  prompt/forbidden. 송신 도구는 두 곳 모두에서 거부한다. 설정 검사가 확인한다.
- **스킬은 `.claude/skills/`에서만 고친다** — Codex는 `.agents/skills` 링크로 같은 파일을 본다. 심볼릭 링크를
  따라가지 못하는 체크아웃에서는 스킬 탐색이 비지만 `AGENTS.md`는 그대로 작동한다.
- **규칙 대 스킬**: 늘 참인 규칙(크레이트 그래프, 수동 감지 불변식, 고정 임계치 금지)은 `AGENTS.md`에, 특정 작업에만 필요한
  절차(규칙 추가, 실측, 커밋)는 스킬에 둔다. `AGENTS.md`는 모든 세션에 로드되고 스킬은 필요할 때 로드된다.
- **에이전트 설정을 고친 뒤에는** `python3 scripts/check_agent_config.py`를 돌린다. CI
  (`.github/workflows/agents.yml`)가 같은 검사와 훅 selftest를 돌린다.
