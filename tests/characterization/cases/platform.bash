#!/usr/bin/env bash

# These cases record host utility and path behavior on the three CI platforms.
# They are legacy Bash/platform checks rather than cross-implementation
# contracts. Platform capability checks are explicit where the current runtime
# delegates behavior to an external utility.

_platform_file_contains() {
  local file="$1"
  local text="$2"
  local label="$3"

  if ! grep -Fq -- "$text" "$file"; then
    fail "$label: expected file to contain $(printf '%q' "$text")"
    return 1
  fi
}

_platform_file_excludes() {
  local file="$1"
  local text="$2"
  local label="$3"

  if grep -Fq -- "$text" "$file"; then
    fail "$label: expected file not to contain $(printf '%q' "$text")"
    return 1
  fi
}

_platform_runtime_utilities_resolve() {
  local utility
  local -a utilities=(
    awk basename cat column dirname find grep less mkdir mv sed touch whoami xargs
  )

  for utility in "${utilities[@]}"; do
    if ! command -v "$utility" >/dev/null 2>&1; then
      fail "runtime utility is unavailable on ${OSTYPE:-unknown}: $utility"
    fi
  done
}

_platform_dirname_accepts_double_dash() {
  local output="$CASE_CAPTURE_DIR/dirname.stdout"
  local errors="$CASE_CAPTURE_DIR/dirname.stderr"
  local status
  local path="$CASE_PROJECTS/-leading-name/file"

  mkdir -p "${path%/*}"
  if dirname -- "$path" >"$output" 2>"$errors"; then
    status=0
  else
    status=$?
  fi

  assert_status 0 "$status" "dirname -- status"
  assert_text_file "$output" "${path%/*}"$'\n' "dirname -- output"
  assert_file_empty "$errors" "dirname -- stderr"
}

_platform_column_accepts_dstack_flags() {
  local output="$CASE_CAPTURE_DIR/column.stdout"
  local errors="$CASE_CAPTURE_DIR/column.stderr"
  local status

  if printf 'NAME\tSTATUS\nweb\trunning\n' |
      column -t -s $'\t' >"$output" 2>"$errors"; then
    status=0
  else
    status=$?
  fi

  assert_status 0 "$status" "column flags status"
  _platform_file_contains "$output" "NAME" "column header"
  _platform_file_contains "$output" "web" "column row"
  assert_file_empty "$errors" "column stderr"
}

_platform_discovery_follows_host_find_depth_support() {
  local base="$CASE_BASES/discovery"
  local find_supports_depth=0

  mkdir -p \
    "$base/one" \
    "$base/group/two" \
    "$base/group/nested/three"
  : >"$base/one/compose.yml"
  : >"$base/group/two/compose.yml"
  : >"$base/group/nested/three/compose.yml"

  if find "$base" -maxdepth 2 -mindepth 1 -type d \
      >"$CASE_CAPTURE_DIR/find-depth.stdout" \
      2>"$CASE_CAPTURE_DIR/find-depth.stderr"; then
    find_supports_depth=1
  fi

  DSTACK_BASES="$base"
  export DSTACK_BASES
  set_child_state_vars
  run_dstack_function "$CASE_CAPTURE_DIR/discovery" dstack ls

  assert_status 0 "$CAPTURE_STATUS" "dstack discovery status"
  assert_file_empty "$CAPTURE_STDERR" "dstack discovery stderr"
  if ((find_supports_depth)); then
    _platform_file_contains "$CAPTURE_STDOUT" "one" "depth-one discovery"
    _platform_file_contains "$CAPTURE_STDOUT" "group/two" "depth-two discovery"
  else
    _platform_file_excludes "$CAPTURE_STDOUT" "$base/one" \
      "discovery when find depth flags are unsupported"
    _platform_file_excludes "$CAPTURE_STDOUT" "$base/group/two" \
      "nested discovery when find depth flags are unsupported"
  fi
  _platform_file_excludes "$CAPTURE_STDOUT" "$base/group/nested/three" \
    "depth-three discovery"
  assert_call_count 0
}

_platform_xargs_empty_input_is_platform_specific() {
  _status_configure_response 1 0 ""
  set_child_state_vars
  run_dstack_function "$CASE_CAPTURE_DIR/xargs-empty" dstopall

  assert_status 0 "$CAPTURE_STATUS" "empty dstopall status"
  assert_file_empty "$CAPTURE_STDOUT" "empty dstopall stdout"
  assert_file_empty "$CAPTURE_STDERR" "empty dstopall stderr"
  assert_argv 1 ps -q
  case "${OSTYPE:-}" in
    darwin*)
      # macOS xargs does not run the command when its input is empty.
      assert_call_count 1
      ;;
    *)
      # GNU xargs, including Git Bash's build, runs it once with no arguments.
      assert_call_count 2
      assert_argv 2 stop
      ;;
  esac
}

_platform_fake_docker_records_physical_space_cwd() {
  local directory="$CASE_PROJECTS/current directory with spaces"
  local physical

  mkdir -p "$directory"
  fixture_set_cwd "$directory"
  physical="$(pwd -P)"
  set_child_state_vars
  run_dstack_function "$CASE_CAPTURE_DIR/physical-cwd" dport

  assert_status 0 "$CAPTURE_STATUS" "physical cwd helper status"
  assert_call_count 1
  assert_argv 1 ps --format 'table {{.Names}}\t{{.Ports}}'
  assert_recorded_cwd 1 "$physical"
}

_platform_git_bash_uses_posix_style_case_root() {
  case "${OSTYPE:-}" in
    msys*|cygwin*)
      if [[ ! "$CASE_ROOT" =~ ^/[[:alpha:]]/ ]]; then
        fail "Git Bash case root is not a POSIX-style drive path: $CASE_ROOT"
      fi
      ;;
    *)
      if [[ "$CASE_ROOT" != /* ]]; then
        fail "Unix case root is not absolute: $CASE_ROOT"
      fi
      ;;
  esac
}

register_case platform platform::runtime_external_utilities_resolve _platform_runtime_utilities_resolve
register_case platform platform::dirname_accepts_double_dash _platform_dirname_accepts_double_dash
register_case platform platform::column_accepts_current_format_flags _platform_column_accepts_dstack_flags
register_case platform platform::auto_discovery_reflects_host_find_depth_support _platform_discovery_follows_host_find_depth_support
register_case platform platform::empty_xargs_input_behavior_is_platform_specific _platform_xargs_empty_input_is_platform_specific
register_case platform platform::fake_docker_records_physical_space_cwd _platform_fake_docker_records_physical_space_cwd
register_case platform platform::git_bash_uses_posix_style_case_root_when_applicable _platform_git_bash_uses_posix_style_case_root
