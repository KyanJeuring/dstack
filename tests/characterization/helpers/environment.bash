#!/usr/bin/env bash

_physical_file_path() {
  local path="$1"
  local directory="${path%/*}"
  local name="${path##*/}"

  if [[ "$directory" == "$path" ]]; then
    directory="."
  fi

  (cd "$directory" 2>/dev/null && printf '%s/%s\n' "$(pwd -P)" "$name")
}

create_case_root() {
  local suite_root="$1"

  CASE_ROOT="$(mktemp -d "$suite_root/case.XXXXXX")" || return 1
  CASE_HOME="$CASE_ROOT/home"
  CASE_BASES="$CASE_ROOT/bases"
  CASE_PROJECTS="$CASE_ROOT/projects"
  CASE_FAKE_BIN="$CASE_ROOT/fake-bin"
  CASE_DOCKER_DENY_BIN="$CASE_ROOT/docker-deny-bin"
  CASE_DOCKER_CONTROL="$CASE_ROOT/docker-control"
  CASE_CAPTURE_DIR="$CASE_ROOT/capture"
  CASE_TMP="$CASE_ROOT/tmp"
  CASE_DOCKER_CONFIG="$CASE_ROOT/docker-config"
  CASE_CURRENT_DIR="$CASE_PROJECTS/current"
  CASE_REGISTRY="$CASE_HOME/.config/dstack/registry"

  mkdir -p \
    "$CASE_HOME" \
    "$CASE_BASES" \
    "$CASE_CURRENT_DIR" \
    "$CASE_FAKE_BIN" \
    "$CASE_DOCKER_DENY_BIN" \
    "$CASE_DOCKER_CONTROL/calls" \
    "$CASE_DOCKER_CONTROL/responses" \
    "$CASE_CAPTURE_DIR" \
    "$CASE_TMP" \
    "$CASE_DOCKER_CONFIG"

  : >"$CASE_ROOT/.dstack-characterization-case"
}

cleanup_case_root() {
  local case_root="$1"
  local suite_root="$2"

  case "$case_root" in
    "$suite_root"/case.*) ;;
    *)
      printf 'refusing to remove case path outside suite root: %s\n' "$case_root" >&2
      return 1
      ;;
  esac

  if [[ ! -f "$case_root/.dstack-characterization-case" ]]; then
    printf 'refusing to remove case path without sentinel: %s\n' "$case_root" >&2
    return 1
  fi

  rm -rf -- "$case_root"
}

cleanup_suite_root() {
  local suite_root="$1"
  local temp_base="$2"

  case "$suite_root" in
    "$temp_base"/dstack-characterization.*) ;;
    *)
      printf 'refusing to remove suite path outside test temp base: %s\n' "$suite_root" >&2
      return 1
      ;;
  esac

  if [[ ! -f "$suite_root/.dstack-characterization-suite" ]]; then
    printf 'refusing to remove suite path without sentinel: %s\n' "$suite_root" >&2
    return 1
  fi

  rm -rf -- "$suite_root"
}

require_fake_docker() {
  local expected="${FAKE_DOCKER_PATH:-}"
  local resolved
  local expected_physical
  local resolved_physical

  if [[ -z "$expected" || ! -x "$expected" ]]; then
    printf 'fake Docker is missing or is not executable: %s\n' "${expected:-<unset>}" >&2
    return 1
  fi

  hash -r
  resolved="$(type -P docker 2>/dev/null)" || {
    printf 'docker does not resolve through the controlled PATH\n' >&2
    return 1
  }

  expected_physical="$(_physical_file_path "$expected")" || return 1
  resolved_physical="$(_physical_file_path "$resolved")" || return 1

  if [[ "$resolved_physical" != "$expected_physical" ]]; then
    printf 'unsafe docker resolution: expected %s, got %s\n' "$expected_physical" "$resolved_physical" >&2
    return 1
  fi
}

invoke_fake_docker_canary() {
  local expected="$CASE_CAPTURE_DIR/canary.expected.nul"
  local call_dir="$DSTACK_TEST_DOCKER_CONTROL/calls/000001"
  local status

  require_fake_docker || return 1
  reset_fake_docker_records || return 1

  if docker __dstack_characterization_canary__ "$DSTACK_TEST_DOCKER_GUARD" \
      >"$CASE_CAPTURE_DIR/canary.stdout" \
      2>"$CASE_CAPTURE_DIR/canary.stderr"; then
    status=0
  else
    status=$?
  fi

  if ((status != 0)); then
    printf 'fake-Docker canary returned status %s\n' "$status" >&2
    return 1
  fi

  write_nul_arguments "$expected" \
    __dstack_characterization_canary__ "$DSTACK_TEST_DOCKER_GUARD"

  if [[ ! -f "$call_dir/complete" ]] || ! cmp -s "$expected" "$call_dir/argv.nul"; then
    printf 'fake-Docker canary did not create the expected record\n' >&2
    return 1
  fi
}

prepare_case_environment() {
  local guard

  cp "$CHARACTERIZATION_STUB_DOCKER" "$CASE_FAKE_BIN/docker" || return 1
  chmod +x "$CASE_FAKE_BIN/docker" || return 1
  cp "$CHARACTERIZATION_STUB_DOCKER_DENY" "$CASE_DOCKER_DENY_BIN/docker" || return 1
  chmod +x "$CASE_DOCKER_DENY_BIN/docker" || return 1
  FAKE_DOCKER_PATH="$CASE_FAKE_BIN/docker"

  : >"$CASE_DOCKER_CONTROL/.dstack-fake-docker-control"
  guard="dstack-test-$$-${RANDOM}-${RANDOM}"
  printf '%s' "$guard" >"$CASE_DOCKER_CONTROL/guard"

  HOME="$CASE_HOME"
  USER="dstack-test-user"
  PATH="$CASE_FAKE_BIN:$CASE_DOCKER_DENY_BIN:$CHARACTERIZATION_ORIGINAL_PATH"
  DSTACK_BASES="$CASE_BASES"
  DOCKER_CONFIG="$CASE_DOCKER_CONFIG"
  DOCKER_HOST="unix://$CASE_ROOT/nonexistent-docker.sock"
  LC_ALL=C
  TERM=dumb
  TMPDIR="$CASE_TMP"
  DSTACK_TEST_DOCKER_CONTROL="$CASE_DOCKER_CONTROL"
  DSTACK_TEST_DOCKER_GUARD="$guard"

  unset DSTACK DSTACK_COMPOSE_FILE DSTACK_COMPOSE_FILES XDG_CONFIG_HOME
  export HOME USER PATH DSTACK_BASES DOCKER_CONFIG DOCKER_HOST LC_ALL TERM TMPDIR
  export DSTACK_TEST_DOCKER_CONTROL DSTACK_TEST_DOCKER_GUARD FAKE_DOCKER_PATH

  cd "$CASE_CURRENT_DIR" || return 1
  hash -r
  invoke_fake_docker_canary || return 1
  reset_fake_docker_records || return 1
}
