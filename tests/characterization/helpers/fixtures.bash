#!/usr/bin/env bash

_fixture_require_relative_path() {
  local path="$1"

  if [[ -z "$path" || "$path" == /* || "$path" == ".." || "$path" == ../* || "$path" == */../* || "$path" == */.. ]]; then
    printf 'fixture path must stay relative to the case root: %s\n' "$path" >&2
    return 1
  fi
}

fixture_create_project() {
  local relative="$1"
  shift
  local filename
  local parent

  _fixture_require_relative_path "$relative" || return 1
  FIXTURE_PROJECT="$CASE_PROJECTS/$relative"
  mkdir -p "$FIXTURE_PROJECT"

  for filename in "$@"; do
    _fixture_require_relative_path "$filename" || return 1
    parent="${filename%/*}"
    if [[ "$parent" != "$filename" ]]; then
      mkdir -p "$FIXTURE_PROJECT/$parent"
    fi
    : >"$FIXTURE_PROJECT/$filename"
  done
}

fixture_create_compose_project() {
  local relative="$1"
  local compose_filename="$2"
  local contents="${3:-}"

  fixture_create_project "$relative" || return 1
  fixture_write_file "projects/$relative/$compose_filename" "$contents"
  FIXTURE_PROJECT="$CASE_PROJECTS/$relative"
}

fixture_write_file() {
  local relative="$1"
  local contents="${2:-}"
  local target

  _fixture_require_relative_path "$relative" || return 1
  target="$CASE_ROOT/$relative"
  mkdir -p "${target%/*}"
  printf '%s' "$contents" >"$target"
  FIXTURE_FILE="$target"
}

fixture_copy_file() {
  local relative="$1"
  local source="$2"
  local target

  _fixture_require_relative_path "$relative" || return 1
  target="$CASE_ROOT/$relative"
  mkdir -p "${target%/*}"
  cp "$source" "$target"
  FIXTURE_FILE="$target"
}

fixture_create_registry() {
  mkdir -p "${CASE_REGISTRY%/*}"
  : >"$CASE_REGISTRY"
}

fixture_add_registry_entry() {
  local name="$1"
  local path="$2"

  if [[ ! -f "$CASE_REGISTRY" ]]; then
    fixture_create_registry || return 1
  fi
  printf '%s=%s\n' "$name" "$path" >>"$CASE_REGISTRY"
}

fixture_create_discovery_base() {
  local relative="$1"

  _fixture_require_relative_path "$relative" || return 1
  FIXTURE_BASE="$CASE_BASES/$relative"
  mkdir -p "$FIXTURE_BASE"
}

fixture_create_discovered_stack() {
  local base_relative="$1"
  local stack_relative="$2"
  local compose_filename="$3"
  local contents="${4:-}"

  _fixture_require_relative_path "$base_relative" || return 1
  _fixture_require_relative_path "$stack_relative" || return 1
  _fixture_require_relative_path "$compose_filename" || return 1

  FIXTURE_BASE="$CASE_BASES/$base_relative"
  FIXTURE_STACK="$FIXTURE_BASE/$stack_relative"
  mkdir -p "$FIXTURE_STACK"
  printf '%s' "$contents" >"$FIXTURE_STACK/$compose_filename"
}

fixture_create_nested_stack() {
  local relative="$1"
  local compose_filename="$2"
  local contents="${3:-}"

  fixture_create_compose_project "$relative" "$compose_filename" "$contents"
  FIXTURE_STACK="$FIXTURE_PROJECT"
}

fixture_set_cwd() {
  local directory="$1"
  local physical_root
  local physical_directory

  [[ -d "$directory" ]] || {
    printf 'fixture cwd does not exist: %s\n' "$directory" >&2
    return 1
  }

  physical_root="$(cd "$CASE_ROOT" && pwd -P)" || return 1
  physical_directory="$(cd "$directory" && pwd -P)" || return 1
  case "$physical_directory" in
    "$physical_root"|"$physical_root"/*) ;;
    *)
      printf 'fixture cwd must stay inside the case root: %s\n' "$directory" >&2
      return 1
      ;;
  esac

  cd "$directory"
}
