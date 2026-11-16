#!/usr/bin/env bash
# 유일한 게이트 (AGENTS.md §1). CI(.github/workflows/ci.yml)도 이 스크립트를 그대로 돌린다.
#
# 단계는 순서대로 돌고 첫 실패에서 멈춘다: 앞 단계를 고치면 뒤 단계의 결과가 바뀐다. Phase 0에서
# Cargo.toml이 생기기 전까지 cargo 단계는 SKIPPED를 출력한다 — 통과한 것이 아니라 돌지 않은 것이다.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
PYTHON="${PYTHON:-python3}"
export PYTHONDONTWRITEBYTECODE=1
# 게이트 출력은 한국어다. Windows 콘솔 기본 코드 페이지(cp1252 등)로는 찍지 못해 멈추므로 UTF-8로 고정한다.
export PYTHONUTF8=1 PYTHONIOENCODING=utf-8

step() { printf '\n▶ %s\n' "$1"; }
skip() { printf '  SKIPPED: %s\n' "$1"; }

step "에이전트 설정"
"$PYTHON" scripts/check_agent_config.py
step "훅 selftest"
"$PYTHON" scripts/agent_hooks/selftest.py
step "수동성 스캔 (추적 파일)"
"$PYTHON" scripts/agent_hooks/guard_passive.py --scan-tracked
step "아키텍처 (core 순수성, 크레이트 그래프, 이름)"
"$PYTHON" scripts/agent_hooks/check_architecture.py --all
step "수동 감지 코드 (송신·디코딩, 실제 MAC, 단정 문구)"
"$PYTHON" scripts/agent_hooks/check_passive.py --all
step "테스트 규칙 (GWT 이름, 제어 흐름 없음, 단언)"
"$PYTHON" scripts/agent_hooks/check_tests.py --all
step "언어 (한국어 전용)"
"$PYTHON" scripts/agent_hooks/check_language.py --all

if [ -f desktop/package.json ]; then
  # 화면(Electron)의 순수 함수 시험. 의존성 설치 없이 Node 내장 시험 도구로 돈다.
  step "화면 시험 (desktop, node --test)"
  command -v node >/dev/null 2>&1 || { echo "node가 없다 — Node.js 22 이상을 설치한다" >&2; exit 1; }
  # 폴더 경로는 Node 22가 모듈로 읽어 실패한다 — 파일 패턴을 따옴표로 넘겨 node가 직접 펼치게 한다(셸·OS와 무관).
  (cd desktop && node --test 'test/*.test.js')
fi

if [ ! -f Cargo.toml ]; then
  step "cargo fmt · clippy · deny · 테스트 + 커버리지"
  skip "아직 Cargo 작업 공간이 없다 (Phase 0에서 만든다)"
  printf '\nverify: Python 게이트 통과, Rust 게이트는 건너뜀.\n'
  exit 0
fi

command -v cargo >/dev/null 2>&1 || { echo "cargo가 없다 — Rust 툴체인을 설치한다" >&2; exit 1; }

# 선택 기능(sdr)까지 모두 빌드한다 (AGENTS.md §2 규칙 8). Windows의 시스템 라이브러리 설치가 정해질 때까지
# (Phase 3) Windows는 기본 빌드만 돌린다 — 이 예외는 그때 없앤다.
case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*) FEATURES=() ;;
  *) FEATURES=(--all-features) ;;
esac

step "툴체인 고정"
# AGENTS.md §4: 떠 있는 채널이 아니라 정확한 버전.
if ! grep -Eq '^channel[[:space:]]*=[[:space:]]*"1\.[0-9]+\.[0-9]+"' rust-toolchain.toml 2>/dev/null; then
  echo 'rust-toolchain.toml은 정확한 버전을 고정해야 한다: channel = "1.NN.0"' >&2
  exit 1
fi
echo "  $(grep -E '^channel' rust-toolchain.toml)"

step "cargo fmt"
cargo fmt --all -- --check
step "cargo clippy (기본 빌드)"
cargo clippy --workspace --all-targets --locked -- -D warnings
if [ "${#FEATURES[@]}" -gt 0 ]; then
  step "cargo clippy (선택 기능 포함)"
  cargo clippy --workspace --all-targets --locked "${FEATURES[@]}" -- -D warnings \
    || { echo "선택 기능 빌드 실패 — SoapySDR 개발 파일이 설치돼 있는지 확인한다" >&2; exit 1; }
fi

need() {
  "$@" >/dev/null 2>&1 || { printf '%s가 필요하다: %s\n' "$1 $2" "$INSTALL" >&2; exit 1; }
}

step "의존성 라이선스와 출처 (cargo-deny)"
INSTALL="cargo install --locked cargo-deny"
need cargo deny --version
cargo deny --locked check licenses bans sources

# 줄 커버리지 하한 (AGENTS.md §5.9). 이 숫자와 AGENTS.md의 숫자가 다르면
# scripts/check_agent_config.py가 실패한다 — 낮추는 것은 편집이 아니라 결정이다.
COVERAGE_FLOOR=85
step "테스트 + 커버리지 (하한 ${COVERAGE_FLOOR}%)"
INSTALL="cargo install --locked cargo-llvm-cov && rustup component add llvm-tools-preview"
need cargo llvm-cov --version
if cargo nextest --version >/dev/null 2>&1; then
  cargo llvm-cov nextest --workspace --locked ${FEATURES[@]+"${FEATURES[@]}"} --fail-under-lines "$COVERAGE_FLOOR"
else
  cargo llvm-cov --workspace --locked ${FEATURES[@]+"${FEATURES[@]}"} --fail-under-lines "$COVERAGE_FLOOR"
fi
step "문서 테스트"
cargo test --workspace --locked --doc

printf '\nverify: 모든 게이트 통과.\n'
