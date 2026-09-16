#!/usr/bin/env bash

_precedence_prepare_custom_candidates() {
  PRECEDENCE_FIRST="$CASE_CURRENT_DIR/first.yml"
  PRECEDENCE_SECOND="$CASE_CURRENT_DIR/second.yml"
  : >"$PRECEDENCE_FIRST"
  : >"$PRECEDENCE_SECOND"
  DSTACK_COMPOSE_FILES="$PRECEDENCE_FIRST $PRECEDENCE_SECOND"
  export DSTACK_COMPOSE_FILES
}

_precedence_leading_file_flag_selects_before_command() {
  _precedence_prepare_custom_candidates
  _compose_run short-file dcompose -f second.yml logs
  assert_status 0 "$CAPTURE_STATUS"
  assert_file_empty "$CAPTURE_STDERR"
  assert_call_count 1
  assert_argv 1 compose -f "$PRECEDENCE_SECOND" logs
}

_precedence_long_file_flags_select_before_command() {
  _precedence_prepare_custom_candidates
  _compose_run file dcompose --file first.yml logs
  _compose_run compose-file dcompose --compose-file second.yml ps
  assert_call_count 2
  assert_argv 1 compose -f "$PRECEDENCE_FIRST" logs
  assert_argv 2 compose -f "$PRECEDENCE_SECOND" ps
}

_precedence_repeated_leading_flags_select_multiple() {
  _precedence_prepare_custom_candidates
  _compose_run repeated dcompose -f first.yml --file second.yml config
  assert_status 0 "$CAPTURE_STATUS"
  assert_file_empty "$CAPTURE_STDERR"
  assert_call_count 1
  assert_argv 1 compose -f "$PRECEDENCE_FIRST" -f "$PRECEDENCE_SECOND" config
}

_precedence_explicit_flag_beats_environment_selector() {
  _precedence_prepare_custom_candidates
  DSTACK_COMPOSE_FILE=first.yml
  export DSTACK_COMPOSE_FILE
  _compose_run explicit-env dcompose -f second.yml logs
  assert_status 0 "$CAPTURE_STATUS"
  assert_call_count 1
  assert_argv 1 compose -f "$PRECEDENCE_SECOND" logs
}

_precedence_environment_selector_beats_automatic() {
  _precedence_prepare_custom_candidates
  DSTACK_COMPOSE_FILE=second.yml
  export DSTACK_COMPOSE_FILE
  _compose_run env-selector dcompose logs
  assert_status 0 "$CAPTURE_STATUS"
  assert_call_count 1
  assert_argv 1 compose -f "$PRECEDENCE_SECOND" logs
}

_precedence_single_candidate_is_automatic() {
  : >"$CASE_CURRENT_DIR/compose.yml"
  _compose_run automatic dcompose logs
  assert_status 0 "$CAPTURE_STATUS"
  assert_call_count 1
  assert_argv 1 compose -f "$CASE_CURRENT_DIR/compose.yml" logs
}

_precedence_file_flag_after_command_is_forwarded() {
  : >"$CASE_CURRENT_DIR/compose.yml"
  _compose_run trailing-file dcompose logs -f
  assert_status 0 "$CAPTURE_STATUS"
  assert_call_count 1
  assert_argv 1 compose -f "$CASE_CURRENT_DIR/compose.yml" logs -f
}

_precedence_non_tty_multiple_error_becomes_file_flag() {
  : >"$CASE_CURRENT_DIR/docker-compose.yml"
  : >"$CASE_CURRENT_DIR/compose.yml"
  _compose_run captured-error dstart
  assert_status 0 "$CAPTURE_STATUS"
  assert_file_empty "$CAPTURE_STDERR"
  assert_call_count 1
  assert_argv 1 compose -f \
    '[ERROR] Multiple compose files found. Choose one with -f, --file, --compose-file, or DSTACK_COMPOSE_FILE.' \
    start
}

_precedence_invalid_selector_error_becomes_file_flag() {
  : >"$CASE_CURRENT_DIR/compose.yml"
  DSTACK_COMPOSE_FILE=missing.yml
  export DSTACK_COMPOSE_FILE
  _compose_run invalid-selector dstart
  assert_status 0 "$CAPTURE_STATUS"
  assert_file_empty "$CAPTURE_STDERR"
  assert_call_count 1
  assert_argv 1 compose -f '[ERROR] Compose file not found: missing.yml' start
}

_precedence_empty_flag_resolver_succeeds() {
  _compose_run empty-flags _dstack_resolve_compose_flags "$CASE_CURRENT_DIR" ""
  _compose_expect 0 ""
}

register_case compose compose::leading_short_file_flag_selects_before_command _precedence_leading_file_flag_selects_before_command
register_case compose compose::leading_long_file_flags_select_before_command _precedence_long_file_flags_select_before_command
register_case compose compose::repeated_leading_file_flags_select_multiple _precedence_repeated_leading_flags_select_multiple
register_case compose compose::explicit_file_flag_beats_environment_selector _precedence_explicit_flag_beats_environment_selector
register_case compose compose::environment_selector_beats_automatic_selection _precedence_environment_selector_beats_automatic
register_case compose compose::single_candidate_is_automatic_at_docker_boundary _precedence_single_candidate_is_automatic
register_case compose compose::file_flag_after_command_is_forwarded _precedence_file_flag_after_command_is_forwarded

# Possible bugs/quirks caused by mapfile/process-substitution status handling.
register_case compose compose::non_tty_multiple_error_becomes_file_flag _precedence_non_tty_multiple_error_becomes_file_flag
register_case compose compose::invalid_selector_error_becomes_file_flag _precedence_invalid_selector_error_becomes_file_flag
register_case compose compose::empty_compose_flag_resolver_succeeds _precedence_empty_flag_resolver_succeeds
