#!/usr/bin/env bash

_docker_call_id() {
  local call_number="$1"

  if [[ ! "$call_number" =~ ^[1-9][0-9]*$ ]] || ((10#$call_number > 999999)); then
    return 1
  fi

  printf '%06d' "$((10#$call_number))"
}

docker_call_dir() {
  local call_number="$1"
  local call_id

  call_id="$(_docker_call_id "$call_number")" || return 1
  printf '%s/calls/%s\n' "$DSTACK_TEST_DOCKER_CONTROL" "$call_id"
}

reset_fake_docker_records() {
  local control="${DSTACK_TEST_DOCKER_CONTROL:-}"

  if [[ -z "$control" || ! -f "$control/.dstack-fake-docker-control" ]]; then
    printf 'refusing to reset an unverified fake-Docker control directory\n' >&2
    return 1
  fi

  rm -rf -- "$control/calls"
  mkdir -p "$control/calls"
}

configure_fake_docker_response() {
  local call_number="$1"
  local status="$2"
  local stdout_file="${3:-}"
  local stderr_file="${4:-}"
  local call_id
  local response_dir

  call_id="$(_docker_call_id "$call_number")" || {
    printf 'invalid fake-Docker call number: %s\n' "$call_number" >&2
    return 1
  }

  if [[ ! "$status" =~ ^[0-9]+$ ]] || ((10#$status > 255)); then
    printf 'invalid fake-Docker response status: %s\n' "$status" >&2
    return 1
  fi

  response_dir="$DSTACK_TEST_DOCKER_CONTROL/responses/$call_id"
  mkdir -p "$response_dir"
  printf '%s\n' "$status" >"$response_dir/status"

  if [[ -n "$stdout_file" ]]; then
    cp "$stdout_file" "$response_dir/stdout"
  else
    : >"$response_dir/stdout"
  fi

  if [[ -n "$stderr_file" ]]; then
    cp "$stderr_file" "$response_dir/stderr"
  else
    : >"$response_dir/stderr"
  fi
}

assert_call_count() {
  local expected="$1"
  local control="${2:-$DSTACK_TEST_DOCKER_CONTROL}"
  local had_nullglob=0
  local -a calls=()

  shopt -q nullglob && had_nullglob=1
  shopt -s nullglob
  calls=("$control"/calls/[0-9][0-9][0-9][0-9][0-9][0-9])
  ((had_nullglob)) || shopt -u nullglob

  if ((${#calls[@]} != expected)); then
    fail "fake Docker call count: expected $expected, got ${#calls[@]}"
    return 1
  fi
}

_print_nul_arguments() {
  local file="$1"
  local argument
  local index=0

  while IFS= read -r -d '' argument; do
    printf '#   argv[%d]=%q\n' "$index" "$argument" >&2
    index=$((index + 1))
  done <"$file"
}

assert_argv() {
  local call_number="$1"
  shift
  local call_dir
  local expected_file
  local recorded_argc

  call_dir="$(docker_call_dir "$call_number")" || {
    fail "invalid fake-Docker call number: $call_number"
    return 1
  }

  if [[ ! -f "$call_dir/complete" || ! -f "$call_dir/argv.nul" ]]; then
    fail "fake Docker call $call_number is missing or incomplete"
    return 1
  fi

  _assertion_next_file "argv-${call_number}.expected.nul"
  expected_file="$ASSERTION_FILE"
  write_nul_arguments "$expected_file" "$@"

  if ! cmp -s "$expected_file" "$call_dir/argv.nul"; then
    fail "fake Docker call $call_number argv differs"
    printf '# expected:\n' >&2
    _print_nul_arguments "$expected_file"
    printf '# actual:\n' >&2
    _print_nul_arguments "$call_dir/argv.nul"
    return 1
  fi

  IFS= read -r recorded_argc <"$call_dir/argc" || true
  if [[ "$recorded_argc" != "$#" ]]; then
    fail "fake Docker call $call_number argc: expected $#, got ${recorded_argc:-<missing>}"
    return 1
  fi
}

assert_recorded_cwd() {
  local call_number="$1"
  local expected="$2"
  local call_dir
  local actual

  call_dir="$(docker_call_dir "$call_number")" || return 1
  IFS= read -r actual <"$call_dir/cwd" || true
  if [[ "$actual" != "$expected" ]]; then
    fail "fake Docker call $call_number cwd: expected $(printf '%q' "$expected"), got $(printf '%q' "$actual")"
    return 1
  fi
}

assert_recorded_env() {
  local call_number="$1"
  local expected_name="$2"
  local expected_state="$3"
  local expected_value="${4:-}"
  local call_dir
  local name
  local state
  local value

  call_dir="$(docker_call_dir "$call_number")" || return 1
  while IFS= read -r -d '' name &&
        IFS= read -r -d '' state &&
        IFS= read -r -d '' value; do
    if [[ "$name" == "$expected_name" ]]; then
      if [[ "$state" != "$expected_state" || "$value" != "$expected_value" ]]; then
        fail "fake Docker env '$expected_name': expected $expected_state/$(printf '%q' "$expected_value"), got $state/$(printf '%q' "$value")"
        return 1
      fi
      return 0
    fi
  done <"$call_dir/environment.nul"

  fail "fake Docker call $call_number did not record environment variable '$expected_name'"
}
