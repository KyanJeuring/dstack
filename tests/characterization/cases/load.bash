#!/usr/bin/env bash

# Loading tests treat sourcing and public function availability as contract
# candidates. Direct argument dispatch remains compatibility-sensitive and
# undecided. Private helper checks are legacy Bash implementation checks.

_load_file_contains() {
  local file="$1"
  local text="$2"
  local label="${3:-$file}"

  if ! grep -Fq "$text" "$file"; then
    fail "$label: expected to contain $(printf '%q' "$text")"
    return 1
  fi
}

_load_assert_no_ansi() {
  local file="$1"

  if grep -q $'\033' "$file"; then
    fail "non-TTY output contains an ANSI escape byte: $file"
    return 1
  fi
}

_load_assert_no_docker_calls() {
  assert_call_count 0
}

_load_create_probe() {
  LOAD_PROBE="$CASE_PROJECTS/load-probe.bash"
  cat >"$LOAD_PROBE" <<'EOF'
characterization_child_main() {
  local mode="$1"
  local dstack_path="$2"
  shift 2

  case "$mode" in
    source)
      source "$dstack_path"
      ;;
    version-state)
      source "$dstack_path"
      if declare -p DSTACK_VERSION >/dev/null 2>&1; then
        LOAD_VERSION_DEFINED=yes
      else
        LOAD_VERSION_DEFINED=no
      fi
      if [[ -n "${DSTACK_VERSION:-}" ]]; then
        LOAD_VERSION_NONEMPTY=yes
      else
        LOAD_VERSION_NONEMPTY=no
      fi
      ;;
    version-output)
      local expected_file="$1"
      source "$dstack_path"
      printf 'DStack %s\n' "$DSTACK_VERSION" >"$expected_file"
      dversion
      ;;
    deprecated-array)
      local declaration
      source "$dstack_path"
      if declaration="$(declare -p DEPRECATED_FUNCTIONS 2>/dev/null)"; then
        LOAD_DEPRECATED_EXISTS=yes
      else
        LOAD_DEPRECATED_EXISTS=no
        declaration=""
      fi
      case "$declaration" in
        declare\ -a*) LOAD_DEPRECATED_KIND=indexed ;;
        *) LOAD_DEPRECATED_KIND=other ;;
      esac
      LOAD_DEPRECATED_COUNT="${#DEPRECATED_FUNCTIONS[@]}"
      ;;
    public-inventory)
      local output_file="$1"
      local function_name
      source "$dstack_path"
      : >"$output_file"
      LOAD_PUBLIC_COUNT=0
      while IFS= read -r function_name; do
        case "$function_name" in
          _*|characterization_child_main|log|info|ok|warn|err|confirm)
            continue
            ;;
        esac
        printf '%s\n' "$function_name" >>"$output_file"
        LOAD_PUBLIC_COUNT=$((LOAD_PUBLIC_COUNT + 1))
      done < <(compgen -A function | LC_ALL=C sort)
      ;;
    internal-inventory)
      local helper_name
      source "$dstack_path"
      LOAD_MISSING_INTERNAL=""
      for helper_name in "$@"; do
        if ! declare -F "$helper_name" >/dev/null; then
          LOAD_MISSING_INTERNAL+="${LOAD_MISSING_INTERNAL:+,}$helper_name"
        fi
      done
      ;;
    preserve-environment)
      source "$dstack_path"
      ;;
    option)
      local option_name="$1"
      local requested_state="$2"
      if [[ "$requested_state" == "on" ]]; then
        set -o "$option_name"
      else
        set +o "$option_name"
      fi
      if [[ -o "$option_name" ]]; then
        LOAD_OPTION_BEFORE=on
      else
        LOAD_OPTION_BEFORE=off
      fi
      source "$dstack_path"
      LOAD_SOURCE_STATUS=$?
      if [[ -o "$option_name" ]]; then
        LOAD_OPTION_AFTER=on
      else
        LOAD_OPTION_AFTER=off
      fi
      return "$LOAD_SOURCE_STATUS"
      ;;
    option-matrix)
      local requested_state="$1"
      if [[ "$requested_state" == "on" ]]; then
        set -e
        set -u
        set -o pipefail
      else
        set +e
        set +u
        set +o pipefail
      fi
      [[ -o errexit ]] && LOAD_ERREXIT_BEFORE=on || LOAD_ERREXIT_BEFORE=off
      [[ -o nounset ]] && LOAD_NOUNSET_BEFORE=on || LOAD_NOUNSET_BEFORE=off
      [[ -o pipefail ]] && LOAD_PIPEFAIL_BEFORE=on || LOAD_PIPEFAIL_BEFORE=off
      source "$dstack_path"
      LOAD_SOURCE_STATUS=$?
      [[ -o errexit ]] && LOAD_ERREXIT_AFTER=on || LOAD_ERREXIT_AFTER=off
      [[ -o nounset ]] && LOAD_NOUNSET_AFTER=on || LOAD_NOUNSET_AFTER=off
      [[ -o pipefail ]] && LOAD_PIPEFAIL_AFTER=on || LOAD_PIPEFAIL_AFTER=off
      return "$LOAD_SOURCE_STATUS"
      ;;
    ifs-default)
      LOAD_IFS_BEFORE="$IFS"
      source "$dstack_path"
      LOAD_SOURCE_STATUS=$?
      LOAD_IFS_AFTER="$IFS"
      return "$LOAD_SOURCE_STATUS"
      ;;
    ifs-custom)
      IFS="$1"
      LOAD_IFS_BEFORE="$IFS"
      source "$dstack_path"
      LOAD_SOURCE_STATUS=$?
      LOAD_IFS_AFTER="$IFS"
      return "$LOAD_SOURCE_STATUS"
      ;;
    shopt-source)
      shopt -s nullglob extglob
      shopt -u dotglob
      shopt -q nullglob && LOAD_NULLGLOB_BEFORE=on || LOAD_NULLGLOB_BEFORE=off
      shopt -q extglob && LOAD_EXTGLOB_BEFORE=on || LOAD_EXTGLOB_BEFORE=off
      shopt -q dotglob && LOAD_DOTGLOB_BEFORE=on || LOAD_DOTGLOB_BEFORE=off
      source "$dstack_path"
      LOAD_SOURCE_STATUS=$?
      shopt -q nullglob && LOAD_NULLGLOB_AFTER=on || LOAD_NULLGLOB_AFTER=off
      shopt -q extglob && LOAD_EXTGLOB_AFTER=on || LOAD_EXTGLOB_AFTER=off
      shopt -q dotglob && LOAD_DOTGLOB_AFTER=on || LOAD_DOTGLOB_AFTER=off
      return "$LOAD_SOURCE_STATUS"
      ;;
    dhelp-nullglob)
      local help_output="$1"
      shopt -s nullglob
      shopt -q nullglob && LOAD_NULLGLOB_BEFORE=on || LOAD_NULLGLOB_BEFORE=off
      source "$dstack_path"
      shopt -q nullglob && LOAD_NULLGLOB_AFTER_SOURCE=on || LOAD_NULLGLOB_AFTER_SOURCE=off
      dhelp >"$help_output"
      LOAD_DHELP_STATUS=$?
      shopt -q nullglob && LOAD_NULLGLOB_AFTER_DHELP=on || LOAD_NULLGLOB_AFTER_DHELP=off
      return "$LOAD_DHELP_STATUS"
      ;;
    dhelp)
      source "$dstack_path"
      dhelp
      ;;
    logger)
      source "$dstack_path"
      info "non-tty log sentinel"
      ;;
    bash-major)
      LOAD_BASH_MAJOR="${BASH_VERSINFO[0]}"
      if ((BASH_VERSINFO[0] >= 4)); then
        LOAD_BASH_SUPPORTED=yes
      else
        LOAD_BASH_SUPPORTED=no
      fi
      ;;
    *)
      printf 'unknown load probe mode: %s\n' "$mode" >&2
      return 64
      ;;
  esac
}
EOF
}

_load_run_probe() {
  local prefix="$1"
  local mode="$2"
  shift 2

  _load_create_probe
  run_child_bash "$prefix" -- "$LOAD_PROBE" \
    "$mode" "$CHARACTERIZATION_REPO_ROOT/dstack.sh" "$@"
}

_load_assert_silent_success() {
  local label="$1"

  assert_status 0 "$CAPTURE_STATUS" "$label status"
  assert_file_empty "$CAPTURE_STDOUT" "$label stdout"
  assert_file_empty "$CAPTURE_STDERR" "$label stderr"
  _load_assert_no_docker_calls
}

_load_selected_bash_is_supported() {
  local prefix="$CASE_CAPTURE_DIR/bash-major"

  set_child_state_vars LOAD_BASH_MAJOR LOAD_BASH_SUPPORTED
  _load_run_probe "$prefix" bash-major
  _load_assert_silent_success "selected Bash check"
  assert_child_state LOAD_BASH_SUPPORTED set yes not-exported
}

_load_source_succeeds() {
  local prefix="$CASE_CAPTURE_DIR/source"

  set_child_state_vars
  _load_run_probe "$prefix" source
  _load_assert_silent_success "source"
}

_load_source_sets_version() {
  local prefix="$CASE_CAPTURE_DIR/version-state"

  set_child_state_vars LOAD_VERSION_DEFINED LOAD_VERSION_NONEMPTY
  _load_run_probe "$prefix" version-state
  _load_assert_silent_success "source version state"
  assert_child_state LOAD_VERSION_DEFINED set yes not-exported
  assert_child_state LOAD_VERSION_NONEMPTY set yes not-exported
}

_load_sourced_version_matches_variable() {
  local prefix="$CASE_CAPTURE_DIR/version-output"
  local expected="$CASE_CAPTURE_DIR/version.expected"

  set_child_state_vars
  _load_run_probe "$prefix" version-output "$expected"
  assert_status 0 "$CAPTURE_STATUS" "sourced dversion status"
  assert_bytes_equal "$expected" "$CAPTURE_STDOUT" "dversion output derived from DSTACK_VERSION"
  assert_file_empty "$CAPTURE_STDERR" "sourced dversion stderr"
  _load_assert_no_docker_calls
}

_load_source_initializes_deprecated_array() {
  local prefix="$CASE_CAPTURE_DIR/deprecated-array"

  set_child_state_vars LOAD_DEPRECATED_EXISTS LOAD_DEPRECATED_KIND LOAD_DEPRECATED_COUNT
  _load_run_probe "$prefix" deprecated-array
  _load_assert_silent_success "deprecated array initialization"
  assert_child_state LOAD_DEPRECATED_EXISTS set yes not-exported
  assert_child_state LOAD_DEPRECATED_KIND set indexed not-exported
  assert_child_state LOAD_DEPRECATED_COUNT set 0 not-exported
}

_load_source_defines_public_inventory() {
  local prefix="$CASE_CAPTURE_DIR/public-inventory"
  local actual="$CASE_CAPTURE_DIR/public.actual"
  local expected="$CASE_CAPTURE_DIR/public.expected"
  local command_name
  local -a expected_public=(
    dclean
    dcleanall
    dcleani
    dcompose
    dconfig
    ddown
    ddownv
    dexec
    dhelp
    dimg
    dinspect
    dip
    dllog
    dllogs
    dlog
    dlogs
    dnet
    dnetinspect
    dport
    dprunewhat
    dps
    dpsa
    dpsg
    dpull
    drebootstack
    drebuild
    drebuildnocache
    drecompose
    drestart
    drun
    dstack
    dstackpurge
    dstackunset
    dstart
    dstats
    dstop
    dstopall
    dsvc
    dupdate
    dversion
    dvol
    dvolinspect
    dvolrm
  )

  : >"$expected"
  for command_name in "${expected_public[@]}"; do
    printf '%s\n' "$command_name" >>"$expected"
  done

  assert_status 43 "${#expected_public[@]}" "declared public inventory size"
  set_child_state_vars LOAD_PUBLIC_COUNT
  _load_run_probe "$prefix" public-inventory "$actual"
  _load_assert_silent_success "public function inventory"
  assert_child_state LOAD_PUBLIC_COUNT set 43 not-exported
  assert_bytes_equal "$expected" "$actual" "complete public function inventory"
}

_load_source_defines_required_internal_helpers() {
  local prefix="$CASE_CAPTURE_DIR/internal-inventory"
  local -a required_internal=(
    log
    _log_emit
    info
    ok
    warn
    err
    confirm
    _dstack_compose_files
    _dstack_find_compose_file
    _dstack_resolve_compose_files
    _dstack_resolve_compose_flags
    _dstack_bases
    _dstack_resolve
    _dcompose
  )

  set_child_state_vars LOAD_MISSING_INTERNAL
  _load_run_probe "$prefix" internal-inventory "${required_internal[@]}"
  _load_assert_silent_success "legacy internal helper inventory"
  assert_child_state LOAD_MISSING_INTERNAL set "" not-exported
}

_load_source_preserves_existing_dstack() {
  local prefix="$CASE_CAPTURE_DIR/existing-dstack"
  local sentinel="sentinel DSTACK value / not a project"

  DSTACK="$sentinel"
  export DSTACK
  set_child_state_vars DSTACK
  _load_run_probe "$prefix" preserve-environment
  _load_assert_silent_success "existing DSTACK source"
  assert_child_state DSTACK set "$sentinel" exported
}

_load_source_preserves_existing_selector() {
  local prefix="$CASE_CAPTURE_DIR/existing-selector"
  local sentinel="all,compose sentinel.yaml"

  DSTACK_COMPOSE_FILE="$sentinel"
  export DSTACK_COMPOSE_FILE
  set_child_state_vars DSTACK_COMPOSE_FILE
  _load_run_probe "$prefix" preserve-environment
  _load_assert_silent_success "existing DSTACK_COMPOSE_FILE source"
  assert_child_state DSTACK_COMPOSE_FILE set "$sentinel" exported
}

_load_direct_execution_no_args() {
  local prefix="$CASE_CAPTURE_DIR/direct-no-args"

  capture_command "$prefix" "$CHARACTERIZATION_REPO_ROOT/dstack.sh"
  assert_status 0 "$CAPTURE_STATUS" "direct execution without arguments"
  assert_file_empty "$CAPTURE_STDOUT" "direct execution stdout"
  assert_file_empty "$CAPTURE_STDERR" "direct execution stderr"
  _load_assert_no_docker_calls
}

_load_direct_execution_dversion() {
  local prefix="$CASE_CAPTURE_DIR/direct-dversion"

  capture_command "$prefix" "$CHARACTERIZATION_REPO_ROOT/dstack.sh" dversion
  assert_status 0 "$CAPTURE_STATUS" "direct execution dversion"
  assert_file_empty "$CAPTURE_STDOUT" "direct dversion stdout"
  assert_file_empty "$CAPTURE_STDERR" "direct dversion stderr"
  _load_assert_no_docker_calls
}

_load_assert_option_trial() {
  local option_name="$1"
  local requested_state="$2"
  local suffix="$3"
  local prefix="$CASE_CAPTURE_DIR/${option_name}-${suffix}"

  set_child_state_vars LOAD_OPTION_BEFORE LOAD_OPTION_AFTER LOAD_SOURCE_STATUS
  _load_run_probe "$prefix" option "$option_name" "$requested_state"
  _load_assert_silent_success "$option_name $requested_state source"
  assert_child_state LOAD_OPTION_BEFORE set "$requested_state" not-exported
  assert_child_state LOAD_OPTION_AFTER set "$requested_state" not-exported
  assert_child_state LOAD_SOURCE_STATUS set 0 not-exported
}

_load_source_preserves_errexit() {
  _load_assert_option_trial errexit on enabled
  _load_assert_option_trial errexit off disabled
}

_load_source_preserves_nounset() {
  _load_assert_option_trial nounset on enabled
  _load_assert_option_trial nounset off disabled
}

_load_source_preserves_pipefail() {
  _load_assert_option_trial pipefail on enabled
  _load_assert_option_trial pipefail off disabled
}

_load_assert_option_matrix_trial() {
  local requested_state="$1"
  local prefix="$CASE_CAPTURE_DIR/options-${requested_state}"

  set_child_state_vars \
    LOAD_ERREXIT_BEFORE LOAD_ERREXIT_AFTER \
    LOAD_NOUNSET_BEFORE LOAD_NOUNSET_AFTER \
    LOAD_PIPEFAIL_BEFORE LOAD_PIPEFAIL_AFTER \
    LOAD_SOURCE_STATUS
  _load_run_probe "$prefix" option-matrix "$requested_state"
  _load_assert_silent_success "strict option matrix $requested_state"
  assert_child_state LOAD_ERREXIT_BEFORE set "$requested_state" not-exported
  assert_child_state LOAD_ERREXIT_AFTER set "$requested_state" not-exported
  assert_child_state LOAD_NOUNSET_BEFORE set "$requested_state" not-exported
  assert_child_state LOAD_NOUNSET_AFTER set "$requested_state" not-exported
  assert_child_state LOAD_PIPEFAIL_BEFORE set "$requested_state" not-exported
  assert_child_state LOAD_PIPEFAIL_AFTER set "$requested_state" not-exported
  assert_child_state LOAD_SOURCE_STATUS set 0 not-exported
}

_load_source_preserves_option_matrix() {
  _load_assert_option_matrix_trial on
  _load_assert_option_matrix_trial off
}

_load_source_preserves_default_ifs() {
  local prefix="$CASE_CAPTURE_DIR/default-ifs"
  local expected_ifs=$' \t\n'

  set_child_state_vars LOAD_IFS_BEFORE LOAD_IFS_AFTER LOAD_SOURCE_STATUS
  _load_run_probe "$prefix" ifs-default
  _load_assert_silent_success "default IFS source"
  assert_child_state LOAD_IFS_BEFORE set "$expected_ifs" not-exported
  assert_child_state LOAD_IFS_AFTER set "$expected_ifs" not-exported
  assert_child_state LOAD_SOURCE_STATUS set 0 not-exported
}

_load_source_preserves_custom_ifs() {
  local prefix="$CASE_CAPTURE_DIR/custom-ifs"
  local custom_ifs=$':, \t\n;'

  set_child_state_vars LOAD_IFS_BEFORE LOAD_IFS_AFTER LOAD_SOURCE_STATUS
  _load_run_probe "$prefix" ifs-custom "$custom_ifs"
  _load_assert_silent_success "custom IFS source"
  assert_child_state LOAD_IFS_BEFORE set "$custom_ifs" not-exported
  assert_child_state LOAD_IFS_AFTER set "$custom_ifs" not-exported
  assert_child_state LOAD_SOURCE_STATUS set 0 not-exported
}

_load_source_preserves_unrelated_shopt() {
  local prefix="$CASE_CAPTURE_DIR/source-shopt"

  set_child_state_vars \
    LOAD_NULLGLOB_BEFORE LOAD_NULLGLOB_AFTER \
    LOAD_EXTGLOB_BEFORE LOAD_EXTGLOB_AFTER \
    LOAD_DOTGLOB_BEFORE LOAD_DOTGLOB_AFTER \
    LOAD_SOURCE_STATUS
  _load_run_probe "$prefix" shopt-source
  _load_assert_silent_success "source shopt state"
  assert_child_state LOAD_NULLGLOB_BEFORE set on not-exported
  assert_child_state LOAD_NULLGLOB_AFTER set on not-exported
  assert_child_state LOAD_EXTGLOB_BEFORE set on not-exported
  assert_child_state LOAD_EXTGLOB_AFTER set on not-exported
  assert_child_state LOAD_DOTGLOB_BEFORE set off not-exported
  assert_child_state LOAD_DOTGLOB_AFTER set off not-exported
  assert_child_state LOAD_SOURCE_STATUS set 0 not-exported
}

_load_dhelp_disables_preexisting_nullglob() {
  local prefix="$CASE_CAPTURE_DIR/dhelp-nullglob"
  local help_output="$CASE_CAPTURE_DIR/dhelp-nullglob.output"

  set_child_state_vars \
    LOAD_NULLGLOB_BEFORE LOAD_NULLGLOB_AFTER_SOURCE \
    LOAD_NULLGLOB_AFTER_DHELP LOAD_DHELP_STATUS
  _load_run_probe "$prefix" dhelp-nullglob "$help_output"
  _load_assert_silent_success "dhelp nullglob probe"
  assert_child_state LOAD_NULLGLOB_BEFORE set on not-exported
  assert_child_state LOAD_NULLGLOB_AFTER_SOURCE set on not-exported
  assert_child_state LOAD_NULLGLOB_AFTER_DHELP set off not-exported
  assert_child_state LOAD_DHELP_STATUS set 0 not-exported
  if [[ ! -s "$help_output" ]]; then
    fail "dhelp nullglob probe produced no help output"
  fi
}

_load_dhelp_smoke() {
  local prefix="$CASE_CAPTURE_DIR/dhelp"

  set_child_state_vars
  _load_run_probe "$prefix" dhelp
  assert_status 0 "$CAPTURE_STATUS" "dhelp status"
  if [[ ! -s "$CAPTURE_STDOUT" ]]; then
    fail "dhelp produced no stdout"
  fi
  assert_file_empty "$CAPTURE_STDERR" "dhelp stderr"
  _load_file_contains "$CAPTURE_STDOUT" "DStack commands" "dhelp heading"
  _load_file_contains "$CAPTURE_STDOUT" "dcompose" "dhelp dcompose entry"
  _load_file_contains "$CAPTURE_STDOUT" "dstack" "dhelp dstack entry"
  _load_file_contains "$CAPTURE_STDOUT" "dversion" "dhelp dversion entry"
  _load_assert_no_docker_calls
}

_load_non_tty_logging_has_no_ansi() {
  local prefix="$CASE_CAPTURE_DIR/non-tty-logger"

  set_child_state_vars
  _load_run_probe "$prefix" logger
  assert_status 0 "$CAPTURE_STATUS" "non-TTY logger status"
  assert_text_file "$CAPTURE_STDOUT" $'[INFO] non-tty log sentinel\n' "non-TTY logger stdout"
  assert_file_empty "$CAPTURE_STDERR" "non-TTY logger stderr"
  _load_assert_no_ansi "$CAPTURE_STDOUT"
  _load_assert_no_docker_calls
}

# Supported-platform precondition; Bash 3 execution remains deferred.
register_case load load::selected_bash_major_is_at_least_4 _load_selected_bash_is_supported

# Intentional contract candidates.
register_case load load::source_succeeds _load_source_succeeds
register_case load load::source_sets_version _load_source_sets_version
register_case load load::sourced_dversion_matches_version_variable _load_sourced_version_matches_variable
register_case load load::source_initializes_empty_deprecated_array _load_source_initializes_deprecated_array
register_case load load::source_defines_public_command_inventory _load_source_defines_public_inventory
register_case load load::source_preserves_existing_dstack _load_source_preserves_existing_dstack
register_case load load::source_preserves_existing_compose_selector _load_source_preserves_existing_selector
register_case load load::source_preserves_errexit _load_source_preserves_errexit
register_case load load::source_preserves_nounset _load_source_preserves_nounset
register_case load load::source_preserves_pipefail _load_source_preserves_pipefail
register_case load load::source_preserves_strict_option_matrix _load_source_preserves_option_matrix
register_case load load::source_preserves_default_ifs _load_source_preserves_default_ifs
register_case load load::source_preserves_custom_ifs _load_source_preserves_custom_ifs
register_case load load::source_preserves_unrelated_shopt_state _load_source_preserves_unrelated_shopt
register_case load load::dhelp_runs_and_lists_representative_commands _load_dhelp_smoke
register_case load load::non_tty_logging_has_no_ansi _load_non_tty_logging_has_no_ansi

# Legacy Bash implementation check.
register_case load load::source_defines_required_internal_helpers _load_source_defines_required_internal_helpers

# Compatibility-sensitive / undecided observations.
register_case load load::direct_execution_no_args_is_silent_success _load_direct_execution_no_args
register_case load load::direct_execution_dversion_is_silent_success _load_direct_execution_dversion

# Possible bug/quirk: dhelp does not restore an enabled nullglob setting.
register_case load load::dhelp_disables_preexisting_nullglob _load_dhelp_disables_preexisting_nullglob
