#!/usr/bin/env bash

# These cases characterize current stack lookup and parser precedence. Command
# collisions and invalid-active behavior are compatibility-sensitive/undecided.

_resolution_run() {
  local name="$1"
  shift

  set_child_state_vars
  run_dstack_function "$CASE_CAPTURE_DIR/$name" "$@"
}

_resolution_expect_clean_call() {
  local expected_status="${1:-0}"

  assert_status "$expected_status" "$CAPTURE_STATUS"
  assert_file_empty "$CAPTURE_STDERR" "DStack stderr"
}

_resolution_make_discovered_stack() {
  local name="$1"
  local filename="${2:-compose.yml}"

  fixture_create_discovered_stack primary "$name" "$filename" 'services: {}'
  DSTACK_BASES="$FIXTURE_BASE"
  export DSTACK_BASES
}

_resolution_registered_stack_resolves() {
  local physical

  fixture_create_compose_project registered compose.yml 'services: {}'
  physical="$(cd "$FIXTURE_PROJECT" && pwd -P)"
  fixture_create_registry
  fixture_add_registry_entry app "$physical"

  _resolution_run registered _dstack_resolve app
  _resolution_expect_clean_call
  assert_text_file "$CAPTURE_STDOUT" "$physical"$'\n' "registered resolution"
  assert_call_count 0
}

_resolution_discovered_stack_resolves() {
  local stack

  _resolution_make_discovered_stack app
  stack="$FIXTURE_STACK"
  _resolution_run discovered _dstack_resolve app
  _resolution_expect_clean_call
  assert_text_file "$CAPTURE_STDOUT" "$stack"$'\n' "discovered resolution"
  assert_call_count 0
}

_resolution_registry_precedes_discovery() {
  local registered

  fixture_create_compose_project registered compose.yml 'services: {}'
  registered="$FIXTURE_PROJECT"
  _resolution_make_discovered_stack app
  fixture_create_registry
  fixture_add_registry_entry app "$registered"

  _resolution_run precedence _dstack_resolve app
  _resolution_expect_clean_call
  assert_text_file "$CAPTURE_STDOUT" "$registered"$'\n' "registry precedence"
  assert_call_count 0
}

_resolution_stale_registry_falls_through() {
  local discovered

  _resolution_make_discovered_stack app
  discovered="$FIXTURE_STACK"
  fixture_create_registry
  fixture_add_registry_entry app "$CASE_PROJECTS/stale"

  _resolution_run stale _dstack_resolve app
  _resolution_expect_clean_call
  assert_text_file "$CAPTURE_STDOUT" "$discovered"$'\n' "stale fallback"
  assert_call_count 0
}

_resolution_default_base_order() {
  local projects="$CASE_HOME/projects/app"
  local src="$CASE_HOME/src/app"

  mkdir -p "$projects" "$src"
  : >"$projects/compose.yml"
  : >"$src/compose.yml"
  DSTACK_BASES=""
  export DSTACK_BASES

  _resolution_run default-bases _dstack_resolve app
  _resolution_expect_clean_call
  assert_text_file "$CAPTURE_STDOUT" "$projects"$'\n' "default base ordering"
  assert_call_count 0
}

_resolution_custom_base_order() {
  local first="$CASE_BASES/first"
  local second="$CASE_BASES/second"

  mkdir -p "$first/app" "$second/app"
  : >"$first/app/compose.yml"
  : >"$second/app/compose.yml"
  DSTACK_BASES="$second $first"
  export DSTACK_BASES

  _resolution_run custom-bases _dstack_resolve app
  _resolution_expect_clean_call
  assert_text_file "$CAPTURE_STDOUT" "$second/app"$'\n' "custom base ordering"
  assert_call_count 0
}

_resolution_space_in_custom_base_is_split() {
  local base="$CASE_BASES/base with space"

  mkdir -p "$base/app"
  : >"$base/app/compose.yml"
  DSTACK_BASES="$base"
  export DSTACK_BASES

  _resolution_run space-base _dstack_resolve app
  _resolution_expect_clean_call 1
  assert_file_empty "$CAPTURE_STDOUT" "space-containing base resolution"
  assert_call_count 0
}

_resolution_nested_name_has_no_leaf_search() {
  local stack

  fixture_create_discovered_stack primary team/api compose.yml 'services: {}'
  stack="$FIXTURE_STACK"
  DSTACK_BASES="$FIXTURE_BASE"
  export DSTACK_BASES

  _resolution_run nested _dstack_resolve team/api
  _resolution_expect_clean_call
  assert_text_file "$CAPTURE_STDOUT" "$stack"$'\n' "nested stack resolution"

  _resolution_run leaf _dstack_resolve api
  _resolution_expect_clean_call 1
  assert_file_empty "$CAPTURE_STDOUT" "leaf basename resolution"
  assert_call_count 0
}

_resolution_missing_stack_fails_silently() {
  _resolution_run missing _dstack_resolve absent
  _resolution_expect_clean_call 1
  assert_file_empty "$CAPTURE_STDOUT" "missing stack stdout"
  assert_call_count 0
}

_resolution_explicit_precedes_active_and_local() {
  local explicit
  local active

  fixture_create_compose_project explicit compose.yml 'services: {}'
  explicit="$FIXTURE_PROJECT"
  fixture_create_compose_project active compose.yml 'services: {}'
  active="$FIXTURE_PROJECT"
  fixture_write_file projects/current/compose.yml 'services: {}'
  fixture_create_registry
  fixture_add_registry_entry chosen "$explicit"
  DSTACK="$active"
  export DSTACK

  _resolution_run explicit-precedence dstart chosen
  _resolution_expect_clean_call
  assert_call_count 1
  assert_argv 1 compose -f "$explicit/compose.yml" start
}

_resolution_active_precedes_local() {
  local active

  fixture_create_compose_project active compose.yml 'services: {}'
  active="$FIXTURE_PROJECT"
  fixture_write_file projects/current/compose.yml 'services: {}'
  DSTACK="$active"
  export DSTACK

  _resolution_run active-precedence dstart
  _resolution_expect_clean_call
  assert_call_count 1
  assert_argv 1 compose -f "$active/compose.yml" start
}

_resolution_local_current_directory_fallback() {
  fixture_write_file projects/current/compose.yml 'services: {}'

  _resolution_run local dstart
  _resolution_expect_clean_call
  assert_call_count 1
  assert_argv 1 compose -f "$CASE_CURRENT_DIR/compose.yml" start
}

_resolution_parent_directory_is_not_searched() {
  local child="$CASE_CURRENT_DIR/child"

  fixture_write_file projects/current/compose.yml 'services: {}'
  mkdir -p "$child"
  fixture_set_cwd "$child"

  _resolution_run parent-not-searched dstart
  _resolution_expect_clean_call
  assert_text_file "$CAPTURE_STDOUT" \
    $'[ERROR] No compose file found in the current directory or active stack context.\n[ERROR] Use \'dstack\' to set an active stack or run from a directory with a compose file.\n' \
    "exact current-directory error"
  assert_call_count 0
}

_resolution_invalid_active_clears_and_keeps_selector() {
  local missing="$CASE_PROJECTS/missing-active"

  fixture_write_file projects/current/compose.yml 'services: {}'
  DSTACK="$missing"
  DSTACK_COMPOSE_FILE=compose.yml
  export DSTACK DSTACK_COMPOSE_FILE
  set_child_state_vars DSTACK DSTACK_COMPOSE_FILE
  run_dstack_function "$CASE_CAPTURE_DIR/invalid-active" dstart

  _resolution_expect_clean_call
  assert_text_file "$CAPTURE_STDOUT" $'[WARN] DSTACK invalid, clearing\n' \
    "invalid active warning"
  assert_child_state DSTACK unset
  assert_child_state DSTACK_COMPOSE_FILE set compose.yml exported
  assert_call_count 1
  assert_argv 1 compose -f "$CASE_CURRENT_DIR/compose.yml" start
}

_resolution_unresolved_first_token_reaches_active_stack() {
  local active

  fixture_create_compose_project active compose.yml 'services: {}'
  active="$FIXTURE_PROJECT"
  DSTACK="$active"
  export DSTACK

  _resolution_run unresolved-token dcompose status --quiet
  _resolution_expect_clean_call
  assert_call_count 1
  assert_argv 1 compose -f "$active/compose.yml" status --quiet
}

_resolution_compose_words_that_are_stacks_are_consumed() {
  local base="$CASE_BASES/collisions"
  local name

  for name in up start restart logs; do
    mkdir -p "$base/$name"
    : >"$base/$name/compose.yml"
  done
  DSTACK_BASES="$base"
  export DSTACK_BASES

  _resolution_run collision-up dcompose up
  _resolution_run collision-start dstart start
  _resolution_run collision-restart drestart restart
  _resolution_run collision-logs dlogs logs
  assert_call_count 4
  assert_argv 1 compose -f "$base/up/compose.yml" up -d --build --remove-orphans
  assert_argv 2 compose -f "$base/start/compose.yml" start
  assert_argv 3 compose -f "$base/restart/compose.yml" restart
  assert_argv 4 compose -f "$base/logs/compose.yml" logs -f --tail=100
}

_resolution_service_name_matching_stack_is_stack() {
  local base="$CASE_BASES/services"

  mkdir -p "$base/api"
  : >"$base/api/compose.yml"
  fixture_write_file projects/current/compose.yml 'services: {}'
  DSTACK_BASES="$base"
  export DSTACK_BASES

  _resolution_run service-collision drestart api
  _resolution_expect_clean_call
  assert_call_count 1
  assert_argv 1 compose -f "$base/api/compose.yml" restart
}

_resolution_drun_requires_two_tokens_to_consume_stack() {
  local base="$CASE_BASES/drun"

  mkdir -p "$base/api"
  : >"$base/api/compose.yml"
  fixture_write_file projects/current/compose.yml 'services: {}'
  DSTACK_BASES="$base"
  export DSTACK_BASES

  _resolution_run drun-one drun api
  _resolution_expect_clean_call
  _resolution_run drun-two drun api echo
  _resolution_expect_clean_call
  assert_call_count 2
  assert_argv 1 compose -f "$CASE_CURRENT_DIR/compose.yml" run --rm api
  assert_argv 2 compose -f "$base/api/compose.yml" run --rm echo
}

_resolution_duplicate_resolvers_accept_explicit_stack() {
  local stack

  _resolution_make_discovered_stack app
  stack="$FIXTURE_STACK"

  _resolution_run explicit-recompose drecompose app
  _resolution_run explicit-reboot drebootstack app
  _resolution_run explicit-update dupdate app
  _resolution_run explicit-rebuild drebuild app web
  _resolution_run explicit-nocache drebuildnocache app

  assert_call_count 10
  assert_argv 1 compose -f "$stack/compose.yml" down -v
  assert_argv 2 compose -f "$stack/compose.yml" up -d
  assert_argv 3 compose -f "$stack/compose.yml" down
  assert_argv 4 compose -f "$stack/compose.yml" up -d
  assert_argv 5 compose -f "$stack/compose.yml" pull
  assert_argv 6 compose -f "$stack/compose.yml" up -d
  assert_argv 7 compose -f "$stack/compose.yml" build web
  assert_argv 8 compose -f "$stack/compose.yml" up -d web
  assert_argv 9 compose -f "$stack/compose.yml" build --no-cache
  assert_argv 10 compose -f "$stack/compose.yml" up -d
}

_resolution_duplicate_resolver_does_not_fallback_invalid_active() {
  fixture_write_file projects/current/compose.yml 'services: {}'
  DSTACK="$CASE_PROJECTS/missing-active"
  export DSTACK

  _resolution_run invalid-direct drecompose
  _resolution_expect_clean_call
  assert_call_count 2
  assert_argv 1 compose down -v
  assert_argv 2 compose up -d
}

register_case resolution resolution::explicit_registered_stack_resolves _resolution_registered_stack_resolves
register_case resolution resolution::explicit_discovered_stack_resolves _resolution_discovered_stack_resolves
register_case resolution resolution::registered_stack_precedes_discovered_stack _resolution_registry_precedes_discovery
register_case resolution resolution::stale_registry_falls_through_to_discovery _resolution_stale_registry_falls_through
register_case resolution resolution::default_base_order_prefers_home_projects _resolution_default_base_order
register_case resolution resolution::custom_base_order_is_whitespace_split _resolution_custom_base_order
register_case resolution resolution::space_in_custom_base_is_split _resolution_space_in_custom_base_is_split
register_case resolution resolution::nested_name_resolves_without_leaf_search _resolution_nested_name_has_no_leaf_search
register_case resolution resolution::missing_stack_returns_one_without_output _resolution_missing_stack_fails_silently
register_case resolution resolution::explicit_stack_precedes_active_and_local _resolution_explicit_precedes_active_and_local
register_case resolution resolution::active_stack_precedes_local _resolution_active_precedes_local
register_case resolution resolution::current_directory_is_local_fallback _resolution_local_current_directory_fallback
register_case resolution resolution::parent_directory_is_not_searched _resolution_parent_directory_is_not_searched
register_case resolution resolution::invalid_active_clears_but_keeps_selector _resolution_invalid_active_clears_and_keeps_selector
register_case resolution resolution::unresolved_first_token_reaches_active_stack _resolution_unresolved_first_token_reaches_active_stack
register_case resolution resolution::compose_words_that_resolve_as_stacks_are_consumed _resolution_compose_words_that_are_stacks_are_consumed
register_case resolution resolution::service_name_matching_stack_is_treated_as_stack _resolution_service_name_matching_stack_is_stack
register_case resolution resolution::drun_consumes_stack_only_with_two_tokens _resolution_drun_requires_two_tokens_to_consume_stack
register_case resolution resolution::duplicate_resolvers_accept_explicit_stack _resolution_duplicate_resolvers_accept_explicit_stack
register_case resolution resolution::duplicate_resolver_does_not_fallback_invalid_active _resolution_duplicate_resolver_does_not_fallback_invalid_active
