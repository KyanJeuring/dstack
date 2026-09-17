#!/usr/bin/env bash

_status_configure_response() {
  local call="$1"
  local status="$2"
  local stdout_text="${3:-}"
  local stderr_text="${4:-}"
  local stdout_file="$CASE_CAPTURE_DIR/response-${call}.stdout"
  local stderr_file="$CASE_CAPTURE_DIR/response-${call}.stderr"

  printf '%s' "$stdout_text" >"$stdout_file"
  printf '%s' "$stderr_text" >"$stderr_file"
  configure_fake_docker_response "$call" "$status" "$stdout_file" "$stderr_file"
}

_status_write_invocation_probe() {
  STATUS_PROBE="$CASE_PROJECTS/status-invoke.bash"
  cat >"$STATUS_PROBE" <<'EOF'
characterization_child_main() {
  local dstack_path="$1"
  local working_directory="$2"
  local function_name="$3"
  shift 3
  cd "$working_directory" || return 125
  source "$dstack_path"
  "$function_name" "$@"
}
EOF
}

_status_run_with_options() {
  local name="$1"
  local function_name="$2"
  shift 2
  local -a bash_options=()

  while [[ $# -gt 0 && "$1" != "--" ]]; do
    bash_options+=("$1")
    shift
  done
  shift
  _status_write_invocation_probe
  set_child_state_vars
  run_child_bash "$CASE_CAPTURE_DIR/$name" "${bash_options[@]}" -- \
    "$STATUS_PROBE" "$CHARACTERIZATION_REPO_ROOT/dstack.sh" "$(pwd -P)" \
    "$function_name" "$@"
}

_status_install_cat_utility() {
  local name="$1"

  cat >"$CASE_FAKE_BIN/$name" <<'EOF'
#!/usr/bin/env bash
cat
EOF
  chmod +x "$CASE_FAKE_BIN/$name"
  hash -r
}

_status_compose_success_propagates_streams() {
  _forward_prepare_local
  _status_configure_response 1 0 $'docker stdout\n' $'docker stderr\n'
  _compose_run compose-success dcompose ps

  assert_status 0 "$CAPTURE_STATUS"
  assert_text_file "$CAPTURE_STDOUT" $'docker stdout\n'
  assert_text_file "$CAPTURE_STDERR" $'docker stderr\n'
  assert_call_count 1
}

_status_compose_failure_is_masked_but_streams_propagate() {
  _forward_prepare_local
  _status_configure_response 1 37 $'failed stdout\n' $'failed stderr\n'
  _compose_run compose-failure dcompose ps

  assert_status 0 "$CAPTURE_STATUS"
  assert_text_file "$CAPTURE_STDOUT" $'failed stdout\n'
  assert_text_file "$CAPTURE_STDERR" $'failed stderr\n'
  assert_call_count 1
}

_status_direct_helper_failure_propagates() {
  _status_configure_response 1 23 $'port output\n' $'port error\n'
  _compose_run direct-failure dport

  assert_status 23 "$CAPTURE_STATUS"
  assert_text_file "$CAPTURE_STDOUT" $'port output\n'
  assert_text_file "$CAPTURE_STDERR" $'port error\n'
  assert_call_count 1
}

_status_masked_direct_helper_failure_returns_zero() {
  _status_configure_response 1 19 "" $'inspect failed\n'
  _compose_run masked-direct dip container-one

  assert_status 0 "$CAPTURE_STATUS"
  assert_file_empty "$CAPTURE_STDOUT"
  assert_text_file "$CAPTURE_STDERR" $'inspect failed\n'
  assert_call_count 1
}

_status_validation_errors_use_stdout_and_usually_zero() {
  _compose_run missing-dexec dexec
  assert_status 0 "$CAPTURE_STATUS"
  assert_text_file "$CAPTURE_STDOUT" $'[ERROR] Usage: dexec [stack] <service>\n'
  assert_file_empty "$CAPTURE_STDERR"
  assert_call_count 0
}

_status_drun_missing_service_returns_one() {
  _compose_run missing-drun drun
  assert_status 1 "$CAPTURE_STATUS"
  assert_text_file "$CAPTURE_STDOUT" $'[ERROR] Usage: drun [stack] <service> [command...]\n'
  assert_file_empty "$CAPTURE_STDERR"
  assert_call_count 0
}

_status_unresolved_explicit_stack_returns_zero() {
  _compose_run missing-update dupdate absent
  assert_status 0 "$CAPTURE_STATUS"
  assert_text_file "$CAPTURE_STDOUT" $'[ERROR] Stack not found: absent\n'
  assert_file_empty "$CAPTURE_STDERR"
  assert_call_count 0
}

_status_missing_local_compose_returns_zero() {
  _compose_run missing-local dstart
  assert_status 0 "$CAPTURE_STATUS"
  assert_text_file "$CAPTURE_STDOUT" \
    $'[ERROR] No compose file found in the current directory or active stack context.\n[ERROR] Use \'dstack\' to set an active stack or run from a directory with a compose file.\n'
  assert_file_empty "$CAPTURE_STDERR"
  assert_call_count 0
}

_status_eof_cancels_confirmation() {
  _forward_prepare_local
  set_child_state_vars
  run_dstack_function "$CASE_CAPTURE_DIR/cancel" ddownv </dev/null

  assert_status 0 "$CAPTURE_STATUS"
  assert_text_file "$CAPTURE_STDOUT" $'[WARN] This will remove containers + volumes\n'
  assert_file_empty "$CAPTURE_STDERR"
  assert_call_count 0
}

_status_non_tty_confirmation_input_matrix() {
  local input_file="$CASE_CAPTURE_DIR/confirmation.input"
  local label
  local input
  local expected_calls
  local -a labels=(lowercase uppercase lowercase-no uppercase-no word blank eof)
  local -a inputs=($'y\n' $'Y\n' $'n\n' $'N\n' $'yes\n' $'\n' '')
  local -a calls=(1 1 0 0 0 0 0)
  local index

  _forward_prepare_local
  for index in "${!labels[@]}"; do
    label="${labels[$index]}"
    input="${inputs[$index]}"
    expected_calls="${calls[$index]}"
    reset_fake_docker_records
    printf '%s' "$input" >"$input_file"
    set_child_state_vars
    run_dstack_function "$CASE_CAPTURE_DIR/confirmation-$label" ddownv <"$input_file"

    assert_status 0 "$CAPTURE_STATUS" "$label confirmation status"
    assert_text_file "$CAPTURE_STDOUT" \
      $'[WARN] This will remove containers + volumes\n' \
      "$label confirmation stdout"
    assert_file_empty "$CAPTURE_STDERR" "$label confirmation stderr"
    assert_call_count "$expected_calls"
    if ((expected_calls)); then
      assert_argv 1 compose -f "$FORWARD_COMPOSE" down -v
    fi
  done
}

_status_rebuild_suppresses_docker_stderr() {
  _forward_prepare_local
  _status_configure_response 1 31 $'build stdout\n' $'build stderr\n'
  _status_configure_response 2 32 $'up stdout\n' $'up stderr\n'
  _compose_run rebuild-stderr drebuild web

  assert_status 0 "$CAPTURE_STATUS"
  assert_text_file "$CAPTURE_STDOUT" $'build stdout\nup stdout\n'
  assert_file_empty "$CAPTURE_STDERR"
  assert_call_count 2
}

_status_pipeline_depends_on_caller_pipefail() {
  _status_install_cat_utility column
  _status_configure_response 1 17 $'service row\n' $'docker pipeline error\n'
  _compose_run pipefail-off dps
  assert_status 0 "$CAPTURE_STATUS"
  assert_text_file "$CAPTURE_STDOUT" $'SERVICE row\n'
  assert_text_file "$CAPTURE_STDERR" $'docker pipeline error\n'

  reset_fake_docker_records
  _status_configure_response 1 17 $'service row\n' $'docker pipeline error\n'
  _status_run_with_options pipefail-on dps -o pipefail --
  assert_status 17 "$CAPTURE_STATUS"
  assert_text_file "$CAPTURE_STDOUT" $'SERVICE row\n'
  assert_text_file "$CAPTURE_STDERR" $'docker pipeline error\n'
  assert_call_count 1
}

_status_errexit_stops_after_unmasked_failure() {
  local probe="$CASE_PROJECTS/errexit-scenario.bash"
  local marker="$CASE_CAPTURE_DIR/after-direct"

  _status_configure_response 1 29
  cat >"$probe" <<'EOF'
characterization_child_main() {
  local dstack_path="$1"
  local marker="$2"
  source "$dstack_path"
  dport
  : >"$marker"
}
EOF
  set_child_state_vars
  run_child_bash "$CASE_CAPTURE_DIR/errexit" -e -- \
    "$probe" "$CHARACTERIZATION_REPO_ROOT/dstack.sh" "$marker"

  assert_status 29 "$CAPTURE_STATUS"
  if [[ -e "$marker" ]]; then
    fail "errexit scenario continued after direct Docker failure"
  fi
  assert_call_count 1
}

_status_errexit_does_not_expose_masked_compose_failure() {
  _forward_prepare_local
  _status_configure_response 1 29
  _status_run_with_options masked-errexit dcompose -e -- ps
  assert_status 0 "$CAPTURE_STATUS"
  assert_call_count 1
}

_status_nounset_changes_missing_argument_path() {
  _status_run_with_options nounset dpsg -u --
  assert_status 1 "$CAPTURE_STATUS"
  assert_file_empty "$CAPTURE_STDOUT"
  if ! grep -Fq 'unbound variable' "$CAPTURE_STDERR"; then
    fail "nounset case did not report an unbound positional parameter"
  fi
  assert_call_count 0
}

_status_pager_receives_docker_output() {
  local pager_input="$DSTACK_TEST_DOCKER_CONTROL/pager.stdin"

  _forward_prepare_local
  cat >"$CASE_FAKE_BIN/less" <<'EOF'
#!/usr/bin/env bash
cat >"$DSTACK_TEST_DOCKER_CONTROL/pager.stdin"
printf 'pager result\n'
EOF
  chmod +x "$CASE_FAKE_BIN/less"
  hash -r
  _status_configure_response 1 0 $'docker page bytes\n' $'docker page stderr\n'
  _compose_run pager dllogs

  assert_status 0 "$CAPTURE_STATUS"
  assert_text_file "$CAPTURE_STDOUT" $'pager result\n'
  assert_text_file "$CAPTURE_STDERR" $'docker page stderr\n'
  assert_text_file "$pager_input" $'docker page bytes\n' "pager stdin"
  assert_call_count 1
}

register_case status status::compose_success_propagates_stdout_and_stderr _status_compose_success_propagates_streams
register_case status status::compose_failure_is_masked_but_streams_propagate _status_compose_failure_is_masked_but_streams_propagate
register_case status status::direct_helper_failure_status_propagates _status_direct_helper_failure_propagates
register_case status status::masked_direct_helper_failure_returns_zero _status_masked_direct_helper_failure_returns_zero
register_case status status::validation_error_uses_stdout_and_returns_zero _status_validation_errors_use_stdout_and_usually_zero
register_case status status::drun_missing_service_returns_one _status_drun_missing_service_returns_one
register_case status status::unresolved_explicit_stack_returns_zero _status_unresolved_explicit_stack_returns_zero
register_case status status::missing_local_compose_returns_zero _status_missing_local_compose_returns_zero
register_case status status::eof_cancels_confirmation_without_docker _status_eof_cancels_confirmation
register_case status status::non_tty_confirmation_input_matrix _status_non_tty_confirmation_input_matrix
register_case status status::drebuild_suppresses_docker_stderr _status_rebuild_suppresses_docker_stderr
register_case status status::pipeline_status_depends_on_caller_pipefail _status_pipeline_depends_on_caller_pipefail
register_case status status::caller_errexit_stops_after_unmasked_failure _status_errexit_stops_after_unmasked_failure
register_case status status::caller_errexit_does_not_expose_masked_compose_failure _status_errexit_does_not_expose_masked_compose_failure
register_case status status::caller_nounset_changes_missing_argument_path _status_nounset_changes_missing_argument_path
register_case status status::pager_receives_docker_stdout _status_pager_receives_docker_output
