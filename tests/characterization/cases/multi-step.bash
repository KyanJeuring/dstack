#!/usr/bin/env bash

_multi_prepare_local() {
  MULTI_COMPOSE="$CASE_CURRENT_DIR/compose.yml"
  : >"$MULTI_COMPOSE"
}

_multi_contains() {
  local file="$1"
  local text="$2"
  local label="$3"

  if ! grep -Fq "$text" "$file"; then
    fail "$label: expected $(printf '%q' "$text") in $file"
  fi
}

_multi_run_command() {
  local prefix="$1"
  local mode="$2"
  local function_name="$3"
  shift 3

  set_child_state_vars
  if [[ "$mode" == confirmed ]]; then
    run_dstack_function_confirmed "$CASE_CAPTURE_DIR/$prefix" "$function_name" "$@"
  else
    run_dstack_function "$CASE_CAPTURE_DIR/$prefix" "$function_name" "$@"
  fi
}

_multi_exercise_failure_matrix() {
  local label="$1"
  local mode="$2"
  local function_name="$3"
  local stderr_mode="$4"
  local argv_callback="$5"
  shift 5

  _status_configure_response 1 0 $'first success\n'
  _status_configure_response 2 0 $'second success\n'
  _multi_run_command "$label-all" "$mode" "$function_name" "$@"
  assert_status 0 "$CAPTURE_STATUS" "$label all-success status"
  assert_call_count 2
  "$argv_callback"
  _multi_contains "$CAPTURE_STDOUT" "first success" "$label first success output"
  _multi_contains "$CAPTURE_STDOUT" "second success" "$label second success output"
  assert_file_empty "$CAPTURE_STDERR" "$label all-success stderr"

  reset_fake_docker_records
  _status_configure_response 1 41 $'first failed stdout\n' $'first failed stderr\n'
  _status_configure_response 2 0 $'second after failure\n'
  _multi_run_command "$label-first-fails" "$mode" "$function_name" "$@"
  assert_status 0 "$CAPTURE_STATUS" "$label first-failure status"
  assert_call_count 2
  "$argv_callback"
  _multi_contains "$CAPTURE_STDOUT" "second after failure" "$label continued after first failure"
  if [[ "$stderr_mode" == suppressed ]]; then
    assert_file_empty "$CAPTURE_STDERR" "$label suppressed first stderr"
  else
    assert_text_file "$CAPTURE_STDERR" $'first failed stderr\n' "$label first stderr"
  fi

  reset_fake_docker_records
  _status_configure_response 1 0 $'first before failure\n'
  _status_configure_response 2 42 $'second failed stdout\n' $'second failed stderr\n'
  _multi_run_command "$label-second-fails" "$mode" "$function_name" "$@"
  assert_status 0 "$CAPTURE_STATUS" "$label second-failure status"
  assert_call_count 2
  "$argv_callback"
  _multi_contains "$CAPTURE_STDOUT" "first before failure" "$label first output before second failure"
  if [[ "$stderr_mode" == suppressed ]]; then
    assert_file_empty "$CAPTURE_STDERR" "$label suppressed second stderr"
  else
    assert_text_file "$CAPTURE_STDERR" $'second failed stderr\n' "$label second stderr"
  fi
}

_multi_argv_recompose() {
  assert_argv 1 compose -f "$MULTI_COMPOSE" down -v
  assert_argv 2 compose -f "$MULTI_COMPOSE" up -d
}

_multi_argv_reboot() {
  assert_argv 1 compose -f "$MULTI_COMPOSE" down
  assert_argv 2 compose -f "$MULTI_COMPOSE" up -d
}

_multi_argv_update() {
  assert_argv 1 compose -f "$MULTI_COMPOSE" pull
  assert_argv 2 compose -f "$MULTI_COMPOSE" up -d
}

_multi_argv_rebuild() {
  assert_argv 1 compose -f "$MULTI_COMPOSE" build web
  assert_argv 2 compose -f "$MULTI_COMPOSE" up -d web
}

_multi_argv_nocache() {
  assert_argv 1 compose -f "$MULTI_COMPOSE" build --no-cache
  assert_argv 2 compose -f "$MULTI_COMPOSE" up -d
}

_multi_argv_purge() {
  assert_argv 1 compose -f "$MULTI_COMPOSE" down -v
  assert_argv 2 system prune -f
}

_multi_recompose_failure_matrix() {
  _multi_prepare_local
  _multi_exercise_failure_matrix recompose normal drecompose visible _multi_argv_recompose
}

_multi_reboot_failure_matrix() {
  _multi_prepare_local
  _multi_exercise_failure_matrix reboot normal drebootstack visible _multi_argv_reboot
}

_multi_update_failure_matrix() {
  _multi_prepare_local
  _multi_exercise_failure_matrix update normal dupdate visible _multi_argv_update
}

_multi_rebuild_failure_matrix() {
  _multi_prepare_local
  _multi_exercise_failure_matrix rebuild normal drebuild suppressed _multi_argv_rebuild web
}

_multi_nocache_failure_matrix() {
  _multi_prepare_local
  _multi_exercise_failure_matrix nocache normal drebuildnocache suppressed _multi_argv_nocache
}

_multi_purge_cancel_and_failure_matrix() {
  _multi_prepare_local
  set_child_state_vars
  run_dstack_function "$CASE_CAPTURE_DIR/purge-cancel" dstackpurge </dev/null
  assert_status 0 "$CAPTURE_STATUS"
  assert_call_count 0

  _multi_exercise_failure_matrix purge confirmed dstackpurge visible _multi_argv_purge
}

register_case multi multi::drecompose_runs_second_call_after_each_failure _multi_recompose_failure_matrix
register_case multi multi::drebootstack_runs_second_call_after_each_failure _multi_reboot_failure_matrix
register_case multi multi::dupdate_runs_second_call_after_each_failure _multi_update_failure_matrix
register_case multi multi::drebuild_masks_each_failure_and_suppresses_stderr _multi_rebuild_failure_matrix
register_case multi multi::drebuildnocache_masks_each_failure_and_suppresses_stderr _multi_nocache_failure_matrix
register_case multi multi::dstackpurge_cancels_or_runs_both_calls_after_failures _multi_purge_cancel_and_failure_matrix
