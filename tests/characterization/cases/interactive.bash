#!/usr/bin/env bash

# Linux-only PTY cases. They characterize terminal-sensitive behavior while
# asserting semantic output and fake-Docker records instead of full terminal
# transcripts. Exact ANSI bytes are limited to the focused logger case.

_interactive_prepare_probe() {
  INTERACTIVE_PROBE="$CASE_PROJECTS/interactive-probe.bash"
  cat >"$INTERACTIVE_PROBE" <<'EOF'
#!/usr/bin/env bash

dstack_path="$1"
working_directory="$2"
function_name="$3"
shift 3

cd "$working_directory" || exit 125
source "$dstack_path"
if ! declare -F "$function_name" >/dev/null; then
  printf 'DStack function is not defined: %s\n' "$function_name" >&2
  exit 127
fi
"$function_name" "$@"
EOF
}

_interactive_run() {
  local name="$1"
  local wait_for="$2"
  local input="$3"
  local function_name="$4"
  shift 4
  local prefix="$CASE_CAPTURE_DIR/$name"
  local input_file="$prefix.input"
  local driver_stdout="$prefix.driver.stdout"
  local driver_stderr="$prefix.driver.stderr"
  local driver_status

  _interactive_prepare_probe
  printf '%s' "$input" >"$input_file"
  if python3 "$CHARACTERIZATION_ROOT/interactive/pty-driver.py" \
      --transcript "$prefix.transcript" \
      --status-file "$prefix.status" \
      --input-file "$input_file" \
      --wait-for "$wait_for" \
      --timeout 5 \
      -- "$CHARACTERIZATION_BASH" --noprofile --norc "$INTERACTIVE_PROBE" \
      "$CHARACTERIZATION_REPO_ROOT/dstack.sh" "$(pwd -P)" \
      "$function_name" "$@" \
      >"$driver_stdout" 2>"$driver_stderr"; then
    driver_status=0
  else
    driver_status=$?
  fi

  assert_status 0 "$driver_status" "PTY driver status"
  assert_file_empty "$driver_stdout" "PTY driver stdout"
  assert_file_empty "$driver_stderr" "PTY driver stderr"
  PTY_TRANSCRIPT="$prefix.transcript"
  IFS= read -r PTY_CHILD_STATUS <"$prefix.status" || PTY_CHILD_STATUS=""
}

_interactive_run_eof() {
  local name="$1"
  local wait_for="$2"
  local function_name="$3"
  shift 3
  local prefix="$CASE_CAPTURE_DIR/$name"
  local driver_stdout="$prefix.driver.stdout"
  local driver_stderr="$prefix.driver.stderr"
  local driver_status

  _interactive_prepare_probe
  if python3 "$CHARACTERIZATION_ROOT/interactive/pty-driver.py" \
      --transcript "$prefix.transcript" \
      --status-file "$prefix.status" \
      --wait-for "$wait_for" \
      --send-eof \
      --timeout 5 \
      -- "$CHARACTERIZATION_BASH" --noprofile --norc "$INTERACTIVE_PROBE" \
      "$CHARACTERIZATION_REPO_ROOT/dstack.sh" "$(pwd -P)" \
      "$function_name" "$@" \
      >"$driver_stdout" 2>"$driver_stderr"; then
    driver_status=0
  else
    driver_status=$?
  fi

  assert_status 0 "$driver_status" "PTY EOF driver status"
  assert_file_empty "$driver_stdout" "PTY EOF driver stdout"
  assert_file_empty "$driver_stderr" "PTY EOF driver stderr"
  PTY_TRANSCRIPT="$prefix.transcript"
  IFS= read -r PTY_CHILD_STATUS <"$prefix.status" || PTY_CHILD_STATUS=""
}

_interactive_run_without_input() {
  local name="$1"
  local function_name="$2"
  shift 2
  local prefix="$CASE_CAPTURE_DIR/$name"
  local driver_status

  _interactive_prepare_probe
  if python3 "$CHARACTERIZATION_ROOT/interactive/pty-driver.py" \
      --transcript "$prefix.transcript" \
      --status-file "$prefix.status" \
      --timeout 5 \
      -- "$CHARACTERIZATION_BASH" --noprofile --norc "$INTERACTIVE_PROBE" \
      "$CHARACTERIZATION_REPO_ROOT/dstack.sh" "$(pwd -P)" \
      "$function_name" "$@" \
      >"$prefix.driver.stdout" 2>"$prefix.driver.stderr"; then
    driver_status=0
  else
    driver_status=$?
  fi

  assert_status 0 "$driver_status" "PTY driver status"
  assert_file_empty "$prefix.driver.stdout" "PTY driver stdout"
  assert_file_empty "$prefix.driver.stderr" "PTY driver stderr"
  PTY_TRANSCRIPT="$prefix.transcript"
  IFS= read -r PTY_CHILD_STATUS <"$prefix.status" || PTY_CHILD_STATUS=""
}

_interactive_file_contains() {
  local file="$1"
  local text="$2"
  local label="$3"

  if ! grep -Fq -- "$text" "$file"; then
    fail "$label: expected transcript to contain $(printf '%q' "$text")"
    return 1
  fi
}

_interactive_file_excludes() {
  local file="$1"
  local text="$2"
  local label="$3"

  if grep -Fq -- "$text" "$file"; then
    fail "$label: expected transcript not to contain $(printf '%q' "$text")"
    return 1
  fi
}

_interactive_prepare_multi_file_project() {
  fixture_write_file projects/current/docker-compose.yml 'services: {}'
  fixture_write_file projects/current/compose.yaml 'services: {}'
  INTERACTIVE_FIRST="$CASE_CURRENT_DIR/docker-compose.yml"
  INTERACTIVE_SECOND="$CASE_CURRENT_DIR/compose.yaml"
}

_interactive_numeric_selection_shows_menu_and_selects_first() {
  _interactive_prepare_multi_file_project
  _interactive_run numeric "Select compose file(s): " $'1\n' dstart

  assert_status 0 "$PTY_CHILD_STATUS" "numeric selection child status"
  _interactive_file_contains "$PTY_TRANSCRIPT" "Multiple compose files found:" \
    "selection menu heading"
  _interactive_file_contains "$PTY_TRANSCRIPT" "1) docker-compose.yml" \
    "first menu entry"
  _interactive_file_contains "$PTY_TRANSCRIPT" "2) compose.yaml" \
    "second menu entry"
  _interactive_file_contains "$PTY_TRANSCRIPT" "3) all" "all menu entry"
  _interactive_file_contains "$PTY_TRANSCRIPT" "Select compose file(s): " \
    "selection prompt"
  assert_call_count 1
  assert_argv 1 compose -f "$INTERACTIVE_FIRST" start
}

_interactive_comma_selection_selects_exact_files() {
  _interactive_prepare_multi_file_project
  _interactive_run comma "Select compose file(s): " $'1,2\n' dstart

  assert_status 0 "$PTY_CHILD_STATUS" "comma selection child status"
  assert_call_count 1
  assert_argv 1 compose -f "$INTERACTIVE_FIRST" -f "$INTERACTIVE_SECOND" start
}

_interactive_all_selection_selects_every_file() {
  _interactive_prepare_multi_file_project
  _interactive_run all "Select compose file(s): " $'all\n' dstart

  assert_status 0 "$PTY_CHILD_STATUS" "all selection child status"
  assert_call_count 1
  assert_argv 1 compose -f "$INTERACTIVE_FIRST" -f "$INTERACTIVE_SECOND" start
}

_interactive_invalid_selection_becomes_file_flag() {
  _interactive_prepare_multi_file_project
  _interactive_run invalid "Select compose file(s): " $'9\n' dstart

  assert_status 0 "$PTY_CHILD_STATUS" "invalid selection child status"
  _interactive_file_excludes "$PTY_TRANSCRIPT" "[ERROR] Compose file not found: 9" \
    "captured invalid selection error"
  assert_call_count 1
  assert_argv 1 compose -f '[ERROR] Compose file not found: 9' start
}

_interactive_empty_selection_becomes_file_flag() {
  _interactive_prepare_multi_file_project
  _interactive_run empty "Select compose file(s): " $'\n' dstart

  assert_status 0 "$PTY_CHILD_STATUS" "empty selection child status"
  _interactive_file_excludes "$PTY_TRANSCRIPT" "[ERROR] Invalid selection" \
    "captured empty selection error"
  assert_call_count 1
  assert_argv 1 compose -f '[ERROR] Invalid selection' start
}

_interactive_eof_selection_becomes_file_flag() {
  _interactive_prepare_multi_file_project
  _interactive_run_eof selection-eof "Select compose file(s): " dstart

  assert_status 0 "$PTY_CHILD_STATUS" "EOF selection child status"
  _interactive_file_excludes "$PTY_TRANSCRIPT" "[ERROR] Invalid selection" \
    "captured EOF selection error"
  assert_call_count 1
  assert_argv 1 compose -f '[ERROR] Invalid selection' start
}

_interactive_prepare_destructive_project() {
  fixture_write_file projects/current/compose.yml 'services: {}'
}

_interactive_confirmation_accepts_lowercase_y() {
  _interactive_prepare_destructive_project
  _interactive_run confirm-y "Continue? [y/n]: " $'y\n' ddownv

  assert_status 0 "$PTY_CHILD_STATUS" "lowercase confirmation status"
  _interactive_file_contains "$PTY_TRANSCRIPT" "[WARN]" "confirmation warning"
  _interactive_file_contains "$PTY_TRANSCRIPT" "Continue? [y/n]: " \
    "confirmation prompt"
  assert_call_count 1
  assert_argv 1 compose -f "$CASE_CURRENT_DIR/compose.yml" down -v
}

_interactive_confirmation_accepts_uppercase_y() {
  _interactive_prepare_destructive_project
  _interactive_run confirm-uppercase "Continue? [y/n]: " $'Y\n' ddownv

  assert_status 0 "$PTY_CHILD_STATUS" "uppercase confirmation status"
  assert_call_count 1
  assert_argv 1 compose -f "$CASE_CURRENT_DIR/compose.yml" down -v
}

_interactive_confirmation_rejection_cancels() {
  _interactive_prepare_destructive_project
  _interactive_run confirm-no "Continue? [y/n]: " $'n\n' ddownv

  assert_status 0 "$PTY_CHILD_STATUS" "rejected confirmation status"
  assert_call_count 0
}

_interactive_confirmation_eof_cancels() {
  _interactive_prepare_destructive_project
  _interactive_run_eof confirm-eof "Continue? [y/n]: " ddownv

  assert_status 0 "$PTY_CHILD_STATUS" "EOF confirmation status"
  assert_call_count 0
}

_interactive_tty_logging_uses_ansi_color() {
  local expected_prefix=$'\033[0;34m\033[1m[INFO]\033[0m terminal message'

  _interactive_run_without_input color info 'terminal message'
  assert_status 0 "$PTY_CHILD_STATUS" "TTY logger status"
  _interactive_file_contains "$PTY_TRANSCRIPT" "$expected_prefix" \
    "TTY ANSI logger output"
  assert_call_count 0
}

register_case interactive interactive::numeric_selection_shows_menu_and_selects_first _interactive_numeric_selection_shows_menu_and_selects_first
register_case interactive interactive::comma_selection_selects_exact_files _interactive_comma_selection_selects_exact_files
register_case interactive interactive::all_selection_selects_every_file _interactive_all_selection_selects_every_file
register_case interactive interactive::invalid_selection_error_becomes_file_flag _interactive_invalid_selection_becomes_file_flag
register_case interactive interactive::empty_selection_error_becomes_file_flag _interactive_empty_selection_becomes_file_flag
register_case interactive interactive::selection_eof_error_becomes_file_flag _interactive_eof_selection_becomes_file_flag
register_case interactive interactive::confirmation_accepts_lowercase_y _interactive_confirmation_accepts_lowercase_y
register_case interactive interactive::confirmation_accepts_uppercase_y _interactive_confirmation_accepts_uppercase_y
register_case interactive interactive::confirmation_rejection_cancels _interactive_confirmation_rejection_cancels
register_case interactive interactive::confirmation_eof_cancels _interactive_confirmation_eof_cancels
register_case interactive interactive::tty_logging_uses_ansi_color _interactive_tty_logging_uses_ansi_color
