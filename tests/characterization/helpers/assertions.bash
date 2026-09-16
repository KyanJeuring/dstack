#!/usr/bin/env bash

# Assertions set TEST_CASE_FAILED so a case can report more than one useful
# mismatch before the runner decides its result.

fail() {
  local message="${1:-assertion failed}"
  TEST_CASE_FAILED=1
  printf '# %s\n' "$message" >&2
  return 1
}

assert_status() {
  local expected="$1"
  local actual="$2"
  local label="${3:-status}"

  if [[ "$actual" != "$expected" ]]; then
    fail "$label: expected status $expected, got $actual"
    return 1
  fi
}

assert_file_empty() {
  local file="$1"
  local label="${2:-$file}"

  if [[ ! -f "$file" ]]; then
    fail "$label: file does not exist: $file"
    return 1
  fi

  if [[ -s "$file" ]]; then
    fail "$label: expected an empty file, got $(wc -c <"$file") bytes"
    return 1
  fi
}

_assertion_next_file() {
  local stem="$1"
  ASSERTION_SEQUENCE=$((ASSERTION_SEQUENCE + 1))
  ASSERTION_FILE="$CASE_CAPTURE_DIR/assertion-${ASSERTION_SEQUENCE}-${stem}"
}

_assertion_diff() {
  local expected="$1"
  local actual="$2"

  diff -u "$expected" "$actual" 2>&1 |
    while IFS= read -r line || [[ -n "$line" ]]; do
      printf '# %s\n' "$line" >&2
    done
}

assert_bytes_equal() {
  local expected="$1"
  local actual="$2"
  local label="${3:-file bytes}"

  if [[ ! -f "$expected" ]]; then
    fail "$label: expected file does not exist: $expected"
    return 1
  fi

  if [[ ! -f "$actual" ]]; then
    fail "$label: actual file does not exist: $actual"
    return 1
  fi

  if ! cmp -s "$expected" "$actual"; then
    fail "$label: files differ (expected $(wc -c <"$expected") bytes, got $(wc -c <"$actual") bytes)"
    _assertion_diff "$expected" "$actual"
    return 1
  fi
}

assert_text_file() {
  local actual="$1"
  local expected_text="$2"
  local label="${3:-$actual}"

  _assertion_next_file text.expected
  printf '%s' "$expected_text" >"$ASSERTION_FILE"
  assert_bytes_equal "$ASSERTION_FILE" "$actual" "$label"
}

assert_var_set() {
  local name="$1"

  if [[ ! "$name" =~ ^[a-zA-Z_][a-zA-Z0-9_]*$ ]]; then
    fail "invalid variable name passed to assert_var_set: $name"
    return 1
  fi

  if ! declare -p "$name" >/dev/null 2>&1; then
    fail "expected variable '$name' to be set"
    return 1
  fi

  if [[ $# -ge 2 && "${!name}" != "$2" ]]; then
    fail "variable '$name': expected $(printf '%q' "$2"), got $(printf '%q' "${!name}")"
    return 1
  fi
}

assert_var_unset() {
  local name="$1"

  if [[ ! "$name" =~ ^[a-zA-Z_][a-zA-Z0-9_]*$ ]]; then
    fail "invalid variable name passed to assert_var_unset: $name"
    return 1
  fi

  if declare -p "$name" >/dev/null 2>&1; then
    fail "expected variable '$name' to be unset, got $(printf '%q' "${!name}")"
    return 1
  fi
}
