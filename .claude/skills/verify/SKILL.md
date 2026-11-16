---
name: verify
description: 전체 품질 게이트(./scripts/verify.sh)를 순서대로 돌리고 실패를 진단한다. 작업을 끝내기 전, 커밋 전, 또는 사용자가 검증·게이트·테스트 실행·통과 확인을 요청할 때 쓴다. 에이전트 설정 검사, 훅 selftest, 수동성 스캔, 아키텍처, 수동 감지 코드 검사, 테스트 규칙, 언어, fmt, clippy, cargo-deny, 테스트와 커버리지를 다루고 실패마다 흔한 원인을 알려준다.
---

# 게이트 돌리기

**작업이 끝나려면 `./scripts/verify.sh`가 통과해야 한다.** 모든 단계를 순서대로 돌리고 첫 실패에서
멈춘다 — 앞 단계를 고치면 뒤 단계 결과가 바뀌므로, 고친 뒤 처음부터 다시 돌린다.

```bash
./scripts/verify.sh
```

돌리는 것, 순서대로:

| # | 단계 | 명령 |
|---|---|---|
| 1 | 에이전트 설정 | `python3 scripts/check_agent_config.py` |
| 2 | 훅 selftest | `python3 scripts/agent_hooks/selftest.py` |
| 3 | 수동성 스캔 | `python3 scripts/agent_hooks/guard_passive.py --scan-tracked` |
| 4 | 아키텍처 | `python3 scripts/agent_hooks/check_architecture.py --all` |
| 5 | 수동 감지 코드 | `python3 scripts/agent_hooks/check_passive.py --all` |
| 6 | 테스트 규칙 | `python3 scripts/agent_hooks/check_tests.py --all` |
| 7 | 언어 | `python3 scripts/agent_hooks/check_language.py --all` |
| 8 | 툴체인 고정 | `rust-toolchain.toml`의 `channel = "1.NN.0"` |
| 9 | 포맷 | `cargo fmt --all -- --check` |
| 10 | 린트 | `cargo clippy --workspace --all-targets --locked -- -D warnings` (기본 빌드와 `--all-features`: sdr) |
| 11 | 의존성 라이선스 | `cargo deny --locked check licenses bans sources` |
| 12 | 테스트 + 커버리지 ≥ 85% | `cargo llvm-cov nextest --workspace --locked --fail-under-lines 85` |
| 13 | 문서 테스트 | `cargo test --workspace --locked --doc` |

Phase 0에서 `Cargo.toml`이 생기기 전까지 8–13단계는 `SKIPPED`를 출력한다. 생긴 뒤에는 `cargo-deny`나
`cargo-llvm-cov`, `--all-features`에 필요한 시스템 라이브러리(SoapySDR·libhackrf)나 화면 시험용 Node.js(22 이상)가 없으면 설치 방법과 함께 실패한다. 보고할
때 그렇게 말한다 — Rust 게이트는 통과한 것이 아니라 돌지 않은 것이다.

## 실패 진단

### 1. 에이전트 설정
- *본문이 Claude 본문과 다르다* — 서브에이전트를 한쪽만 고쳤다. `.claude/agents/<이름>.md`를 고치고
  `.codex/agents/<이름_밑줄>.toml`의 `description`과 `developer_instructions`를 글자 그대로 맞춘다.
- *목록이 훅과 다르다* — AGENTS.md의 core 금지 크레이트(§2)·송신 도구(§3.1)·디코더 프로젝트(§3.2)·
  캡처 확장자(§3.3) 줄이 훅의 집합과 어긋났다. 둘을 함께 바꾼다.
- *.codex/rules에 없다* — `.claude/settings.json`의 ask/deny를 바꿨으면 `.codex/rules/default.rules`에
  같은 접두사로 `prompt`/`forbidden` 규칙을 더한다.
- *나오지 않는다* — 새 스킬·서브에이전트·규칙 파일을 `CLAUDE.md`와 `.agents/README.md`에 적는다.

### 2. 훅 selftest
훅의 동작이 바뀌었다. 실패한 사례 이름이 기대한 결정을 말한다. 사례가 아니라 훅을 고친다 — 사례가
틀렸다면 왜 틀렸는지 말한다.

### 3. 수동성 스캔
캡처·Kismet DB·관측 녹화, 서명 인증서·키, 디코더 이름의 디렉터리가 추적되고 있다. 한 번이라도 푸시했다면 **새 커밋에서 지우는
것으로 끝나지 않는다**: 기록에 남는다. 멈추고 사람에게 알린다.

### 4. 아키텍처
- *core에 std IO / 시계 / 출력 / async* — IO와 비동기는 수집기·net·app으로 옮기고 값을 넘긴다. 시각은
  `Timestamp`로 받는다.
- *해시 컨테이너* — `BTreeMap`, 작은 id로 인덱싱하는 배열, 정렬된 `Vec`.
- *OS 분기* — core·net·dashboard에서는 안 된다. 수집기의 `sys/` 모듈이나 app으로 옮기고 변환 로직은 순수 함수로 뺀다.
- *core에만 의존한다 / 앱·devtools에 의존한다* — 크레이트 그래프(AGENTS.md §2)를 다시 본다. 공용 타입(`Observation`,
  `SensorReport`, `Localization`)이면 core에 둔다.
- *SAFETY 없는 unsafe* — 건전성 논증을 쓴다. FFI가 아니면 `unsafe`를 없앤다.
- `// PURITY-OK: <이유>`는 리뷰어가 받아들일 이유를 위한 것이지 편의를 위한 것이 아니다.

### 5. 수동 감지 코드
- *송신·연결·HackRF 송신* — 그 코드를 지운다. 수신 경로인데 이름이 걸렸다면 `// PASSIVE-OK: <이유>`를 달고
  `compliance-auditor`에게 확인받는다.
- *셀룰러 식별자·데이터 프레임* — 필드를 버린다. 대역 에너지는 `uplink`처럼 이름 짓는다.
- *원격 수집·텔레메트리* — 지운다. 관측 데이터는 시험장 내부망 밖으로 나가지 않는다.
- *문서용 범위 밖의 MAC* — `00:00:5E:00:53:xx` / `02:00:5E:00:53:xx`로 바꾼다. 실측 주소였다면 이미
  푸시했는지 확인하고 사람에게 알린다.
- *단정 문구* — "확인 필요"로 다시 쓴다. 앱·대시보드 프런트엔드 파일도 본다.

### 6. 테스트 규칙
- *GWT 형식이 아니다* — 동작을 따라 이름을 바꾼다. `then_` 뒤는 관찰할 수 있는 것.
- *제어 흐름* — 반복을 실패 목록을 돌려주는 하네스 함수로 옮기고 목록이 비었음을 단언한다.
- *단언 없음* — 단언을 더하거나 `#[should_panic(expected = "...")]`.

### 7. 언어
영어로만 쓴 문단·주석·사용자 문구(Rust와 앱·대시보드 프런트엔드)를 한국어로 다시 쓴다. 식별자·명령·경로는 그대로 둔다.
번역본(`-EN.md` 등)은 지운다.

### 8–10. 툴체인 / fmt / clippy
`cargo fmt --all`이 포맷을 고친다. clippy는 코드를 고친다. 정당한 예외는 항목에
`#[expect(clippy::<lint>, reason = "...")]`, 크레이트 전체 `allow`는 안 된다.

### 11. cargo-deny
의존성의 라이선스가 `deny.toml` 허용 목록 밖이다. 다른 크레이트를 고른다. 목록을 넓히는 것은 그것이
필요했던 변경과 별개인 `decision` 이슈다.

### 12. 테스트와 커버리지
- **커버리지 85% 미만** — **하한을 낮추거나 파일을 빼지 않는다.** 동작을 단언하는 테스트를 쓴다
  (`test-author`에 맡긴다). `cargo llvm-cov --html`이 덮이지 않은 줄을 보여준다. OS 어댑터라면 로직을
  순수 함수로 빼서 덮는다.
- **스냅숏 불일치는 회귀로 본다, 반대가 증명되기 전까지.** 일괄 승인하지 않는다. 어떤 규칙·가중치가
  바뀌었는지 찾고, 새 결과가 맞다면 PR에 이유를 쓴다.
- **proptest 실패** — `proptest-regressions/` 항목을 남겨 커밋하고 코드를 고친다.
- **결정성 실패**(두 번 돌린 결과가 다름) — 해시 컨테이너, 벽시계, 동점 정렬, 중앙값 규칙, 부동소수 점수, 센서 보고서
  도착 순서 의존을 찾는다.
- **시나리오 실패** — 규칙 노트의 기대값과 비교한다. 우리 출력이 아니라 노트가 기준이다.

## 마지막

무엇을 돌렸고 결과가 어땠는지, 건너뛴 단계까지 사실대로 보고한다. **통과하지 않았으면 통과했다고 말하지 않는다.**
