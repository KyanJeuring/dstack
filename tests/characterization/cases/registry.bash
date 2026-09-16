#!/usr/bin/env bash

# Core registry cases are intentional contract candidates where README and
# implementation agree. Unescaped names and whitespace failures are explicitly
# possible bugs/quirks rather than permanent contracts.

_registry_file_contains() {
  local file="$1"
  local text="$2"
  local label="${3:-$file}"

  if ! grep -Fq "$text" "$file"; then
    fail "$label: expected to contain $(printf '%q' "$text")"
    return 1
  fi
}

_registry_assert_absent() {
  local path="$1"
  local label="${2:-$path}"

  if [[ -e "$path" || -L "$path" ]]; then
    fail "$label: expected path to be absent"
    return 1
  fi
}

_registry_assert_success() {
  local label="$1"

  assert_status 0 "$CAPTURE_STATUS" "$label status"
  assert_file_empty "$CAPTURE_STDERR" "$label stderr"
  assert_call_count 0
}

_registry_run() {
  local name="$1"
  shift

  set_child_state_vars
  run_dstack_function "$CASE_CAPTURE_DIR/$name" "$@"
}

_registry_physical_path() {
  (cd "$1" && pwd -P)
}

_registry_list_creates_empty_registry() {
  _registry_run list dstack
  _registry_assert_success "empty registry list"
  assert_text_file "$CASE_REGISTRY" "" "created registry"
  assert_text_file "$CAPTURE_STDOUT" \
    $'[INFO] Registered stacks:\n  No registered stacks found.\n[INFO] Auto-discovered stacks:\n' \
    "empty registry listing"
}

_registry_help_does_not_create_registry() {
  _registry_run help dstack --help
  _registry_assert_success "dstack help"
  _registry_file_contains "$CAPTURE_STDOUT" "USAGE:" "dstack help output"
  _registry_assert_absent "$CASE_REGISTRY" "registry after help"
}

_registry_ignores_xdg_config_home() {
  local xdg_root="$CASE_ROOT/xdg-config"

  XDG_CONFIG_HOME="$xdg_root"
  export XDG_CONFIG_HOME
  _registry_run xdg dstack
  _registry_assert_success "XDG registry list"
  if [[ ! -f "$CASE_REGISTRY" ]]; then
    fail "fixed HOME registry was not created"
  fi
  _registry_assert_absent "$xdg_root/dstack/registry" "XDG registry"
}

_registry_register_valid_stack() {
  local physical

  fixture_create_compose_project registered compose.yml 'services: {}'
  physical="$(_registry_physical_path "$FIXTURE_PROJECT")"
  _registry_run register dstack add app "$FIXTURE_PROJECT"
  _registry_assert_success "valid registration"
  assert_text_file "$CASE_REGISTRY" "app=$physical"$'\n' "registered entry"
  assert_text_file "$CAPTURE_STDOUT" "[OK] Registered stack 'app' -> $physical"$'\n' \
    "registration output"
  _registry_assert_absent "$CASE_REGISTRY.tmp" "registration temp file"
}

_registry_register_symlink_uses_physical_path_when_supported() {
  local canonical
  local link="$CASE_PROJECTS/registered-link"

  fixture_create_compose_project registered-target compose.yml 'services: {}'
  if ! ln -s "$FIXTURE_PROJECT" "$link" 2>/dev/null; then
    # Windows runners may not permit symlink creation. Phase 7 records the
    # platform capability explicitly; supported hosts exercise pwd -P here.
    assert_call_count 0
    return 0
  fi
  canonical="$(_registry_physical_path "$link")"

  _registry_run symlink dstack add linked "$link"
  _registry_assert_success "symlink registration"
  assert_text_file "$CASE_REGISTRY" "linked=$canonical"$'\n' "pwd -P symlink result"
}

_registry_duplicate_registration_replaces_one_entry() {
  local physical

  fixture_create_compose_project duplicate compose.yml 'services: {}'
  physical="$(_registry_physical_path "$FIXTURE_PROJECT")"
  _registry_run duplicate-first dstack add app "$FIXTURE_PROJECT"
  _registry_assert_success "first duplicate registration"
  _registry_run duplicate-second dstack add app "$FIXTURE_PROJECT"
  _registry_assert_success "second duplicate registration"
  assert_text_file "$CASE_REGISTRY" "app=$physical"$'\n' "deduplicated registry"
  _registry_assert_absent "$CASE_REGISTRY.tmp" "duplicate registration temp file"
}

_registry_replaces_existing_registration() {
  local first
  local second

  fixture_create_compose_project first compose.yml 'services: {}'
  first="$FIXTURE_PROJECT"
  fixture_create_compose_project second compose.yaml 'services: {}'
  second="$FIXTURE_PROJECT"
  _registry_run replace-first dstack add app "$first"
  _registry_assert_success "first replacement registration"
  _registry_run replace-second dstack add app "$second"
  _registry_assert_success "replacement registration"
  assert_text_file "$CASE_REGISTRY" \
    "app=$(_registry_physical_path "$second")"$'\n' "replaced registry entry"
}

_registry_invalid_directory_returns_success() {
  local missing="$CASE_PROJECTS/missing"

  _registry_run invalid-directory dstack add app "$missing"
  _registry_assert_success "invalid directory registration"
  assert_text_file "$CAPTURE_STDOUT" $'[ERROR] Invalid path\n' "invalid directory output"
  assert_text_file "$CASE_REGISTRY" "" "registry after invalid directory"
}

_registry_directory_without_compose_returns_success() {
  local physical

  fixture_create_project empty
  physical="$(_registry_physical_path "$FIXTURE_PROJECT")"
  _registry_run no-compose dstack add app "$FIXTURE_PROJECT"
  _registry_assert_success "directory without compose registration"
  assert_text_file "$CAPTURE_STDOUT" \
    "[ERROR] No Docker Compose file found in $physical"$'\n' \
    "missing compose output"
  assert_text_file "$CASE_REGISTRY" "" "registry after missing compose"
}

_registry_list_prints_stale_entry() {
  local stale="$CASE_PROJECTS/does-not-exist"

  fixture_create_registry
  fixture_add_registry_entry stale "$stale"
  _registry_run stale-list dstack ls
  _registry_assert_success "stale registry list"
  _registry_file_contains "$CAPTURE_STDOUT" "stale" "stale entry name"
  _registry_file_contains "$CAPTURE_STDOUT" "$stale" "stale entry path"
  assert_text_file "$CASE_REGISTRY" "stale=$stale"$'\n' "unchanged stale registry"
}

_registry_unregisters_exact_entry() {
  local first="$CASE_PROJECTS/first"
  local second="$CASE_PROJECTS/second"

  fixture_create_registry
  fixture_add_registry_entry first "$first"
  fixture_add_registry_entry second "$second"
  _registry_run unregister dstackunset first
  _registry_assert_success "unregister existing stack"
  assert_text_file "$CASE_REGISTRY" "second=$second"$'\n' "registry after unregister"
  assert_text_file "$CAPTURE_STDOUT" "[OK] Unregistered docker stack 'first'"$'\n' \
    "unregister output"
  _registry_assert_absent "$CASE_REGISTRY.tmp" "unregister temp file"
}

_registry_unregister_without_registry_returns_success() {
  _registry_run unregister-missing dstackunset ghost
  _registry_assert_success "unregister without registry"
  assert_text_file "$CAPTURE_STDOUT" $'[ERROR] No stack registry found\n' \
    "missing registry output"
  _registry_assert_absent "$CASE_REGISTRY" "registry after missing unregister"
}

_registry_unregister_unknown_name_returns_success() {
  local path="$CASE_PROJECTS/existing"

  fixture_create_registry
  fixture_add_registry_entry existing "$path"
  _registry_run unregister-unknown dstackunset ghost
  _registry_assert_success "unregister unknown stack"
  assert_text_file "$CAPTURE_STDOUT" "[ERROR] Stack 'ghost' is not registered"$'\n' \
    "unknown unregister output"
  assert_text_file "$CASE_REGISTRY" "existing=$path"$'\n' "unchanged registry"
}

_registry_nested_physical_path_round_trips() {
  local physical

  fixture_create_nested_stack monorepo/services/api docker-compose.yml 'services: {}'
  physical="$(_registry_physical_path "$FIXTURE_STACK")"
  _registry_run nested dstack add nested-api "$FIXTURE_STACK"
  _registry_assert_success "nested registration"
  assert_text_file "$CASE_REGISTRY" "nested-api=$physical"$'\n' "nested registry entry"
}

_registry_path_with_spaces_fails_compose_probe() {
  local physical

  fixture_create_compose_project "project with spaces" compose.yml 'services: {}'
  physical="$(_registry_physical_path "$FIXTURE_PROJECT")"
  _registry_run spaces dstack add spaced "$FIXTURE_PROJECT"
  _registry_assert_success "space path registration"
  assert_text_file "$CAPTURE_STDOUT" \
    "[ERROR] No Docker Compose file found in $physical"$'\n' \
    "space path compose probe"
  assert_text_file "$CASE_REGISTRY" "" "registry after space path"
}

_registry_regex_name_removes_matching_entries() {
  local first="$CASE_PROJECTS/first"
  local keep="$CASE_PROJECTS/keep"
  local added

  fixture_create_compose_project added compose.yml 'services: {}'
  added="$(_registry_physical_path "$FIXTURE_PROJECT")"
  fixture_create_registry
  fixture_add_registry_entry application "$first"
  fixture_add_registry_entry keep "$keep"

  _registry_run regex-name dstack add 'app.*' "$FIXTURE_PROJECT"
  _registry_assert_success "regex-sensitive registration"
  assert_text_file "$CASE_REGISTRY" \
    "keep=$keep"$'\n'"app.*=$added"$'\n' \
    "regex-sensitive registry rewrite"
}

_registry_equals_name_is_written_unescaped() {
  local physical

  fixture_create_compose_project equals compose.yml 'services: {}'
  physical="$(_registry_physical_path "$FIXTURE_PROJECT")"
  _registry_run equals-name dstack add 'team=prod' "$FIXTURE_PROJECT"
  _registry_assert_success "equals name registration"
  assert_text_file "$CASE_REGISTRY" "team=prod=$physical"$'\n' \
    "unescaped equals registry entry"
}

_registry_fixed_temp_file_is_replaced_and_removed() {
  local physical

  fixture_create_compose_project temp compose.yml 'services: {}'
  physical="$(_registry_physical_path "$FIXTURE_PROJECT")"
  fixture_create_registry
  printf 'sentinel temp bytes\n' >"$CASE_REGISTRY.tmp"
  _registry_run fixed-temp dstack add temp "$FIXTURE_PROJECT"
  _registry_assert_success "fixed registry temp update"
  assert_text_file "$CASE_REGISTRY" "temp=$physical"$'\n' "registry after temp update"
  _registry_assert_absent "$CASE_REGISTRY.tmp" "fixed registry temp after update"
}

# Intentional contract candidates.
register_case registry registry::list_creates_empty_registry _registry_list_creates_empty_registry
register_case registry registry::help_does_not_create_registry _registry_help_does_not_create_registry
register_case registry registry::xdg_config_home_is_ignored _registry_ignores_xdg_config_home
register_case registry registry::register_valid_stack_writes_physical_path _registry_register_valid_stack
register_case registry registry::register_symlink_uses_pwd_p_when_supported _registry_register_symlink_uses_physical_path_when_supported
register_case registry registry::duplicate_registration_replaces_one_entry _registry_duplicate_registration_replaces_one_entry
register_case registry registry::same_name_new_path_replaces_old_path _registry_replaces_existing_registration
register_case registry registry::invalid_directory_reports_error_and_returns_zero _registry_invalid_directory_returns_success
register_case registry registry::directory_without_compose_reports_error_and_returns_zero _registry_directory_without_compose_returns_success
register_case registry registry::list_prints_stale_entries_without_validation _registry_list_prints_stale_entry
register_case registry registry::unregister_removes_exact_entry _registry_unregisters_exact_entry
register_case registry registry::unregister_missing_registry_returns_zero _registry_unregister_without_registry_returns_success
register_case registry registry::unregister_unknown_name_returns_zero _registry_unregister_unknown_name_returns_success
register_case registry registry::nested_physical_path_round_trips _registry_nested_physical_path_round_trips
register_case registry registry::fixed_registry_tmp_is_replaced_and_removed _registry_fixed_temp_file_is_replaced_and_removed

# Possible bugs/quirks caused by unescaped or whitespace-sensitive storage.
register_case registry registry::registered_path_with_spaces_fails_current_file_probe _registry_path_with_spaces_fails_compose_probe
register_case registry registry::regex_name_removes_matching_entries _registry_regex_name_removes_matching_entries
register_case registry registry::equals_name_is_written_unescaped _registry_equals_name_is_written_unescaped
