#!/usr/bin/env bash
# Compare current coverage metrics against the base commit.
#
# Required environment variables:
#   CURRENT_FUNCTION_COVERAGE, CURRENT_LINE_COVERAGE, CURRENT_REGION_COVERAGE
#   EVENT_NAME          — "pull_request" or "push"
#   BASE_SHA_PR         — base SHA for pull_request events
#   BASE_SHA_PUSH       — base SHA for push events
#   GITHUB_STEP_SUMMARY — path to the step summary file (set by GitHub Actions)

set -euo pipefail

if [ "${EVENT_NAME}" = "pull_request" ]; then
  BASE_SHA="${BASE_SHA_PR}"
else
  BASE_SHA="${BASE_SHA_PUSH}"
fi

if [ -z "${BASE_SHA}" ] || [ "${BASE_SHA}" = "0000000000000000000000000000000000000000" ]; then
  echo "No valid base SHA available. Skipping coverage comparison."
  exit 0
fi

if ! git cat-file -e "${BASE_SHA}^{commit}" 2>/dev/null; then
  echo "Base SHA ${BASE_SHA} not found locally. Fetching remote branch refs..."
  git fetch --no-tags origin "+refs/heads/*:refs/remotes/origin/*" || true
fi

if ! git cat-file -e "${BASE_SHA}^{commit}" 2>/dev/null; then
  echo "Base SHA ${BASE_SHA} is unavailable (likely rewritten history). Skipping coverage comparison."
  exit 0
fi

if [ -z "${CURRENT_FUNCTION_COVERAGE}" ] || [ -z "${CURRENT_LINE_COVERAGE}" ] || [ -z "${CURRENT_REGION_COVERAGE}" ]; then
  echo "Current coverage metrics are unavailable from coverage-report outputs."
  exit 1
fi

if ! git checkout --force "${BASE_SHA}"; then
  echo "Checkout of ${BASE_SHA} failed. Trying to fetch commit object directly..."
  if ! git fetch --no-tags --depth=1 origin "${BASE_SHA}"; then
    echo "Could not fetch base SHA ${BASE_SHA}. Skipping coverage comparison."
    exit 0
  fi
  git checkout --force "${BASE_SHA}" || {
    echo "Checkout still failed after fetch. Skipping coverage comparison."
    exit 0
  }
fi

cargo llvm-cov clean --workspace
cargo llvm-cov --workspace --no-default-features --tests --no-report
cargo llvm-cov --workspace --tests --no-report
cargo llvm-cov --workspace --features impl_from --tests --no-report
cargo llvm-cov --workspace --all-features --tests --no-report
mkdir -p target/llvm-cov
cargo llvm-cov report --json --summary-only --output-path target/llvm-cov/base-summary.json

BASE_LINE_COVERAGE=$(jq -r '.data[0].totals.lines.percent' target/llvm-cov/base-summary.json)
BASE_REGION_COVERAGE=$(jq -r '.data[0].totals.regions.percent' target/llvm-cov/base-summary.json)
BASE_FUNCTION_COVERAGE=$(jq -r '.data[0].totals.functions.percent' target/llvm-cov/base-summary.json)

LINE_DELTA=$(awk -v current="${CURRENT_LINE_COVERAGE}" -v base="${BASE_LINE_COVERAGE}" 'BEGIN { printf "%.4f", current - base }')
REGION_DELTA=$(awk -v current="${CURRENT_REGION_COVERAGE}" -v base="${BASE_REGION_COVERAGE}" 'BEGIN { printf "%.4f", current - base }')
FUNCTION_DELTA=$(awk -v current="${CURRENT_FUNCTION_COVERAGE}" -v base="${BASE_FUNCTION_COVERAGE}" 'BEGIN { printf "%.4f", current - base }')

{
  echo "## Coverage Comparison"
  echo ""
  echo "Current function coverage: ${CURRENT_FUNCTION_COVERAGE}%"
  echo "Base function coverage: ${BASE_FUNCTION_COVERAGE}%"
  echo "Function delta: ${FUNCTION_DELTA}%"
  echo ""
  echo "Current line coverage: ${CURRENT_LINE_COVERAGE}%"
  echo "Base line coverage: ${BASE_LINE_COVERAGE}%"
  echo "Line delta: ${LINE_DELTA}%"
  echo ""
  echo "Current region coverage: ${CURRENT_REGION_COVERAGE}%"
  echo "Base region coverage: ${BASE_REGION_COVERAGE}%"
  echo "Region delta: ${REGION_DELTA}%"
} >> "${GITHUB_STEP_SUMMARY}"

if awk -v current="${CURRENT_LINE_COVERAGE}" -v base="${BASE_LINE_COVERAGE}" 'BEGIN { exit (current + 1e-9 >= base) ? 0 : 1 }'; then
  echo "Coverage status: very good (no line coverage regression)."
  {
    echo ""
    echo "Result: very good"
    echo "Reason: line coverage did not regress."
  } >> "${GITHUB_STEP_SUMMARY}"
  exit 0
fi

echo "Coverage regression detected. Evaluating relaxed thresholds..."
if awk -v fns="${CURRENT_FUNCTION_COVERAGE}" -v line="${CURRENT_LINE_COVERAGE}" -v region="${CURRENT_REGION_COVERAGE}" 'BEGIN { exit (fns + 1e-9 >= 100.0 && line + 1e-9 >= 90.0 && region + 1e-9 >= 90.0) ? 0 : 1 }'; then
  echo "Coverage status: acceptable regression (functions >= 100%, lines >= 90%, regions >= 90%)."
  {
    echo ""
    echo "Result: acceptable"
    echo "Reason: regression exists, but quality thresholds are satisfied."
  } >> "${GITHUB_STEP_SUMMARY}"
  exit 0
fi

echo "Coverage status: failed (regression with thresholds not satisfied)."
{
  echo ""
  echo "Result: failed"
  echo "Reason: regression detected and quality thresholds not met."
  echo "Required for acceptable regression: functions >= 100%, lines >= 90%, regions >= 90%."
} >> "${GITHUB_STEP_SUMMARY}"
exit 1
