#!/usr/bin/env bash

_compose_run() {
  local name="$1"
  shift

  set_child_state_vars
  run_dstack_function "$CASE_CAPTURE_DIR/$name" "$@"
}

_compose_expect() {
  local status="$1"
  local stdout="$2"

  assert_status "$status" "$CAPTURE_STATUS"
  assert_text_file "$CAPTURE_STDOUT" "$stdout" "compose helper stdout"
  assert_file_empty "$CAPTURE_STDERR" "compose helper stderr"
  assert_call_count 0
}

_compose_discovery_no_candidates() {
  _compose_run none _dstack_compose_files "$CASE_CURRENT_DIR"
  _compose_expect 0 ""

  _compose_run resolve-none _dstack_resolve_compose_files "$CASE_CURRENT_DIR" ""
  _compose_expect 1 ""
}

_compose_discovery_standard_candidate_order() {
  local dir="$CASE_CURRENT_DIR"

  : >"$dir/docker-compose.yml"
  : >"$dir/docker-compose.yaml"
  : >"$dir/compose.yml"
  : >"$dir/compose.yaml"

  _compose_run standard-order _dstack_compose_files "$dir"
  _compose_expect 0 \
    "$dir/docker-compose.yml"$'\n'"$dir/docker-compose.yaml"$'\n'"$dir/compose.yml"$'\n'"$dir/compose.yaml"$'\n'
}

_compose_discovery_globs_follow_standard_candidates() {
  local dir="$CASE_CURRENT_DIR"

  : >"$dir/docker-compose.yml"
  : >"$dir/docker-compose.yaml"
  : >"$dir/compose.yml"
  : >"$dir/compose.yaml"
  : >"$dir/aaa-compose.yml"
  : >"$dir/zzz-compose.yaml"

  _compose_run glob-order _dstack_compose_files "$dir"
  _compose_expect 0 \
    "$dir/docker-compose.yml"$'\n'"$dir/docker-compose.yaml"$'\n'"$dir/compose.yml"$'\n'"$dir/compose.yaml"$'\n'"$dir/aaa-compose.yml"$'\n'"$dir/zzz-compose.yaml"$'\n'
}

_compose_discovery_duplicate_candidates_are_removed() {
  local dir="$CASE_CURRENT_DIR"

  : >"$dir/docker-compose.yml"
  : >"$dir/compose.yml"
  _compose_run duplicates _dstack_compose_files "$dir"
  _compose_expect 0 "$dir/docker-compose.yml"$'\n'"$dir/compose.yml"$'\n'
}

_compose_discovery_one_candidate_is_automatic() {
  local file="$CASE_CURRENT_DIR/compose.yaml"

  : >"$file"
  _compose_run automatic _dstack_resolve_compose_files "$CASE_CURRENT_DIR" ""
  _compose_expect 0 "$file"$'\n'
}

_compose_discovery_multiple_candidates_non_tty_errors() {
  local dir="$CASE_CURRENT_DIR"

  : >"$dir/docker-compose.yml"
  : >"$dir/compose.yml"
  _compose_run multiple _dstack_resolve_compose_files "$dir" ""
  _compose_expect 1 \
    $'[ERROR] Multiple compose files found. Choose one with -f, --file, --compose-file, or DSTACK_COMPOSE_FILE.\n'
}

_compose_discovery_custom_values_are_verbatim() {
  local dir="$CASE_CURRENT_DIR"

  DSTACK_COMPOSE_FILES='relative.yml missing.yaml /absolute/custom.yml'
  export DSTACK_COMPOSE_FILES
  _compose_run custom-values _dstack_compose_files "$dir"
  _compose_expect 0 $'relative.yml\nmissing.yaml\n/absolute/custom.yml\n'

  _compose_run custom-index _dstack_resolve_compose_files "$dir" 2
  _compose_expect 0 $'missing.yaml\n'
}

_compose_discovery_custom_values_split_spaces() {
  local value="$CASE_CURRENT_DIR/file with spaces.yml"

  DSTACK_COMPOSE_FILES="$value"
  export DSTACK_COMPOSE_FILES
  _compose_run custom-spaces _dstack_compose_files "$CASE_CURRENT_DIR"
  _compose_expect 0 \
    "$CASE_CURRENT_DIR/file"$'\nwith\nspaces.yml\n'
}

_compose_discovery_default_paths_preserve_spaces() {
  local dir="$CASE_PROJECTS/project with spaces"

  mkdir -p "$dir"
  : >"$dir/compose.yml"
  _compose_run default-spaces _dstack_resolve_compose_files "$dir" 1
  _compose_expect 0 "$dir/compose.yml"$'\n'
}

register_case compose compose::discovery_no_candidates _compose_discovery_no_candidates
register_case compose compose::standard_candidate_order _compose_discovery_standard_candidate_order
register_case compose compose::glob_candidates_follow_standard_candidates _compose_discovery_globs_follow_standard_candidates
register_case compose compose::duplicate_candidates_are_removed _compose_discovery_duplicate_candidates_are_removed
register_case compose compose::one_candidate_is_automatic _compose_discovery_one_candidate_is_automatic
register_case compose compose::multiple_candidates_non_tty_errors _compose_discovery_multiple_candidates_non_tty_errors
register_case compose compose::custom_candidate_values_are_verbatim _compose_discovery_custom_values_are_verbatim
register_case compose compose::custom_candidate_values_split_spaces _compose_discovery_custom_values_split_spaces
register_case compose compose::default_candidate_path_preserves_spaces _compose_discovery_default_paths_preserve_spaces
