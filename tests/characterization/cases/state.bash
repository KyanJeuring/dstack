#!/usr/bin/env bash

_state_prepare_registered() {
  local name="$1"

  fixture_create_compose_project "$name" compose.yml 'services: {}'
  STATE_STACK="$FIXTURE_PROJECT"
  fixture_create_registry
  fixture_add_registry_entry "$name" "$STATE_STACK"
}

_state_selecting_stack_exports_context_and_clears_selector() {
  _state_prepare_registered app
  DSTACK_COMPOSE_FILE=old-selector
  export DSTACK_COMPOSE_FILE
  set_child_state_vars DSTACK DSTACK_COMPOSE_FILE
  run_dstack_function "$CASE_CAPTURE_DIR/select" dstack app

  assert_status 0 "$CAPTURE_STATUS"
  assert_child_state DSTACK set "$STATE_STACK" exported
  assert_child_state DSTACK_COMPOSE_FILE unset
  assert_call_count 0
}

_state_selecting_stack_and_selector_exports_both() {
  _state_prepare_registered app
  set_child_state_vars DSTACK DSTACK_COMPOSE_FILE
  run_dstack_function "$CASE_CAPTURE_DIR/select-file" dstack app missing.yml

  assert_status 0 "$CAPTURE_STATUS"
  assert_child_state DSTACK set "$STATE_STACK" exported
  assert_child_state DSTACK_COMPOSE_FILE set missing.yml exported
  assert_call_count 0
}

_state_changing_stack_replaces_context() {
  local first
  local second
  local probe="$CASE_PROJECTS/change-stack.bash"

  fixture_create_compose_project first compose.yml 'services: {}'
  first="$FIXTURE_PROJECT"
  fixture_create_compose_project second compose.yml 'services: {}'
  second="$FIXTURE_PROJECT"
  fixture_create_registry
  fixture_add_registry_entry first "$first"
  fixture_add_registry_entry second "$second"
  cat >"$probe" <<'EOF'
characterization_child_main() {
  local dstack_path="$1"
  source "$dstack_path"
  dstack first chosen.yml
  dstack second
}
EOF
  set_child_state_vars DSTACK DSTACK_COMPOSE_FILE
  run_child_bash "$CASE_CAPTURE_DIR/change" -- "$probe" "$CHARACTERIZATION_REPO_ROOT/dstack.sh"

  assert_status 0 "$CAPTURE_STATUS"
  assert_child_state DSTACK set "$second" exported
  assert_child_state DSTACK_COMPOSE_FILE unset
  assert_call_count 0
}

_state_unregister_active_registered_stack_leaves_context_quirk() {
  _state_prepare_registered app
  DSTACK="$STATE_STACK"
  DSTACK_COMPOSE_FILE=compose.yml
  export DSTACK DSTACK_COMPOSE_FILE
  set_child_state_vars DSTACK DSTACK_COMPOSE_FILE
  run_dstack_function "$CASE_CAPTURE_DIR/unregister-active" dstackunset app

  assert_status 0 "$CAPTURE_STATUS"
  assert_child_state DSTACK set "$STATE_STACK" exported
  assert_child_state DSTACK_COMPOSE_FILE set compose.yml exported
  assert_text_file "$CASE_REGISTRY" ""
  assert_call_count 0
}

_state_exported_context_is_inherited_by_child_process() {
  local probe="$CASE_PROJECTS/inherited-context.bash"
  local inherited="$CASE_CAPTURE_DIR/inherited.nul"

  _state_prepare_registered app
  cat >"$probe" <<'EOF'
characterization_child_main() {
  local dstack_path="$1"
  local bash_path="$2"
  local inherited="$3"
  source "$dstack_path"
  dstack app 1 >/dev/null
  "$bash_path" -c 'printf "%s\0%s\0" "$DSTACK" "$DSTACK_COMPOSE_FILE"' >"$inherited"
}
EOF
  set_child_state_vars
  run_child_bash "$CASE_CAPTURE_DIR/inherited" -- "$probe" \
    "$CHARACTERIZATION_REPO_ROOT/dstack.sh" "$CHARACTERIZATION_BASH" "$inherited"

  assert_status 0 "$CAPTURE_STATUS"
  _assertion_next_file inherited.expected.nul
  write_nul_arguments "$ASSERTION_FILE" "$STATE_STACK" 1
  assert_bytes_equal "$ASSERTION_FILE" "$inherited" "inherited stack context"
  assert_call_count 0
}

register_case status state::selecting_stack_exports_context_and_clears_selector _state_selecting_stack_exports_context_and_clears_selector
register_case status state::selecting_stack_and_unvalidated_selector_exports_both _state_selecting_stack_and_selector_exports_both
register_case status state::changing_stack_replaces_context_and_clears_selector _state_changing_stack_replaces_context

# Possible bug/quirk documented by AGENTS.md.
register_case status state::unregister_active_registered_stack_leaves_context _state_unregister_active_registered_stack_leaves_context_quirk
register_case status state::exported_context_is_inherited_by_child_process _state_exported_context_is_inherited_by_child_process
