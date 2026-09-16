#!/usr/bin/env bash

child_script="$1"
state_request="$2"
state_output="$3"
shift 3

# The characterization harness selects the per-case probe script at runtime,
# so there is no fixed source path for ShellCheck to follow.
# shellcheck disable=SC1090
source "$child_script"

if ! declare -F characterization_child_main >/dev/null; then
  printf 'child script must define characterization_child_main\n' >&2
  exit 127
fi

characterization_child_main "$@"
child_status=$?

: >"$state_output"
while IFS= read -r variable_name || [[ -n "$variable_name" ]]; do
  [[ -n "$variable_name" ]] || continue
  if [[ ! "$variable_name" =~ ^[a-zA-Z_][a-zA-Z0-9_]*$ ]]; then
    printf 'invalid state variable name: %s\n' "$variable_name" >&2
    exit 126
  fi

  if declare -p "$variable_name" >/dev/null 2>&1; then
    declaration="$(declare -p "$variable_name")"
    attribute="not-exported"
    case "$declaration" in
      declare\ -x*) attribute="exported" ;;
    esac
    printf '%s\0set\0%s\0%s\0' "$variable_name" "${!variable_name}" "$attribute" >>"$state_output"
  else
    printf '%s\0unset\0\0not-exported\0' "$variable_name" >>"$state_output"
  fi
done <"$state_request"

exit "$child_status"
