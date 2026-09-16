#!/usr/bin/env bash

_selector_prepare_three() {
  SELECTOR_DIR="$CASE_CURRENT_DIR"
  SELECTOR_FIRST="$SELECTOR_DIR/docker-compose.yml"
  SELECTOR_SECOND="$SELECTOR_DIR/compose.yml"
  SELECTOR_LAST="$SELECTOR_DIR/extra-compose.yaml"
  : >"$SELECTOR_FIRST"
  : >"$SELECTOR_SECOND"
  : >"$SELECTOR_LAST"
}

_selector_run() {
  local name="$1"
  local selector="$2"

  _compose_run "$name" _dstack_resolve_compose_files "$SELECTOR_DIR" "$selector"
}

_selector_numeric_first_and_last() {
  _selector_prepare_three
  _selector_run first 1
  _compose_expect 0 "$SELECTOR_FIRST"$'\n'
  _selector_run last 3
  _compose_expect 0 "$SELECTOR_LAST"$'\n'
}

_selector_zero_is_invalid() {
  _selector_prepare_three
  _selector_run zero 0
  _compose_expect 1 $'[ERROR] Compose file not found: 0\n'
}

_selector_out_of_range_is_invalid() {
  _selector_prepare_three
  _selector_run range 4
  _compose_expect 1 $'[ERROR] Compose file not found: 4\n'
}

_selector_all_returns_every_candidate() {
  _selector_prepare_three
  _selector_run all all
  _compose_expect 0 \
    "$SELECTOR_FIRST"$'\n'"$SELECTOR_SECOND"$'\n'"$SELECTOR_LAST"$'\n'
}

_selector_comma_list_and_whitespace() {
  _selector_prepare_three
  _selector_run comma ' 1 , 3 '
  _compose_expect 0 "$SELECTOR_FIRST"$'\n'"$SELECTOR_LAST"$'\n'
}

_selector_duplicate_items_are_removed() {
  _selector_prepare_three
  _selector_run duplicate '1,1,compose.yml'
  _compose_expect 0 "$SELECTOR_FIRST"$'\n'"$SELECTOR_SECOND"$'\n'
}

_selector_basename_and_relative_name() {
  _selector_prepare_three
  _selector_run basename extra-compose.yaml
  _compose_expect 0 "$SELECTOR_LAST"$'\n'
  _selector_run relative compose.yml
  _compose_expect 0 "$SELECTOR_SECOND"$'\n'
}

_selector_existing_slash_path_need_not_be_candidate() {
  local selected="$CASE_CURRENT_DIR/sub/custom.yml"

  _selector_prepare_three
  mkdir -p "${selected%/*}"
  : >"$selected"
  _selector_run slash sub/custom.yml
  _compose_expect 0 $'sub/custom.yml\n'
}

_selector_absolute_path_need_not_be_candidate() {
  local selected="$CASE_PROJECTS/external/custom.yml"

  _selector_prepare_three
  mkdir -p "${selected%/*}"
  : >"$selected"
  _selector_run absolute "$selected"
  _compose_expect 0 "$selected"$'\n'
}

_selector_invalid_name_errors_to_stdout() {
  _selector_prepare_three
  _selector_run invalid missing.yml
  _compose_expect 1 $'[ERROR] Compose file not found: missing.yml\n'
}

_selector_empty_item_is_skipped() {
  _selector_prepare_three
  _selector_run empty-item '1,,3,'
  _compose_expect 0 "$SELECTOR_FIRST"$'\n'"$SELECTOR_LAST"$'\n'
}

register_case compose compose::selector_numeric_first_and_last _selector_numeric_first_and_last
register_case compose compose::selector_zero_is_invalid _selector_zero_is_invalid
register_case compose compose::selector_out_of_range_is_invalid _selector_out_of_range_is_invalid
register_case compose compose::selector_all_returns_every_candidate _selector_all_returns_every_candidate
register_case compose compose::selector_comma_list_removes_whitespace _selector_comma_list_and_whitespace
register_case compose compose::selector_duplicate_items_are_removed _selector_duplicate_items_are_removed
register_case compose compose::selector_basename_and_relative_name _selector_basename_and_relative_name
register_case compose compose::selector_existing_slash_path_need_not_be_candidate _selector_existing_slash_path_need_not_be_candidate
register_case compose compose::selector_absolute_path_need_not_be_candidate _selector_absolute_path_need_not_be_candidate
register_case compose compose::selector_invalid_name_errors_to_stdout _selector_invalid_name_errors_to_stdout
register_case compose compose::selector_empty_comma_item_is_skipped _selector_empty_item_is_skipped
