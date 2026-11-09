#!/usr/bin/env bash
#
# verify-agent.sh — run the machine-checkable subset of CONTRIBUTING.md.
#
# This is the evidence gate referenced by AGENTS.md §2. A pull request must paste
# this script's result table into its description.
#
# It is deliberately diff-scoped: rules the existing tree cannot satisfy are
# enforced as ratchets on lines you add, so the gate is green on a clean branch
# and red the moment a contribution introduces the pattern.
#
# Usage:
#   ./scripts/verify-agent.sh                # fast lane (~30 s)
#   ./scripts/verify-agent.sh --full         # adds dx check, tests, clippy, mutants
#   ./scripts/verify-agent.sh --base <ref>   # diff against an explicit base
#   ./scripts/verify-agent.sh --allow-deps   # maintainer: permit Cargo.toml/lock
#   ./scripts/verify-agent.sh --allow-frozen # maintainer: permit frozen paths
#
# Environment:
#   BASE_REF     default base ref (overridden by --base)
#   PR_LABELS    comma-separated labels, e.g. "dep-change,area:data"
#   PR_BODY      pull request body, scanned for forbidden commands
#
# Exit status: 0 when there is no FAIL, 1 otherwise.

set -uo pipefail

FULL=0
ALLOW_DEPS=0
ALLOW_FROZEN=0
BASE_REF_OVERRIDE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --full) FULL=1 ;;
    --allow-deps) ALLOW_DEPS=1 ;;
    --allow-frozen) ALLOW_FROZEN=1 ;;
    --base) BASE_REF_OVERRIDE="${2:-}"; shift ;;
    -h|--help) sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done

cd "$(git rev-parse --show-toplevel 2>/dev/null || pwd)" || exit 2

if ! git rev-parse --git-dir >/dev/null 2>&1; then
  echo "verify-agent: not a git repository" >&2
  exit 2
fi

# ---------------------------------------------------------------------------
# Base resolution and change inventory
# ---------------------------------------------------------------------------

resolve_base() {
  if [[ -n "$BASE_REF_OVERRIDE" ]] && git rev-parse --verify -q "$BASE_REF_OVERRIDE" >/dev/null; then
    echo "$BASE_REF_OVERRIDE"; return
  fi
  if [[ -n "${BASE_REF:-}" ]] && git rev-parse --verify -q "$BASE_REF" >/dev/null; then
    echo "$BASE_REF"; return
  fi
  local candidate mb
  for candidate in origin/main origin/master main master; do
    if git rev-parse --verify -q "$candidate" >/dev/null 2>&1; then
      mb=$(git merge-base "$candidate" HEAD 2>/dev/null)
      if [[ -n "$mb" ]]; then echo "$mb"; return; fi
    fi
  done
  git rev-parse HEAD
}

BASE=$(resolve_base)

UNTRACKED=$(git ls-files --others --exclude-standard 2>/dev/null)
CHANGED=$(
  {
    git diff --name-only "$BASE" 2>/dev/null
    printf '%s\n' "$UNTRACKED"
  } | sed '/^$/d' | sort -u
)

changed_any() {
  local p
  for p in "$@"; do
    if grep -qxF "$p" <<<"$CHANGED" || grep -qE "^${p%/}/" <<<"$CHANGED"; then
      return 0
    fi
  done
  return 1
}

# Lines this contribution adds to a single file. New files count in full.
file_added_lines() {
  local file="$1"
  [[ -f "$file" ]] || return 0
  if git ls-files --error-unmatch -- "$file" >/dev/null 2>&1; then
    git diff -U0 "$BASE" -- "$file" 2>/dev/null \
      | grep -E '^\+' | grep -vE '^\+\+\+' || true
  else
    sed 's/^/+/' -- "$file" 2>/dev/null || true
  fi
}

# grep across tracked *and* untracked files. `git grep` only sees tracked ones,
# and a contributor may not have committed yet.
scan() {
  local flags="$1"; shift
  local pattern="$1"; shift
  local paths=("$@")

  git grep -n $flags -E "$pattern" -- "${paths[@]}" 2>/dev/null || true

  local f
  while IFS= read -r f; do
    [[ -z "$f" || ! -f "$f" ]] && continue
    local matched=0 p
    for p in "${paths[@]}"; do
      [[ "$f" == "$p" || "$f" == "$p"/* ]] && matched=1 && break
    done
    [[ $matched -eq 1 ]] || continue
    grep -n $flags -E "$pattern" -- "$f" 2>/dev/null | sed "s|^|$f:|" || true
  done <<<"$UNTRACKED"
}

is_test_path() {
  case "$1" in
    */tests.rs|*/tests/*|tests/*) return 0 ;;
  esac
  return 1
}

# `git grep` and the untracked fallback both emit `path:line:content`, so this
# keeps documentation that *describes* a rule from tripping the rule.
only_source() {
  grep -E '^[^:]+\.(rs|sql):[0-9]+:' || true
}

# ---------------------------------------------------------------------------
# Result collection
# ---------------------------------------------------------------------------

PASSES=()
FAILURES=()
WARNINGS=()

pass() { PASSES+=("$1|$2"); }
fail() { FAILURES+=("$1|$2"); }
warn() { WARNINGS+=("$1|$2"); }

UNWRAP_RE='\.unwrap\(\)|\.expect\('
SQL_KW_RE='select |insert |update |delete | from | where |order by|join '
COMMENT_RE='//.*(This function|This method|Returns the|§[0-9])'

# ---------------------------------------------------------------------------
# 1. Formatting
# ---------------------------------------------------------------------------

if [[ "${SKIP_FMT:-0}" == "1" ]]; then
  warn "§4 fmt" "skipped (SKIP_FMT=1; the CI formatting job owns this gate)"
elif ! command -v cargo >/dev/null 2>&1; then
  warn "§4 fmt" "cargo not found; skipped"
elif cargo fmt --all --check >/dev/null 2>&1; then
  pass "§4 fmt" "cargo fmt --all --check"
else
  fail "§4 fmt" "run 'cargo fmt --all' (and 'dx fmt' for changed RSX files)"
fi

# ---------------------------------------------------------------------------
# 2. Frozen paths
# ---------------------------------------------------------------------------

FROZEN_LIST=".github/frozen-paths.txt"

if [[ ! -f "$FROZEN_LIST" ]]; then
  warn "§0 frozen" "$FROZEN_LIST missing; skipped"else
  FROZEN_HITS=""
  while IFS= read -r raw; do
    pattern="${raw%%#*}"
    pattern="$(printf '%s' "$pattern" | tr -d '[:space:]')"
    [[ -z "$pattern" ]] && continue
    while IFS= read -r file; do
      [[ -z "$file" ]] && continue
      # shellcheck disable=SC2254 # intentional glob matching
      case "$file" in
        $pattern) FROZEN_HITS+="$file ";;
      esac
    done <<<"$CHANGED"
  done <"$FROZEN_LIST"

  if [[ -z "$FROZEN_HITS" ]]; then
    pass "§0 frozen" "no frozen path modified"
  elif [[ $ALLOW_FROZEN -eq 1 ]]; then
    warn "§0 frozen" "modified with --allow-frozen: $FROZEN_HITS"
  else
    fail "§0 frozen" "frozen path modified: $FROZEN_HITS — open an issue instead"
  fi
fi

# ---------------------------------------------------------------------------
# 3. Migration history is append-only
# ---------------------------------------------------------------------------

MIGRATION_EDITS=$(git diff --name-only --diff-filter=MDR "$BASE" -- src/data/migrations 2>/dev/null | tr '\n' ' ')
if [[ -z "${MIGRATION_EDITS// /}" ]]; then
  pass "§6.5 migrations" "no shipped migration edited"
else
  fail "§6.5 migrations" "shipped migration modified or deleted: $MIGRATION_EDITS"
fi

# ---------------------------------------------------------------------------
# 4. No routine unwrap()/expect() in added production code
# ---------------------------------------------------------------------------

UNWRAP_HITS=""
while IFS= read -r file; do
  [[ -z "$file" ]] && continue
  [[ "$file" == *.rs ]] || continue
  is_test_path "$file" && continue
  hits=$(file_added_lines "$file" | grep -E "$UNWRAP_RE" | grep -v 'allow-unwrap' || true)
  if [[ -n "$hits" ]]; then
    UNWRAP_HITS+="$file: $(head -2 <<<"$hits" | tr '\n' ' ') "
  fi
done <<<"$CHANGED"

if [[ -z "$UNWRAP_HITS" ]]; then
  pass "§8.2 errors" "no new unwrap()/expect() outside tests"
else
  fail "§8.2 errors" "new unwrap()/expect(): $UNWRAP_HITS"
fi

# ---------------------------------------------------------------------------
# 5. No SQL values interpolated into queries
# ---------------------------------------------------------------------------

SQL_HITS=""
while IFS= read -r file; do
  [[ -z "$file" ]] && continue
  [[ "$file" == src/data/*.rs ]] || continue
  hits=$(file_added_lines "$file" \
    | grep -E 'format!\(' \
    | grep -iE "$SQL_KW_RE" \
    | grep -v 'allow-sql-format' || true)
  [[ -n "$hits" ]] && SQL_HITS+="$file: $(head -1 <<<"$hits" | cut -c1-80) "
done <<<"$CHANGED"

if [[ -z "$SQL_HITS" ]]; then
  pass "§6.3 sql" "no new interpolated SQL"
else
  fail "§6.3 sql" "SQL built with format!: $SQL_HITS"
fi

# ---------------------------------------------------------------------------
# 6. No SELECT *
# ---------------------------------------------------------------------------

SELECT_STAR=$(scan "-i" 'SELECT \*' src crates tools | only_source | head -2 || true)
if [[ -z "$SELECT_STAR" ]]; then
  pass "§6.3 select" "no SELECT *"
else
  fail "§6.3 select" "SELECT * at: $(tr '\n' ' ' <<<"$SELECT_STAR")"
fi

# ---------------------------------------------------------------------------
# 7. No native filesystem APIs in browser code
# ---------------------------------------------------------------------------

WASM_HITS=$(scan "" 'std::fs|dirs::|std::path::Path|DATABASE_PATH' crates/web-worker/src src/web crates/core/src | only_source | head -2 || true)
if [[ -z "$WASM_HITS" ]]; then
  pass "§3 wasm-fs" "no native filesystem APIs in browser code"
else
  fail "§3 wasm-fs" "native fs API in browser code: $(tr '\n' ' ' <<<"$WASM_HITS")"
fi

# ---------------------------------------------------------------------------
# 8. USD formatting stays inside the formatter
# ---------------------------------------------------------------------------

USD_HITS=$(scan "" 'format_usd\(' src | only_source | grep -v '^src/components/format.rs:' | head -2 || true)
if [[ -z "$USD_HITS" ]]; then
  pass "§3.1 fiat" "format_usd used only inside format.rs"
else
  fail "§3.1 fiat" "format_usd called outside format.rs: $(tr '\n' ' ' <<<"$USD_HITS")"
fi

# ---------------------------------------------------------------------------
# 9. The corrupting formatter command is never used
# ---------------------------------------------------------------------------

FMT_ALL_CODE=$(git log --format=%B "$BASE..HEAD" 2>/dev/null | grep -c 'dx fmt --all-code' || true)
BODY_HIT=0
if [[ -n "${PR_BODY:-}" ]] && grep -q 'dx fmt --all-code' <<<"$PR_BODY"; then
  BODY_HIT=1
fi

if [[ "${FMT_ALL_CODE:-0}" -eq 0 && $BODY_HIT -eq 0 ]]; then
  pass "§4.1 dx-fmt" "dx fmt --all-code not used"
else
  fail "§4.1 dx-fmt" "dx fmt --all-code corrupts rsx! handlers; use 'cargo fmt --all' + 'dx fmt'"
fi

# ---------------------------------------------------------------------------
# 10. No dependency changes
# ---------------------------------------------------------------------------

DEP_FILES=$(grep -xE 'Cargo\.toml|Cargo\.lock' <<<"$CHANGED" | tr '\n' ' ')
if [[ -z "${DEP_FILES// /}" ]]; then
  pass "§3 deps" "no dependency change"
elif [[ $ALLOW_DEPS -eq 1 ]] || grep -q 'dep-change' <<<"${PR_LABELS:-}"; then
  warn "§3 deps" "dependency files changed ($DEP_FILES) with dep-change approval"
else
  fail "§3 deps" "dependency files changed ($DEP_FILES); needs the 'dep-change' label"
fi

# ---------------------------------------------------------------------------
# 11. Comment discipline on added lines
# ---------------------------------------------------------------------------

COMMENT_HITS=""
while IFS= read -r file; do
  [[ -z "$file" ]] && continue
  [[ "$file" == *.rs ]] || continue
  is_test_path "$file" && continue
  hits=$(file_added_lines "$file" | grep -E "$COMMENT_RE" | head -1 || true)
  [[ -n "$hits" ]] && COMMENT_HITS+="$file: $(cut -c1-70 <<<"$hits") "
done <<<"$CHANGED"

if [[ -z "$COMMENT_HITS" ]]; then
  pass "§8.3 comments" "added comments explain why, not what"
else
  fail "§8.3 comments" "restated signature or section reference: $COMMENT_HITS"
fi

# ---------------------------------------------------------------------------
# 12. API surface version (advisory)
# ---------------------------------------------------------------------------

if changed_any src/api crates/web-worker/src/rpc.rs; then
  if changed_any src/domain/health.rs crates/core/src/lib.rs; then
    pass "§5.15 api-version" "API or RPC surface changed with a version file"
  else
    warn "§5.15 api-version" "src/api or rpc.rs changed without API_VERSION/WORKER_PROTOCOL_VERSION — confirm the wire shape is unchanged"
  fi
else
  pass "§5.15 api-version" "no transport surface change"
fi

# ---------------------------------------------------------------------------
# 13. Native / web route parity (advisory)
# ---------------------------------------------------------------------------

# `/api/health` is the bootstrap probe, served before the wrapper layer, so it is
# intentionally absent from the `*_api` wrapper set.
native_routes=$(
  git grep -hoE '#\[(get|post|put|delete)\("[^"]*"\)\]' -- 'src/api/*.rs' 2>/dev/null \
    | sed -E 's/^#\[[a-z]+\("//; s/"\)\]$//; s/\?.*$//; s/:[a-z_]+/{}/g; s/\{[a-z_]+\}/{}/g' \
    | LC_ALL=C sort -u | grep -v '^/api/health$' || true
)
web_routes=$(
  git grep -hoE '"/api/[^"?]*"' -- 'src/api/web/*.rs' 2>/dev/null \
    | tr -d '"' | sed -E 's/\{[a-z_]+\}/{}/g' \
    | LC_ALL=C sort -u || true
)

if [[ -z "$native_routes" || -z "$web_routes" ]]; then
  warn "§9 parity" "could not enumerate routes; skipped"
else
  parity_delta=$(LC_ALL=C comm -3 <(printf '%s\n' "$native_routes") <(printf '%s\n' "$web_routes") 2>/dev/null | tr -d '[:blank:]' | sed '/^$/d' | paste -sd' ' - || true)
  native_count=$(printf '%s\n' "$native_routes" | grep -c . || true)
  if [[ -z "${parity_delta:-}" ]]; then
    pass "§9 parity" "$native_count native routes mirrored by web wrappers"
  else
    warn "§9 parity" "route sets differ: $(printf '%s' "$parity_delta" | head -c 200)"
  fi
fi

# ---------------------------------------------------------------------------
# 14. Full lane
# ---------------------------------------------------------------------------

if [[ $FULL -eq 1 ]]; then
  if command -v dx >/dev/null 2>&1; then
    if dx check --platform web >/dev/null 2>&1; then
      pass "§4 dx-check web" "dx check --platform web"
    else
      fail "§4 dx-check web" "dx check --platform web failed"
    fi
    if dx check --platform desktop >/dev/null 2>&1; then
      pass "§4 dx-check desktop" "dx check --platform desktop"
    else
      fail "§4 dx-check desktop" "dx check --platform desktop failed"
    fi
  else
    warn "§4 dx-check" "dx not found; run 'cargo prepare'"
  fi

  if cargo test -p address-explorer --no-default-features --features server >/dev/null 2>&1; then
    pass "§4 tests server" "cargo test --features server"
  else
    fail "§4 tests server" "cargo test --features server failed"
  fi

  if cargo clippy -p address-explorer --all-targets --no-default-features --features server -- -D warnings >/dev/null 2>&1; then
    pass "§4 clippy server" "clippy clean"
  else
    fail "§4 clippy server" "clippy reported warnings or errors"
  fi

  if command -v cargo-mutants >/dev/null 2>&1; then
    if git diff -U0 "$BASE" -- 'src/**/*.rs' 'crates/**/*.rs' | cargo mutants --in-diff - >/dev/null 2>&1; then
      pass "§9 mutants" "no surviving mutants in changed code"
    else
      fail "§9 mutants" "changed lines contain mutants the tests do not catch"
    fi
  else
    warn "§9 mutants" "cargo-mutants not installed; skipped"
  fi
fi

# ---------------------------------------------------------------------------
# Report
# ---------------------------------------------------------------------------

row() { printf '%-6s %-20s %s\n' "$1" "$2" "$3"; }

echo
echo "CONTRIBUTING compliance"
echo "base ${BASE:0:12}  files $(grep -c . <<<"$CHANGED" || echo 0)  lane $([[ $FULL -eq 1 ]] && echo full || echo fast)"
echo "----------------------------------------------------------------------------"
for entry in "${PASSES[@]}"; do row "[ok]" "${entry%%|*}" "${entry#*|}"; done
for entry in "${WARNINGS[@]}"; do row "[warn]" "${entry%%|*}" "${entry#*|}"; done
for entry in "${FAILURES[@]}"; do row "[FAIL]" "${entry%%|*}" "${entry#*|}"; done
echo "----------------------------------------------------------------------------"

total=$(( ${#PASSES[@]} + ${#WARNINGS[@]} + ${#FAILURES[@]} ))
echo "RESULT: $(( ${#PASSES[@]} + ${#WARNINGS[@]} ))/$total pass, ${#FAILURES[@]} failure(s)"
echo

if [[ ${#FAILURES[@]} -gt 0 ]]; then
  echo "Not ready for review. Fix the failures above, then re-run."
  exit 1
fi

echo "Ready for review. Paste the table above into the pull request."
exit 0
