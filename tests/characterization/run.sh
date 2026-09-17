#!/usr/bin/env bash
# shellcheck source-path=SCRIPTDIR

set -u

CHARACTERIZATION_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
CHARACTERIZATION_REPO_ROOT="$(cd "$CHARACTERIZATION_ROOT/../.." && pwd -P)"
CHARACTERIZATION_RUNNER="$CHARACTERIZATION_ROOT/run.sh"
CHARACTERIZATION_STUB_DOCKER="$CHARACTERIZATION_ROOT/stubs/docker"
CHARACTERIZATION_STUB_DOCKER_DENY="$CHARACTERIZATION_ROOT/stubs/docker-deny"
CHARACTERIZATION_CHILD_DRIVER="$CHARACTERIZATION_ROOT/helpers/child-driver.bash"
CHARACTERIZATION_ORIGINAL_PATH="$PATH"
CHARACTERIZATION_BASH="$(type -P bash)"

if [[ "${DSTACK_TEST_INTERNAL_BROKEN_DOCKER_STUB:-0}" == "1" ]]; then
  CHARACTERIZATION_STUB_DOCKER="$CHARACTERIZATION_ROOT/stubs/intentionally-missing-docker"
fi

# These paths are computed at runtime; source annotations let ShellCheck inspect
# the complete runner without changing how the suite locates its files.
# shellcheck source=helpers/assertions.bash
source "$CHARACTERIZATION_ROOT/helpers/assertions.bash"
# shellcheck source=helpers/capture.bash
source "$CHARACTERIZATION_ROOT/helpers/capture.bash"
# shellcheck source=helpers/docker-records.bash
source "$CHARACTERIZATION_ROOT/helpers/docker-records.bash"
# shellcheck source=helpers/environment.bash
source "$CHARACTERIZATION_ROOT/helpers/environment.bash"
# shellcheck source=helpers/fixtures.bash
source "$CHARACTERIZATION_ROOT/helpers/fixtures.bash"
# shellcheck source=helpers/child-bash.bash
source "$CHARACTERIZATION_ROOT/helpers/child-bash.bash"
# shellcheck source=helpers/dstack-invoke.bash
source "$CHARACTERIZATION_ROOT/helpers/dstack-invoke.bash"

TEST_CASE_NAMES=()
TEST_CASE_GROUPS=()
TEST_CASE_FUNCTIONS=()

register_case() {
  local group="$1"
  local name="$2"
  local function_name="$3"

  TEST_CASE_GROUPS+=("$group")
  TEST_CASE_NAMES+=("$name")
  TEST_CASE_FUNCTIONS+=("$function_name")
}

# shellcheck source=cases/harness.bash
source "$CHARACTERIZATION_ROOT/cases/harness.bash"
# shellcheck source=cases/load.bash
source "$CHARACTERIZATION_ROOT/cases/load.bash"
# shellcheck source=cases/registry.bash
source "$CHARACTERIZATION_ROOT/cases/registry.bash"
# shellcheck source=cases/resolution.bash
source "$CHARACTERIZATION_ROOT/cases/resolution.bash"
# shellcheck source=cases/compose-discovery.bash
source "$CHARACTERIZATION_ROOT/cases/compose-discovery.bash"
# shellcheck source=cases/compose-selectors.bash
source "$CHARACTERIZATION_ROOT/cases/compose-selectors.bash"
# shellcheck source=cases/compose-precedence.bash
source "$CHARACTERIZATION_ROOT/cases/compose-precedence.bash"
# shellcheck source=cases/forwarding.bash
source "$CHARACTERIZATION_ROOT/cases/forwarding.bash"
# shellcheck source=cases/streams-status.bash
source "$CHARACTERIZATION_ROOT/cases/streams-status.bash"
# shellcheck source=cases/state.bash
source "$CHARACTERIZATION_ROOT/cases/state.bash"

_runner_usage() {
  cat <<'EOF'
Usage:
  bash tests/characterization/run.sh [--debug] [all|harness|load|registry|resolution|compose|forwarding|status]
  bash tests/characterization/run.sh [--debug] harness
  bash tests/characterization/run.sh [--debug] load
  bash tests/characterization/run.sh [--debug] registry
  bash tests/characterization/run.sh [--debug] resolution
  bash tests/characterization/run.sh [--debug] compose
  bash tests/characterization/run.sh [--debug] forwarding
  bash tests/characterization/run.sh [--debug] status
  bash tests/characterization/run.sh [--debug] case <case-name>

--debug preserves failed case artifacts. Successful case state is always removed.
EOF
}

RUNNER_DEBUG=0
if [[ "${1:-}" == "--debug" ]]; then
  RUNNER_DEBUG=1
  shift
fi

selection="${1:-all}"
if [[ $# -gt 0 ]]; then
  shift
fi

SELECTED_CASE_INDEXES=()
case "$selection" in
  all)
    if (($#)); then
      _runner_usage >&2
      exit 2
    fi
    for index in "${!TEST_CASE_NAMES[@]}"; do
      [[ "${TEST_CASE_GROUPS[$index]}" != "internal" ]] && SELECTED_CASE_INDEXES+=("$index")
    done
    ;;
  harness|load|registry|resolution|compose|forwarding|status)
    if (($#)); then
      _runner_usage >&2
      exit 2
    fi
    for index in "${!TEST_CASE_NAMES[@]}"; do
      [[ "${TEST_CASE_GROUPS[$index]}" == "$selection" ]] && SELECTED_CASE_INDEXES+=("$index")
    done
    ;;
  case)
    if [[ $# -ne 1 ]]; then
      _runner_usage >&2
      exit 2
    fi
    requested_case="$1"
    for index in "${!TEST_CASE_NAMES[@]}"; do
      [[ "${TEST_CASE_NAMES[$index]}" == "$requested_case" ]] && SELECTED_CASE_INDEXES+=("$index")
    done
    if ((${#SELECTED_CASE_INDEXES[@]} == 0)); then
      printf 'Unknown characterization case: %s\n' "$requested_case" >&2
      exit 2
    fi
    ;;
  -h|--help|help)
    _runner_usage
    exit 0
    ;;
  *)
    _runner_usage >&2
    exit 2
    ;;
esac

if ((${#SELECTED_CASE_INDEXES[@]} == 0)); then
  printf 'No characterization cases selected.\n' >&2
  exit 2
fi

RUNNER_TEMP_BASE="${DSTACK_TEST_TMPDIR:-${RUNNER_TEMP:-${TMPDIR:-/tmp}}}"
mkdir -p "$RUNNER_TEMP_BASE"
RUNNER_TEMP_BASE="$(cd "$RUNNER_TEMP_BASE" && pwd -P)"
RUNNER_SUITE_ROOT="$(mktemp -d "$RUNNER_TEMP_BASE/dstack-characterization.XXXXXX")"
: >"$RUNNER_SUITE_ROOT/.dstack-characterization-suite"
RUNNER_PRESERVE_SUITE=0

# Invoked indirectly by the EXIT trap; ShellCheck versions report this as
# SC2317 or SC2329 because they do not recognize the trap callback.
# shellcheck disable=SC2317,SC2329
_runner_exit_cleanup() {
  if ((RUNNER_PRESERVE_SUITE == 0)) && [[ -d "$RUNNER_SUITE_ROOT" ]]; then
    cleanup_suite_root "$RUNNER_SUITE_ROOT" "$RUNNER_TEMP_BASE" || true
  fi
}
trap _runner_exit_cleanup EXIT
trap 'exit 130' HUP INT TERM

_runner_print_case_diagnostics() {
  local label="$1"
  local file="$2"
  local line

  [[ -s "$file" ]] || return 0
  printf '# %s:\n' "$label"
  while IFS= read -r line || [[ -n "$line" ]]; do
    printf '#   %s\n' "$line"
  done <"$file"
}

passed=0
failed=0
test_number=0
aborted=0

printf 'TAP version 13\n'
printf '1..%d\n' "${#SELECTED_CASE_INDEXES[@]}"

for index in "${SELECTED_CASE_INDEXES[@]}"; do
  test_number=$((test_number + 1))
  case_name="${TEST_CASE_NAMES[$index]}"
  case_function="${TEST_CASE_FUNCTIONS[$index]}"

  if ! create_case_root "$RUNNER_SUITE_ROOT"; then
    printf 'not ok %d - %s\n' "$test_number" "$case_name"
    printf 'Bail out! Could not create an isolated case root.\n'
    aborted=1
    failed=$((failed + 1))
    break
  fi
  printf '%s\n' "$case_name" >"$CASE_ROOT/case-name"
  case_stdout="$CASE_CAPTURE_DIR/case.stdout"
  case_stderr="$CASE_CAPTURE_DIR/case.stderr"

  if (
    ASSERTION_SEQUENCE=0
    TEST_CASE_FAILED=0
    prepare_case_environment || exit 97
    "$case_function"
    function_status=$?
    if ((function_status != 0 && TEST_CASE_FAILED == 0)); then
      printf '# case function returned status %s without an assertion failure\n' "$function_status" >&2
    fi
    ((function_status == 0 && TEST_CASE_FAILED == 0))
  ) >"$case_stdout" 2>"$case_stderr"; then
    case_status=0
  else
    case_status=$?
  fi

  if ((case_status == 97)); then
    printf 'not ok %d - %s\n' "$test_number" "$case_name"
    _runner_print_case_diagnostics stderr "$case_stderr"
    printf 'Bail out! Fake Docker could not be proven active; suite stopped.\n'
    failed=$((failed + 1))
    aborted=1
    if ((RUNNER_DEBUG)); then
      RUNNER_PRESERVE_SUITE=1
    else
      cleanup_case_root "$CASE_ROOT" "$RUNNER_SUITE_ROOT" || true
    fi
    break
  fi

  if ((case_status == 0)); then
    if cleanup_case_root "$CASE_ROOT" "$RUNNER_SUITE_ROOT"; then
      printf 'ok %d - %s\n' "$test_number" "$case_name"
      passed=$((passed + 1))
    else
      printf 'not ok %d - %s\n' "$test_number" "$case_name"
      printf '# failed to clean successful case root safely: %s\n' "$CASE_ROOT"
      failed=$((failed + 1))
      RUNNER_PRESERVE_SUITE=1
    fi
  else
    printf 'not ok %d - %s\n' "$test_number" "$case_name"
    _runner_print_case_diagnostics stdout "$case_stdout"
    _runner_print_case_diagnostics stderr "$case_stderr"
    failed=$((failed + 1))
    if ((RUNNER_DEBUG)); then
      RUNNER_PRESERVE_SUITE=1
      printf '# failed case artifacts: %s\n' "$CASE_ROOT"
    else
      cleanup_case_root "$CASE_ROOT" "$RUNNER_SUITE_ROOT" || {
        printf '# failed to clean failed case root safely: %s\n' "$CASE_ROOT"
        RUNNER_PRESERVE_SUITE=1
      }
    fi
  fi
done

printf '# %d passed, %d failed\n' "$passed" "$failed"

if ((RUNNER_PRESERVE_SUITE)); then
  printf '# suite artifacts: %s\n' "$RUNNER_SUITE_ROOT"
fi

if ((aborted || failed)); then
  exit 1
fi
exit 0
