#!/usr/bin/env bash

write_nul_arguments() {
  local output="$1"
  shift
  : >"$output"
  if (($#)); then
    printf '%s\0' "$@" >"$output"
  fi
}

capture_command() {
  local prefix="$1"
  shift
  local directory="${prefix%/*}"

  if [[ "$directory" == "$prefix" ]]; then
    directory="."
  fi
  mkdir -p "$directory"

  CAPTURE_STDOUT="${prefix}.stdout"
  CAPTURE_STDERR="${prefix}.stderr"
  CAPTURE_STATUS_FILE="${prefix}.status"

  if "$@" >"$CAPTURE_STDOUT" 2>"$CAPTURE_STDERR"; then
    CAPTURE_STATUS=0
  else
    CAPTURE_STATUS=$?
  fi

  printf '%s\n' "$CAPTURE_STATUS" >"$CAPTURE_STATUS_FILE"
  return 0
}
