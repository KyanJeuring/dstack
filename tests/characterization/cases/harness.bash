#!/usr/bin/env bash

_selftest_file_contains() {
  local file="$1"
  local text="$2"
  local label="${3:-$file}"

  if ! grep -Fq "$text" "$file"; then
    fail "$label: expected to contain $(printf '%q' "$text")"
    return 1
  fi
}

_selftest_count_suites() {
  local directory="$1"
  local had_nullglob=0
  local -a suites=()

  shopt -q nullglob && had_nullglob=1
  shopt -s nullglob
  suites=("$directory"/dstack-characterization.*)
  ((had_nullglob)) || shopt -u nullglob
  SELFTEST_SUITE_COUNT=${#suites[@]}
  SELFTEST_SUITES=("${suites[@]}")
}

_selftest_nested_runner() {
  local prefix="$1"
  local temp_base="$2"
  shift 2

  mkdir -p "$temp_base"
  capture_command "$prefix" \
    env \
      DSTACK_TEST_INTERNAL_PROBES=1 \
      DSTACK_TEST_TMPDIR="$temp_base" \
      "$CHARACTERIZATION_BASH" "$CHARACTERIZATION_RUNNER" "$@"
}

_selftest_record_arguments() {
  local output="$1"
  shift
  write_nul_arguments "$output" "$@"
}

_selftest_missing_guard() (
  unset DSTACK_TEST_DOCKER_GUARD
  "$FAKE_DOCKER_PATH" should-not-record
)

_selftest_wrong_guard() (
  DSTACK_TEST_DOCKER_GUARD="wrong-$DSTACK_TEST_DOCKER_GUARD"
  export DSTACK_TEST_DOCKER_GUARD
  "$FAKE_DOCKER_PATH" should-not-record
)

_selftest_capture_zero_status() {
  local prefix="$CASE_CAPTURE_DIR/zero"

  capture_command "$prefix" "$CHARACTERIZATION_BASH" -c 'exit 0'
  assert_status 0 "$CAPTURE_STATUS" "captured zero status"
  assert_text_file "$CAPTURE_STATUS_FILE" $'0\n' "zero status file"
  assert_file_empty "$CAPTURE_STDOUT" "zero-status stdout"
  assert_file_empty "$CAPTURE_STDERR" "zero-status stderr"
}

_selftest_capture_nonzero_status() {
  local prefix="$CASE_CAPTURE_DIR/nonzero"

  capture_command "$prefix" "$CHARACTERIZATION_BASH" -c 'exit 37'
  assert_status 37 "$CAPTURE_STATUS" "captured nonzero status"
  assert_text_file "$CAPTURE_STATUS_FILE" $'37\n' "nonzero status file"
  assert_file_empty "$CAPTURE_STDOUT" "nonzero-status stdout"
  assert_file_empty "$CAPTURE_STDERR" "nonzero-status stderr"
}

_selftest_capture_streams() {
  local prefix="$CASE_CAPTURE_DIR/streams"

  capture_command "$prefix" "$CHARACTERIZATION_BASH" -c \
    'printf %s stdout-value; printf %s stderr-value >&2'
  assert_status 0 "$CAPTURE_STATUS"
  assert_text_file "$CAPTURE_STDOUT" "stdout-value" "captured stdout"
  assert_text_file "$CAPTURE_STDERR" "stderr-value" "captured stderr"
}

_selftest_capture_empty_streams() {
  local prefix="$CASE_CAPTURE_DIR/empty-streams"

  capture_command "$prefix" "$CHARACTERIZATION_BASH" -c ':'
  assert_status 0 "$CAPTURE_STATUS"
  assert_file_empty "$CAPTURE_STDOUT" "empty stdout"
  assert_file_empty "$CAPTURE_STDERR" "empty stderr"
}

_selftest_trailing_newlines() {
  local prefix="$CASE_CAPTURE_DIR/trailing-newlines"
  local expected="$CASE_CAPTURE_DIR/trailing-newlines.expected"

  printf 'line\n\n\n' >"$expected"
  capture_command "$prefix" "$CHARACTERIZATION_BASH" -c "printf 'line\\n\\n\\n'"
  assert_status 0 "$CAPTURE_STATUS"
  assert_bytes_equal "$expected" "$CAPTURE_STDOUT" "multiple trailing newlines"
  assert_file_empty "$CAPTURE_STDERR"
}

_selftest_preserve_empty_argument() {
  local actual="$CASE_CAPTURE_DIR/empty-argument.actual.nul"
  local expected="$CASE_CAPTURE_DIR/empty-argument.expected.nul"

  _selftest_record_arguments "$actual" first "" third
  write_nul_arguments "$expected" first "" third
  assert_bytes_equal "$expected" "$actual" "empty argument boundaries"
}

_selftest_preserve_space_argument() {
  local actual="$CASE_CAPTURE_DIR/space-argument.actual.nul"
  local expected="$CASE_CAPTURE_DIR/space-argument.expected.nul"

  _selftest_record_arguments "$actual" "one argument with spaces" $'tab\targument'
  write_nul_arguments "$expected" "one argument with spaces" $'tab\targument'
  assert_bytes_equal "$expected" "$actual" "space and tab argument boundaries"
}

_selftest_preserve_metacharacters() {
  local actual="$CASE_CAPTURE_DIR/metacharacters.actual.nul"
  local expected="$CASE_CAPTURE_DIR/metacharacters.expected.nul"

  _selftest_record_arguments "$actual" -leading '*' '$HOME' ';' '|' '&&'
  write_nul_arguments "$expected" -leading '*' '$HOME' ';' '|' '&&'
  assert_bytes_equal "$expected" "$actual" "literal shell metacharacters"
}

_selftest_assertion_helpers() {
  local empty="$CASE_CAPTURE_DIR/empty"
  local expected="$CASE_CAPTURE_DIR/bytes.expected"
  local actual="$CASE_CAPTURE_DIR/bytes.actual"
  local ASSERTION_VALUE="value with spaces"

  : >"$empty"
  printf 'a\0b\n' >"$expected"
  cp "$expected" "$actual"

  assert_status 7 7
  assert_file_empty "$empty"
  assert_text_file "$empty" ""
  assert_bytes_equal "$expected" "$actual"
  assert_var_set ASSERTION_VALUE "value with spaces"
  unset ASSERTION_MISSING
  assert_var_unset ASSERTION_MISSING
}

_selftest_expected_assertion_failure() (
  TEST_CASE_FAILED=0
  assert_status 0 9 "deliberate assertion probe" || true
  [[ "$TEST_CASE_FAILED" -eq 1 ]]
)

_selftest_assertion_failure_diagnostic() {
  local prefix="$CASE_CAPTURE_DIR/assertion-failure"

  capture_command "$prefix" _selftest_expected_assertion_failure
  assert_status 0 "$CAPTURE_STATUS"
  _selftest_file_contains "$CAPTURE_STDERR" \
    "deliberate assertion probe: expected status 0, got 9" \
    "assertion diagnostic"
}

_selftest_runner_success_reporting() {
  local prefix="$CASE_CAPTURE_DIR/nested-success"
  local temp_base="$CASE_PROJECTS/nested-success-temp"

  _selftest_nested_runner "$prefix" "$temp_base" case harness::__passing_probe
  assert_status 0 "$CAPTURE_STATUS" "passing nested runner"
  _selftest_file_contains "$CAPTURE_STDOUT" \
    "ok 1 - harness::__passing_probe" "passing TAP output"
  _selftest_file_contains "$CAPTURE_STDOUT" \
    "# 1 passed, 0 failed" "passing TAP summary"
  _selftest_count_suites "$temp_base"
  assert_status 0 "$SELFTEST_SUITE_COUNT" "successful suite cleanup count"
}

_selftest_runner_failure_reporting() {
  local prefix="$CASE_CAPTURE_DIR/nested-failure"
  local temp_base="$CASE_PROJECTS/nested-failure-temp"

  _selftest_nested_runner "$prefix" "$temp_base" case harness::__failing_probe
  assert_status 1 "$CAPTURE_STATUS" "failing nested runner"
  _selftest_file_contains "$CAPTURE_STDOUT" \
    "not ok 1 - harness::__failing_probe" "failing TAP output"
  _selftest_file_contains "$CAPTURE_STDOUT" \
    "# 0 passed, 1 failed" "failing TAP summary"
  _selftest_count_suites "$temp_base"
  assert_status 0 "$SELFTEST_SUITE_COUNT" "non-debug failed suite cleanup count"
}

_selftest_runner_debug_cleanup() {
  local success_prefix="$CASE_CAPTURE_DIR/nested-debug-success"
  local failure_prefix="$CASE_CAPTURE_DIR/nested-debug-failure"
  local temp_base="$CASE_PROJECTS/nested-debug-temp"
  local preserved_case

  _selftest_nested_runner "$success_prefix" "$temp_base" \
    --debug case harness::__passing_probe
  assert_status 0 "$CAPTURE_STATUS" "debug passing nested runner"
  _selftest_count_suites "$temp_base"
  assert_status 0 "$SELFTEST_SUITE_COUNT" "debug success cleanup count"

  _selftest_nested_runner "$failure_prefix" "$temp_base" \
    --debug case harness::__failing_probe
  assert_status 1 "$CAPTURE_STATUS" "debug failing nested runner"
  _selftest_count_suites "$temp_base"
  assert_status 1 "$SELFTEST_SUITE_COUNT" "debug failure preserved suite count"
  if ((SELFTEST_SUITE_COUNT == 1)); then
    preserved_case="${SELFTEST_SUITES[0]}"
    _selftest_file_contains "$failure_prefix.stdout" \
      "# suite artifacts: $preserved_case" "debug artifact report"
    if [[ ! -f "$preserved_case/.dstack-characterization-suite" ]]; then
      fail "debug-preserved suite is missing its sentinel"
    fi
  fi
}

_selftest_canary_failure_aborts_suite() {
  local prefix="$CASE_CAPTURE_DIR/nested-canary-failure"
  local temp_base="$CASE_PROJECTS/nested-canary-failure-temp"

  mkdir -p "$temp_base"
  capture_command "$prefix" \
    env \
      DSTACK_TEST_INTERNAL_PROBES=1 \
      DSTACK_TEST_INTERNAL_BROKEN_DOCKER_STUB=1 \
      DSTACK_TEST_TMPDIR="$temp_base" \
      "$CHARACTERIZATION_BASH" "$CHARACTERIZATION_RUNNER" \
      case harness::__passing_probe

  assert_status 1 "$CAPTURE_STATUS" "runner with unavailable fake Docker"
  _selftest_file_contains "$CAPTURE_STDOUT" \
    "Bail out! Fake Docker could not be proven active; suite stopped." \
    "fake-Docker safety bailout"
  _selftest_count_suites "$temp_base"
  assert_status 0 "$SELFTEST_SUITE_COUNT" "aborted suite cleanup count"
}

_selftest_cleanup_refuses_unmarked_path() {
  local unmarked="$CASE_PROJECTS/unmarked"
  local prefix="$CASE_CAPTURE_DIR/refused-cleanup"

  mkdir -p "$unmarked"
  capture_command "$prefix" cleanup_case_root "$unmarked" "$CASE_ROOT"
  assert_status 1 "$CAPTURE_STATUS" "unsafe cleanup refusal"
  if [[ ! -d "$unmarked" ]]; then
    fail "cleanup removed an unmarked path"
  fi
}

_selftest_fake_docker_resolves_first() {
  local expected
  local resolved

  require_fake_docker
  expected="$(_physical_file_path "$FAKE_DOCKER_PATH")"
  resolved="$(_physical_file_path "$(type -P docker)")"
  if [[ "$resolved" != "$expected" ]]; then
    fail "docker resolution differs: expected $expected, got $resolved"
  fi
}

_selftest_fake_docker_canary() {
  invoke_fake_docker_canary
  assert_call_count 1
  assert_argv 1 __dstack_characterization_canary__ "$DSTACK_TEST_DOCKER_GUARD"
  assert_file_empty "$CASE_CAPTURE_DIR/canary.stdout"
  assert_file_empty "$CASE_CAPTURE_DIR/canary.stderr"
}

_selftest_fake_docker_missing_guard() {
  local prefix="$CASE_CAPTURE_DIR/missing-guard"

  reset_fake_docker_records
  capture_command "$prefix" _selftest_missing_guard
  assert_status 125 "$CAPTURE_STATUS" "missing fake-Docker guard"
  _selftest_file_contains "$CAPTURE_STDERR" "guard is not set" "missing-guard diagnostic"
  assert_call_count 0
}

_selftest_fake_docker_wrong_guard() {
  local prefix="$CASE_CAPTURE_DIR/wrong-guard"

  reset_fake_docker_records
  capture_command "$prefix" _selftest_wrong_guard
  assert_status 125 "$CAPTURE_STATUS" "wrong fake-Docker guard"
  _selftest_file_contains "$CAPTURE_STDERR" "guard does not match" "wrong-guard diagnostic"
  assert_call_count 0
}

_selftest_fake_docker_fails_closed() {
  local prefix="$CASE_CAPTURE_DIR/fail-closed"

  mv "$FAKE_DOCKER_PATH" "$FAKE_DOCKER_PATH.disabled"
  hash -r
  capture_command "$prefix" docker should-never-reach-real-docker
  assert_status 125 "$CAPTURE_STATUS" "fail-closed Docker blocker"
  _selftest_file_contains "$CAPTURE_STDERR" \
    "primary fake is unavailable; refusing fallback" \
    "fail-closed Docker diagnostic"
  assert_call_count 0
  mv "$FAKE_DOCKER_PATH.disabled" "$FAKE_DOCKER_PATH"
  hash -r
  require_fake_docker
}

_selftest_fake_docker_call_order() {
  reset_fake_docker_records
  docker first-call >/dev/null
  docker second-call "two words" >/dev/null
  docker third-call >/dev/null

  assert_call_count 3
  assert_argv 1 first-call
  assert_argv 2 second-call "two words"
  assert_argv 3 third-call
  assert_text_file "$DSTACK_TEST_DOCKER_CONTROL/calls/000001/sequence" $'1\n'
  assert_text_file "$DSTACK_TEST_DOCKER_CONTROL/calls/000002/sequence" $'2\n'
  assert_text_file "$DSTACK_TEST_DOCKER_CONTROL/calls/000003/sequence" $'3\n'
}

_selftest_fake_docker_exact_argv() {
  reset_fake_docker_records
  docker compose "" "space value" -leading '*' '$HOME' ';' '|' '&&' $'line\nbreak' >/dev/null

  assert_call_count 1
  assert_argv 1 compose "" "space value" -leading '*' '$HOME' ';' '|' '&&' $'line\nbreak'
  if [[ -e "$DSTACK_TEST_DOCKER_CONTROL/calls/000001/stdin" ]]; then
    fail "fake Docker captured stdin without an explicit opt-in"
  fi
}

_selftest_fake_docker_sequential_responses() {
  local first_out="$CASE_CAPTURE_DIR/first-response.stdout"
  local first_err="$CASE_CAPTURE_DIR/first-response.stderr"
  local second_out="$CASE_CAPTURE_DIR/second-response.stdout"
  local first_capture="$CASE_CAPTURE_DIR/first-call"
  local second_capture="$CASE_CAPTURE_DIR/second-call"

  printf 'first output\n\n' >"$first_out"
  printf 'first error\n' >"$first_err"
  printf 'second output' >"$second_out"
  configure_fake_docker_response 1 17 "$first_out" "$first_err"
  configure_fake_docker_response 2 0 "$second_out"

  capture_command "$first_capture" docker first
  assert_status 17 "$CAPTURE_STATUS" "first fake-Docker response"
  assert_bytes_equal "$first_out" "$CAPTURE_STDOUT"
  assert_bytes_equal "$first_err" "$CAPTURE_STDERR"

  capture_command "$second_capture" docker second
  assert_status 0 "$CAPTURE_STATUS" "second fake-Docker response"
  assert_bytes_equal "$second_out" "$CAPTURE_STDOUT"
  assert_file_empty "$CAPTURE_STDERR"
  assert_call_count 2
  assert_argv 1 first
  assert_argv 2 second
}

_selftest_fake_docker_records_context() {
  local expected_compose="$CASE_CAPTURE_DIR/compose-files.expected.nul"
  local physical_cwd

  fixture_create_compose_project "project with spaces" compose.yml "services: {}"
  fixture_set_cwd "$FIXTURE_PROJECT"
  physical_cwd="$(pwd -P)"

  docker compose -f "$FIXTURE_PROJECT/compose.yml" config >/dev/null
  assert_recorded_cwd 1 "$physical_cwd"
  assert_recorded_env 1 HOME set "$CASE_HOME"
  assert_recorded_env 1 DSTACK unset ""
  write_nul_arguments "$expected_compose" "$FIXTURE_PROJECT/compose.yml"
  assert_bytes_equal "$expected_compose" \
    "$DSTACK_TEST_DOCKER_CONTROL/calls/000001/compose-files.nul" \
    "recorded Compose files"
}

_selftest_fake_docker_stdin_capture() {
  local input="$CASE_CAPTURE_DIR/docker.stdin.expected"

  printf 'stdin line 1\nstdin line 2\n' >"$input"
  DSTACK_FAKE_CAPTURE_STDIN=1 docker consume-stdin <"$input" >/dev/null
  assert_call_count 1
  assert_bytes_equal "$input" \
    "$DSTACK_TEST_DOCKER_CONTROL/calls/000001/stdin" \
    "explicit fake-Docker stdin capture"
}

_selftest_environment_isolated() {
  assert_var_set HOME "$CASE_HOME"
  assert_var_set USER "dstack-test-user"
  assert_var_set DSTACK_BASES "$CASE_BASES"
  assert_var_unset DSTACK
  assert_var_unset DSTACK_COMPOSE_FILE
  assert_var_unset DSTACK_COMPOSE_FILES
  assert_var_set DOCKER_CONFIG "$CASE_DOCKER_CONFIG"
  assert_var_set DOCKER_HOST "unix://$CASE_ROOT/nonexistent-docker.sock"
  assert_var_set LC_ALL C
  assert_var_set TERM dumb

  for required_directory in home bases projects fake-bin docker-control capture; do
    if [[ ! -d "$CASE_ROOT/$required_directory" ]]; then
      fail "case sandbox is missing $required_directory"
    fi
  done
}

_selftest_fixture_helpers() {
  local content=$'services:\n  app:\n    image: example\n'

  fixture_create_project empty-project
  if [[ ! -d "$FIXTURE_PROJECT" ]]; then
    fail "empty project fixture was not created"
  fi

  fixture_create_project multi-project compose.yml compose.yaml docker-compose.yml
  for compose_file in compose.yml compose.yaml docker-compose.yml; do
    if [[ ! -f "$FIXTURE_PROJECT/$compose_file" ]]; then
      fail "multiple-file fixture is missing $compose_file"
    fi
  done

  fixture_create_compose_project "project with spaces" custom-compose.yaml "$content"
  assert_text_file "$FIXTURE_PROJECT/custom-compose.yaml" "$content" "Compose fixture contents"

  fixture_create_nested_stack "media/jellyfin" docker-compose.yml "$content"
  if [[ ! -f "$FIXTURE_STACK/docker-compose.yml" ]]; then
    fail "nested stack fixture was not created"
  fi

  fixture_create_discovery_base base-a
  if [[ ! -d "$FIXTURE_BASE" ]]; then
    fail "discovery base fixture was not created"
  fi

  fixture_create_discovered_stack base-b "nested/discovered" compose.yaml "$content"
  if [[ ! -f "$FIXTURE_STACK/compose.yaml" ]]; then
    fail "nested discovered-stack fixture was not created"
  fi

  fixture_create_registry
  fixture_add_registry_entry jellyfin "$FIXTURE_STACK"
  assert_text_file "$CASE_REGISTRY" "jellyfin=$FIXTURE_STACK"$'\n' "registry fixture"

  fixture_set_cwd "$FIXTURE_STACK"
  if [[ "$(pwd -P)" != "$(cd "$FIXTURE_STACK" && pwd -P)" ]]; then
    fail "fixture_set_cwd did not enter the requested fixture"
  fi
}

_selftest_child_bash_execution() {
  local child_script="$CASE_PROJECTS/dummy-child.bash"
  local prefix="$CASE_CAPTURE_DIR/child"

  DSTACK="$CASE_PROJECTS/controlled-active-stack"
  DSTACK_TEST_PARENT_LEAK="must not enter child"
  export DSTACK DSTACK_TEST_PARENT_LEAK

  cat >"$child_script" <<'EOF'
characterization_child_main() {
  case "$-" in
    *u*) ;;
    *) return 98 ;;
  esac

  printf 'child stdout\n\n'
  printf 'child stderr\n' >&2
  CHILD_VALUE="$1"
  CHILD_EMPTY="$2"
  CHILD_EXPORTED="exported value"
  export CHILD_EXPORTED
  unset CHILD_MISSING
  return 23
}
EOF

  set_child_state_vars \
    CHILD_VALUE CHILD_EMPTY CHILD_EXPORTED CHILD_MISSING \
    HOME DSTACK DSTACK_TEST_PARENT_LEAK
  run_child_bash "$prefix" -u -- "$child_script" "value with spaces" ""

  assert_status 23 "$CAPTURE_STATUS" "child Bash status"
  assert_text_file "$CAPTURE_STDOUT" $'child stdout\n\n' "child Bash stdout"
  assert_text_file "$CAPTURE_STDERR" $'child stderr\n' "child Bash stderr"
  assert_child_state CHILD_VALUE set "value with spaces" not-exported
  assert_child_state CHILD_EMPTY set "" not-exported
  assert_child_state CHILD_EXPORTED set "exported value" exported
  assert_child_state CHILD_MISSING unset "" not-exported
  assert_child_state HOME set "$CASE_HOME" exported
  assert_child_state DSTACK set "$DSTACK" exported
  assert_child_state DSTACK_TEST_PARENT_LEAK unset "" not-exported
}

register_case harness harness::captures_zero_status _selftest_capture_zero_status
register_case harness harness::captures_nonzero_status _selftest_capture_nonzero_status
register_case harness harness::captures_stdout_and_stderr_separately _selftest_capture_streams
register_case harness harness::captures_empty_streams _selftest_capture_empty_streams
register_case harness harness::preserves_trailing_newlines _selftest_trailing_newlines
register_case harness harness::preserves_empty_argument _selftest_preserve_empty_argument
register_case harness harness::preserves_space_argument _selftest_preserve_space_argument
register_case harness harness::preserves_shell_metacharacters _selftest_preserve_metacharacters
register_case harness harness::assertion_helpers_accept_expected_values _selftest_assertion_helpers
register_case harness harness::assertion_failures_are_diagnostic _selftest_assertion_failure_diagnostic
register_case harness harness::reports_successful_case _selftest_runner_success_reporting
register_case harness harness::reports_failed_case_and_nonzero_suite _selftest_runner_failure_reporting
register_case harness harness::preserves_only_failed_debug_artifacts _selftest_runner_debug_cleanup
register_case harness harness::canary_failure_aborts_suite _selftest_canary_failure_aborts_suite
register_case harness harness::cleanup_refuses_unmarked_paths _selftest_cleanup_refuses_unmarked_path
register_case harness harness::fake_docker_resolves_first _selftest_fake_docker_resolves_first
register_case harness harness::fake_docker_records_canary _selftest_fake_docker_canary
register_case harness harness::fake_docker_rejects_missing_guard _selftest_fake_docker_missing_guard
register_case harness harness::fake_docker_rejects_wrong_guard _selftest_fake_docker_wrong_guard
register_case harness harness::fake_docker_fails_closed_if_primary_missing _selftest_fake_docker_fails_closed
register_case harness harness::fake_docker_records_multiple_calls_in_order _selftest_fake_docker_call_order
register_case harness harness::fake_docker_preserves_exact_argv _selftest_fake_docker_exact_argv
register_case harness harness::fake_docker_applies_sequential_responses _selftest_fake_docker_sequential_responses
register_case harness harness::fake_docker_records_context _selftest_fake_docker_records_context
register_case harness harness::fake_docker_captures_stdin_only_when_enabled _selftest_fake_docker_stdin_capture
register_case harness harness::environment_is_isolated _selftest_environment_isolated
register_case harness harness::fixture_helpers_create_isolated_projects _selftest_fixture_helpers
register_case harness harness::child_bash_captures_status_streams_and_state _selftest_child_bash_execution

if [[ "${DSTACK_TEST_INTERNAL_PROBES:-0}" == "1" ]]; then
  _selftest_internal_passing_probe() {
    return 0
  }

  _selftest_internal_failing_probe() {
    fail "intentional runner failure probe"
  }

  register_case internal harness::__passing_probe _selftest_internal_passing_probe
  register_case internal harness::__failing_probe _selftest_internal_failing_probe
fi
