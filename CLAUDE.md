# CLAUDE.md

@AGENTS.md

위 파일이 이 저장소의 정본이다 — Windows·macOS 데스크톱 앱의 크레이트 그래프와 core 순수성, 실행 모드(단독·센서·
서버·본부 화면 보기, 감독관 화면은 단독 모드·Electron 화면), 수동 감지·법·개인정보 불변식(송신 없음 · 디코딩 없음 · 실제 식별자 없음 · 내부망 밖 전송 없음 ·
"확인 필요" 문구), 고정 RSSI 임계치 금지와 교실 보정, 의존성 라이선스 규칙, Rust 관례, 테스트 규칙
(GWT 이름, 제어 흐름 없음, 결정성, 합성 데이터, 85% 커버리지 하한), 한국어 전용 규칙. 아래는 Claude Code에서만 필요한 내용이다.

## 서브에이전트

- `architecture-guard` — 크레이트 그래프 위반(core ← 수집기·net·dashboard, app·devtools는 끝), core의 IO·시계·async·
  난수·OS 분기, 결정성을 깨는 컬렉션·동점·중앙값, 고정 RSSI 임계치, 판정하는 센서·수집기·UI, macOS UUID의 잘못된 상관,
  조용히 꺼지는 수집기, FFI 밖의 `unsafe`, 현재 단계를 넘는 작업을 잡는다. `crates/`나 `Cargo.toml`을 건드린 뒤 부른다.
- `compliance-auditor` — 송신·HackRF 송신(전파법), 패킷 내용·식별자 디코딩(통신비밀보호법), 실제
  식별자·캡처·녹화·내부망 밖 전송·보존 정책(개인정보), 단정적 알림 문구, 의료기기·보청기 화이트리스트, 범위 확장,
  의존성 라이선스, 서명 키를 본다. 수집기, `crates/net/`, `crates/dashboard/`, `crates/app/`,
  `crates/devtools/`, 테스트 데이터, `docs/field/`, README를 건드렸거나 의존성·바이너리 파일을 더했으면 **반드시** 부른다.
- `detection-reviewer` — 규칙·가중치·교실 보정·기준선·기기 상관·다교실 위치 판정 변경이 위협 모델(T1–T7) 커버리지,
  오탐 요인, 교실 배정에 주는 영향을 검토한다. `crates/core/`의 탐지 로직이나 `docs/rules/`를 바꾼 뒤 부른다.
- `test-author` — 규칙대로 테스트를 쓴다(GWT 이름, 제어 흐름 없음, 하네스 함수, 위협·오탐·다교실 합성 시나리오,
  결정성, 합성 식별자). 구현 전에 실패하는 테스트를 놓거나 커버리지가 모자랄 때 부른다.

리뷰 서브에이전트(`architecture-guard`, `compliance-auditor`, `detection-reviewer`)는 **한 메시지에서 병렬로** 부르고,
코드를 쓴 세션에서 돌리지 않는다 — 새 리뷰어는 작성자의 맹점을 공유하지 않는다.

## 스킬

- `verify` — 전체 게이트(`./scripts/verify.sh`)를 돌리고 실패마다 원인을 진단한다.
- `add-rule` — 탐지 규칙·위치 판정·보정 방식을 추가·변경한다. **0단계는 단계 확인, 1단계는 `docs/rules/` 노트** —
  테스트와 코드보다 먼저.
- `add-collector` — 수집기나 Windows·macOS 지원, 권한 온보딩, 드라이버 처리를 추가한다. 플랫폼 노트에 **송신 여부와
  라이선스**를 먼저 적는다.
- `triage-detection` — 오탐·미탐·위치 오판을 조사한다: 익명화한 재현 → 점수·위치 분해 → 규칙 노트 → 실패 테스트 → 수정.
- `field-test` — 교실 보정, 신뢰도 기준, 안경·폰 특성, 플랫폼 동작 실측을 계획하고 요약을 기록한다.
  송신 없이, 원본 없이.
- `commit` — 커밋 메시지·PR 제목·브랜치 이름 규칙(`ADD:`/`FIX:`/`REF:`/`DEL:`/`DOCS:`/`UPT:`)과 PR 본문 규칙. 커밋 전에 읽는다.

## 규칙 파일

`.claude/rules/`의 파일은 경로별로 떠오르는 **요약**이다(`paths` 프런트매터). 정본은 AGENTS.md이고, 규칙 파일은 해당 절을
가리킬 뿐 새 규칙을 만들지 않는다. 규칙을 바꾸려면 AGENTS.md를 고친다.

## 훅

`.claude/settings.json`이 훅 일곱 개를 연결한다. 본문은 `scripts/agent_hooks/`에 있고 Codex와 공유한다.

| 이벤트 | 동작 |
| --- | --- |
| `SessionStart` | 현재 단계(`docs/STATUS.md`)와 절대 규칙을 알린다. |
| `PreToolUse` | `guard_passive.py` — 송신 도구, 디코더 프로젝트, 실제 캡처·Kismet DB·관측 녹화 읽기·커밋, 문서용 범위 밖 MAC 쓰기를 막는다. |
| `PostToolUse` (Write/Edit) | `format_rust.py` — 편집한 `*.rs`에 `rustfmt`. |
| `PostToolUse` (Write/Edit) | `check_architecture.py` — core 순수성, core·net·dashboard의 OS 분기, 크레이트 그래프, 이름, `SAFETY:` 없는 `unsafe`. |
| `PostToolUse` (Write/Edit) | `check_passive.py` — 송신·연결·HackRF 송신·디코딩·텔레메트리 코드, 실제 MAC, 단정적 알림 문구(Rust와 프런트엔드). |
| `PostToolUse` (Write/Edit) | `check_tests.py` — GWT 아닌 이름, 테스트 본문의 제어 흐름, 단언 없는 테스트. |
| `PostToolUse` (Write/Edit) | `check_language.py` — 영어로만 쓴 문서·주석·사용자 문구(Rust와 프런트엔드), 번역본 파일. |

훅이 `block`이나 `deny`를 돌려주면 알려준 문제를 고치고 다시 한다 — 우회하지 않는다. 훅은 최선 노력이라(도구 편집에만
걸린다) `./scripts/verify.sh`와 CI가 진짜 게이트다. 수동성 가드는 판단하지 못하면 닫힌 쪽으로 실패한다.

## 도구 사용

- 작업이 끝났다고 말하기 전에 `./scripts/verify.sh`를 돌린다.
- 넓은 탐색은 Explore 서브에이전트에 맡기고 결론만 받는다.
- 크레이트 그래프, 점수·위치 판정 모델, 기본 가중치·신뢰도 기준, 센서 프로토콜, 보존 정책, 라이선스 같은 설계 결정
  전에는 plan 모드에 들어가고, 결정은 `decision` 라벨 GitHub 이슈로 남긴다(AGENTS.md §6).
- 커밋은 사용자가 요청할 때만 한다.
