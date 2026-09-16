#!/usr/bin/env bash

run_dstack_function() {
  local prefix="$1"
  local function_name="$2"
  shift 2
  local probe="$CASE_PROJECTS/dstack-invoke.bash"
  local working_directory

  working_directory="$(pwd -P)"
  cat >"$probe" <<'EOF'
characterization_child_main() {
  local dstack_path="$1"
  local working_directory="$2"
  local function_name="$3"
  shift 3

  cd "$working_directory" || return 125
  source "$dstack_path"
  if ! declare -F "$function_name" >/dev/null; then
    printf 'DStack function is not defined: %s\n' "$function_name" >&2
    return 127
  fi
  "$function_name" "$@"
}
EOF

  run_child_bash "$prefix" -- "$probe" \
    "$CHARACTERIZATION_REPO_ROOT/dstack.sh" "$working_directory" \
    "$function_name" "$@"
}
