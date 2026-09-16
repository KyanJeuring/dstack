#!/usr/bin/env bash

CHILD_STATE_VARS=()

set_child_state_vars() {
  local variable_name
  CHILD_STATE_VARS=()

  for variable_name in "$@"; do
    if [[ ! "$variable_name" =~ ^[a-zA-Z_][a-zA-Z0-9_]*$ ]]; then
      printf 'invalid child state variable name: %s\n' "$variable_name" >&2
      return 1
    fi
    CHILD_STATE_VARS+=("$variable_name")
  done
}

_append_child_env_if_set() {
  local variable_name="$1"

  if declare -p "$variable_name" >/dev/null 2>&1; then
    CHILD_ENV+=("$variable_name=${!variable_name}")
  fi
}

run_child_bash() {
  local prefix="$1"
  shift
  local -a bash_options=()
  local child_script
  local state_request="${prefix}.state-request"
  local state_output="${prefix}.state.nul"
  local variable_name
  local -a CHILD_ENV=()

  while [[ $# -gt 0 && "$1" != "--" ]]; do
    bash_options+=("$1")
    shift
  done

  if [[ $# -eq 0 || "$1" != "--" ]]; then
    printf 'run_child_bash requires -- before the child script\n' >&2
    return 1
  fi
  shift

  if [[ $# -eq 0 ]]; then
    printf 'run_child_bash requires a child script\n' >&2
    return 1
  fi
  child_script="$1"
  shift

  : >"$state_request"
  for variable_name in "${CHILD_STATE_VARS[@]}"; do
    printf '%s\n' "$variable_name" >>"$state_request"
  done

  CHILD_ENV=(
    "HOME=$HOME"
    "USER=$USER"
    "PATH=$PATH"
    "PWD=$PWD"
    "DSTACK_BASES=$DSTACK_BASES"
    "DOCKER_CONFIG=$DOCKER_CONFIG"
    "DOCKER_HOST=$DOCKER_HOST"
    "LC_ALL=$LC_ALL"
    "TERM=$TERM"
    "TMPDIR=$TMPDIR"
    "DSTACK_TEST_DOCKER_CONTROL=$DSTACK_TEST_DOCKER_CONTROL"
    "DSTACK_TEST_DOCKER_GUARD=$DSTACK_TEST_DOCKER_GUARD"
    "FAKE_DOCKER_PATH=$FAKE_DOCKER_PATH"
  )

  _append_child_env_if_set DSTACK
  _append_child_env_if_set DSTACK_COMPOSE_FILE
  _append_child_env_if_set DSTACK_COMPOSE_FILES
  _append_child_env_if_set XDG_CONFIG_HOME

  case "${OSTYPE:-}" in
    msys*|cygwin*)
      _append_child_env_if_set SYSTEMROOT
      _append_child_env_if_set SystemRoot
      _append_child_env_if_set COMSPEC
      _append_child_env_if_set ComSpec
      _append_child_env_if_set WINDIR
      _append_child_env_if_set SYSTEMDRIVE
      _append_child_env_if_set PATHEXT
      _append_child_env_if_set TEMP
      _append_child_env_if_set TMP
      _append_child_env_if_set MSYSTEM
      _append_child_env_if_set MSYS
      _append_child_env_if_set MSYS2_PATH_TYPE
      ;;
  esac

  capture_command "$prefix" \
    env -i "${CHILD_ENV[@]}" \
    "$CHARACTERIZATION_BASH" "${bash_options[@]}" \
    "$CHARACTERIZATION_CHILD_DRIVER" \
    "$child_script" "$state_request" "$state_output" "$@"

  CHILD_STATE_FILE="$state_output"
}

assert_child_state() {
  local expected_name="$1"
  local expected_state="$2"
  local expected_value="${3:-}"
  local expected_attribute="${4:-not-exported}"
  local name
  local state
  local value
  local attribute

  while IFS= read -r -d '' name &&
        IFS= read -r -d '' state &&
        IFS= read -r -d '' value &&
        IFS= read -r -d '' attribute; do
    if [[ "$name" == "$expected_name" ]]; then
      if [[ "$state" != "$expected_state" || "$value" != "$expected_value" || "$attribute" != "$expected_attribute" ]]; then
        fail "child state '$expected_name': expected $expected_state/$(printf '%q' "$expected_value")/$expected_attribute, got $state/$(printf '%q' "$value")/$attribute"
        return 1
      fi
      return 0
    fi
  done <"$CHILD_STATE_FILE"

  fail "child state did not contain '$expected_name'"
}
