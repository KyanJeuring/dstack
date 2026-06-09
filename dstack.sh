#!/usr/bin/env bash
# shellcheck disable=SC2221,SC2222,SC2317,SC2155,SC2329,SC2086

if ((BASH_VERSINFO[0] < 4)); then
  echo "DStack requires Bash 4 or newer."
  exit 1
fi

# When sourced (common for shell helpers), do not change global shell options.
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  set -euo pipefail
  IFS=$'\n\t'
fi

DSTACK_VERSION="v1.9.0"

# ==================================================
# Logging and confirmation helpers (internal)
# ==================================================

### Basic logging function (internal)
log() {
  printf '%b\n' "$*"
}

_log_emit() {
  local level="$1"
  shift

  local prefix
  if [[ -t 1 ]]; then
    case "$level" in
      INFO)  prefix='\033[0;34m\033[1m[INFO]\033[0m' ;;
      OK)    prefix='\033[0;32m\033[1m[OK]\033[0m' ;;
      WARN)  prefix='\033[0;33m\033[1m[WARN]\033[0m' ;;
      ERROR) prefix='\033[0;31m\033[1m[ERROR]\033[0m' ;;
      *)     prefix="[$level]" ;;
    esac
  else
    prefix="[$level]"
  fi

  log "$prefix $*"
}

info() { _log_emit INFO "$@"; }
ok()   { _log_emit OK "$@"; }
warn() { _log_emit WARN "$@"; }
err()  { _log_emit ERROR "$@"; }

### Confirmation prompt (internal)
confirm() {
  read -rp "$1 [y/N]: " ans
  [[ "$ans" =~ ^[Yy]$ ]]
}

# ==================================================
# Docker helpers (internal)
# ==================================================

### List of deprecated functions
DEPRECATED_FUNCTIONS=(

)

### Remove deprecated functions automatically
for fn in "${DEPRECATED_FUNCTIONS[@]}"; do
  if declare -F "$fn" >/dev/null; then
    warn "'$fn' is deprecated and has been removed"
    unset -f "$fn"
  fi
done

### List of supported docker compose files (internal)
_dstack_compose_files() {
  local dir="$1"
  local file

  if [[ -n "${DSTACK_COMPOSE_FILES:-}" ]]; then
    printf '%s\n' $DSTACK_COMPOSE_FILES
    return
  fi

  for file in \
    "$dir/docker-compose.yml" \
    "$dir/docker-compose.yaml" \
    "$dir/compose.yml" \
    "$dir/compose.yaml" \
    "$dir"/*compose*.yml \
    "$dir"/*compose*.yaml
  do
    [[ -f "$file" ]] && printf '%s\n' "$file"
  done | awk '!seen[$0]++'
}

### Find docker compose file in directory (internal)
_dstack_find_compose_file() {
  local dir="$1"
  local file

  for file in $(_dstack_compose_files "$dir"); do
    [[ -f "$file" ]] && {
      echo "$file"
      return 0
    }
  done

  return 1
}

### Resolve compose files for a stack directory (internal)
_dstack_resolve_compose_files() {
  local dir="$1"
  local selector="${2:-}"
  local -a candidates=()
  local -a resolved=()
  local -a output=()
  local candidate
  local item
  local found
  local i
  local choice

  mapfile -t candidates < <(_dstack_compose_files "$dir")

  ((${#candidates[@]})) || return 1

  if [[ -z "$selector" ]]; then
    if ((${#candidates[@]} == 1)); then
      printf '%s\n' "${candidates[0]}"
      return 0
    fi

    if [[ ! -t 0 ]]; then
      err "Multiple compose files found. Choose one with -f, --file, --compose-file, or DSTACK_COMPOSE_FILE."
      return 1
    fi

    printf '%s\n' "Multiple compose files found:" >&2
    for i in "${!candidates[@]}"; do
      printf '  %s) %s\n' "$((i + 1))" "${candidates[$i]#"$dir"/}" >&2
    done
    printf '  %s) all\n' "$((${#candidates[@]} + 1))" >&2

    read -rp "Select compose file(s): " choice
    choice="${choice//[[:space:]]/}"

    [[ -n "$choice" ]] || {
      err "Invalid selection"
      return 1
    }

    if [[ "$choice" == "all" || "$choice" == "$((${#candidates[@]} + 1))" ]]; then
      printf '%s\n' "${candidates[@]}"
      return 0
    fi

    _dstack_resolve_compose_files "$dir" "$choice"
    return $?
  fi

  IFS=',' read -r -a resolved <<< "$selector"

  for item in "${resolved[@]}"; do
    found=""
    item="${item//[[:space:]]/}"

    [[ -n "$item" ]] || continue

    case "$item" in
      all)
        printf '%s\n' "${candidates[@]}"
        return 0
        ;;
      [0-9]*)
        if (( item >= 1 && item <= ${#candidates[@]} )); then
          found="${candidates[$((item - 1))]}"
        fi
        ;;
      */*|./*|../*|/*)
        if [[ -f "$item" ]]; then
          found="$item"
        elif [[ -f "$dir/$item" ]]; then
          found="$dir/$item"
        fi
        ;;
    esac

    if [[ -z "$found" ]]; then
      for candidate in "${candidates[@]}"; do
        if [[ "$candidate" == "$dir/$item" || "${candidate#"$dir"/}" == "$item" || "$(basename "$candidate")" == "$item" ]]; then
          found="$candidate"
          break
        fi
      done
    fi

    [[ -n "$found" ]] || {
      err "Compose file not found: $item"
      return 1
    }

    output+=("$found")
  done

  printf '%s\n' "${output[@]}" | awk '!seen[$0]++'
}

### Resolve compose files to docker compose -f flags (internal)
_dstack_resolve_compose_flags() {
  local dir="$1"
  local selector="${2:-}"
  local -a files=()
  local file

  mapfile -t files < <(_dstack_resolve_compose_files "$dir" "$selector") || return 1

  for file in "${files[@]}"; do
    printf '%s\n' -f "$file"
  done
}

### Detect compose file selector syntax (internal)
_dstack_looks_like_compose_selector() {
  local token="$1"

  [[ "$token" == "all" || "$token" == *,* || "$token" =~ ^[0-9]+$ || "$token" == *.yml || "$token" == *.yaml || "$token" == */* ]]
}

### Resolve docker compose stack path (internal)
_dstack_bases() {
  local user
  user="${USER:-$(whoami)}"

  if [[ -n "${DSTACK_BASES:-}" ]]; then
    echo "$DSTACK_BASES"
    return
  fi

  echo \
    "$HOME/projects" \
    "$HOME/src" \
    "$HOME/code" \
    "/opt/services" \
    "C:/Users/$user/projects" \
    "C:/Users/$user/src" \
    "C:/Users/$user/code"
}

### Resolve docker compose stack path by name (internal)
_dstack_resolve() {
  local STACK="$1"
  local REGISTRY="$HOME/.config/dstack/registry"
  local path

  if [[ -f "$REGISTRY" ]]; then
    path="$(awk -F= -v s="$STACK" '$1 == s {print $2}' "$REGISTRY")"
    if [[ -n "$path" ]] && _dstack_find_compose_file "$path" >/dev/null; then
      echo "$path"
      return 0
    fi
  fi

  for base in $(_dstack_bases); do
    path="$base/$STACK"
    [[ -d "$path" ]] || continue
    _dstack_find_compose_file "$path" >/dev/null || continue
    echo "$path"
    return 0
  done

  return 1
}

### Check if argument is a docker compose verb (internal)
_is_compose_verb() {
  if docker compose "$1" --help >/dev/null 2>&1; then
    return 0
  fi
  return 1
}

### Docker compose command wrapper (internal)
_dcompose() {
  local dir
  local compose
  local -a compose_files=()
  local -a compose_flags=()
  local selector=""
  local first_arg
  local interactive_select=1

  if [[ "${1:-}" == "--no-select" ]]; then
    interactive_select=0
    shift
  fi

  first_arg="${1:-}"

  if [[ $# -gt 0 ]]; then
    if dir="$(_dstack_resolve "$first_arg")"; then
      shift

      while [[ $# -gt 1 && ( "$1" == "-f" || "$1" == "--file" || "$1" == "--compose-file" ) ]]; do
        selector+="${selector:+,}$2"
        shift 2
      done

      if [[ -z "$selector" && -n "${DSTACK_COMPOSE_FILE:-}" ]]; then
        selector="$DSTACK_COMPOSE_FILE"
      fi

      if [[ "$interactive_select" -eq 0 && -z "$selector" ]]; then
        selector="1"
      fi

      mapfile -t compose_files < <(_dstack_resolve_compose_files "$dir" "$selector") || {
        err "No Docker Compose file found in $dir"
        return 0
      }

      for compose in "${compose_files[@]}"; do
        compose_flags+=("-f" "$compose")
      done

      docker compose "${compose_flags[@]}" "$@" || true
      return 0
    fi
  fi

  if [[ -n "${DSTACK:-}" ]]; then
    while [[ $# -gt 1 && ( "$1" == "-f" || "$1" == "--file" || "$1" == "--compose-file" ) ]]; do
      selector+="${selector:+,}$2"
      shift 2
    done

    if [[ -z "$selector" && -n "${DSTACK_COMPOSE_FILE:-}" ]]; then
      selector="$DSTACK_COMPOSE_FILE"
    fi

    if [[ "$interactive_select" -eq 0 && -z "$selector" ]]; then
      selector="1"
    fi

    if mapfile -t compose_files < <(_dstack_resolve_compose_files "$DSTACK" "$selector"); then
      for compose in "${compose_files[@]}"; do
        compose_flags+=("-f" "$compose")
      done

      docker compose "${compose_flags[@]}" "$@" || true
      return 0
    else
      warn "DSTACK invalid, clearing"
      unset DSTACK
    fi
  fi

  while [[ $# -gt 1 && ( "$1" == "-f" || "$1" == "--file" || "$1" == "--compose-file" ) ]]; do
    selector+="${selector:+,}$2"
    shift 2
  done

  if [[ -z "$selector" && -n "${DSTACK_COMPOSE_FILE:-}" ]]; then
    selector="$DSTACK_COMPOSE_FILE"
  fi

  if [[ "$interactive_select" -eq 0 && -z "$selector" ]]; then
    selector="1"
  fi

  if mapfile -t compose_files < <(_dstack_resolve_compose_files "$(pwd)" "$selector"); then
    for compose in "${compose_files[@]}"; do
      compose_flags+=("-f" "$compose")
    done

    docker compose "${compose_flags[@]}" "$@" || true
    return 0
  fi

  err "No Docker Compose context available"

  if [[ -n "$first_arg" ]] && ! _is_compose_verb "$first_arg"; then
    err "No Compose stack named '$first_arg' was found"
    err "Use 'dstack' to list available stacks"
    return 0
  fi

  err "Provide a stack name, select a stack, or run this inside a Compose project"
  err "Examples:"
  err "  <command> <stack>"
  err "  dstack <stack>"
  err "  cd <project> && <command>"
  return 0
}

# ==================================================
# DStack command listing
# ==================================================

## Show DStack commands
dhelp() {
  info "DStack commands"
  log

  local DSTACK_ROOT
  DSTACK_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"

  shopt -s nullglob
  local files=(
    "${BASH_SOURCE[0]}"
    "$DSTACK_ROOT"/commands/*.sh
  )
  shopt -u nullglob

  ((${#files[@]})) || {
    warn "No dstack command files found"
    return
  }

  awk '
    # Detect separator lines
    /^# [=]{5,}$/ {
      prev_sep = 1
      next
    }

    # Section title must be BETWEEN separators
    prev_sep && /^# / {
      section = substr($0, 3)
      prev_sep = 0
      in_section = 1
      next
    }

    # Anything else cancels separator expectation
    {
      prev_sep = 0
    }

    # Documented public commands
    /^## / && in_section {
      desc = substr($0, 4)
      getline

      if ($0 ~ /^[a-zA-Z_][a-zA-Z0-9_]*\(\)/) {
        name = $0
        sub(/\(\).*/, "", name)

        # Ignore internal helpers
        if (name ~ /^_/) next

        printf "[%s]\n%-22s %s\n", section, name, desc
      }
    }
  ' "${files[@]}" |
  awk '
    /^\[/ {
      if ($0 != last) {
        if (NR > 1) print ""
        print $0
        last = $0
      }
      next
    }
    { print "  " $0 }
  '
}

## Show DStack version
dversion() {
  echo "DStack $DSTACK_VERSION"
}

# ==================================================
# Docker lists & info
# ==================================================

## List running containers with status and ports
dps() {
  docker ps --format "table {{.ID}}\t{{.Label \"com.docker.compose.service\"}}\t{{.Names}}\t{{.Status}}\t{{.Ports}}" \
    | sed '1 s/service/SERVICE/' | column -t -s $'\t'
}

## List all containers with status and ports
dpsa() {
  docker ps -a --format "table {{.ID}}\t{{.Label \"com.docker.compose.service\"}}\t{{.Names}}\t{{.Status}}\t{{.Ports}}" \
    | sed '1 s/service/SERVICE/' | column -t -s $'\t'
}

## Grep running containers by name
dpsg() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dpsg <pattern>

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  dpsg nginx        # Find containers matching 'nginx'
  dpsg my-app       # Find containers matching 'my-app'
EOF
    return 0
  fi

  if [ -z "$1" ]; then
    err "Usage: dpsg <pattern>"
    return 0
  fi
  docker ps --format "table {{.ID}}\t{{.Names}}\t{{.Status}}" | grep -i "$1" || true
}

## List docker compose services
dsvc() {
   _dcompose --no-select "$@" ps --services
}

## Show container port mappings
dport() {
  docker ps --format "table {{.Names}}\t{{.Ports}}"
}

## Show IP address of a container
dip() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dip <container-name>

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  dip my-container      # Show IP of 'my-container'
EOF
    return 0
  fi

  if [ -z "$1" ]; then
    err "Usage: dip <container-name>"
    return 0
  fi
  docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$1" || true
}

# ==================================================
# Docker compose stack management
# ==================================================

## Show available stacks or register a new stack with 'dstack add'
dstack() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dstack [ls]
  dstack add <name> <path>
  dstack <stack> [compose-file]
  dstack <stack> -f <compose-file>

OPTIONS:
  -h, --help    Show this help message
  -f, --file, --compose-file    Select compose file for active stack

EXAMPLES:
  dstack                              # List all available stacks
  dstack ls                           # List all available stacks
  dstack add myapp ~/projects/myapp   # Register a stack
  dstack myapp                        # Set myapp as active stack context
  dstack myapp compose.prod.yaml      # Set myapp and select a compose file
  dstack myapp -f compose.prod.yaml   # Set myapp and select a compose file

NOTES:
  - Stacks are auto-discovered from common base directories
  - Registered stacks are stored in ~/.config/dstack/registry
  - Setting a stack exports the DSTACK environment variable
  - You can select compose files with a positional name, numeric index, or `-f`
EOF
    return 0
  fi

  local cmd="${1:-}"
  local name="${2:-}"
  local path="${3:-}"
  local compose_file=""

  if [[ $# -ge 3 ]]; then
    case "$2" in
      -f|--file|--compose-file)
        compose_file="$3"
        ;;
      *)
        compose_file="$2"
        ;;
    esac
  elif [[ $# -eq 2 ]]; then
    compose_file="$2"
  fi

  local REGISTRY="$HOME/.config/dstack/registry"
  local MAX_DEPTH=2

  mkdir -p "$(dirname "$REGISTRY")"
  touch "$REGISTRY"

  if [[ -z "$cmd" || "$cmd" == "ls" ]]; then
    info "Registered stacks:"
    if [[ -s "$REGISTRY" ]]; then
      awk -F= '{printf "  %-20s %s\n", $1, $2}' "$REGISTRY"
    else
      echo "  No registered stacks found."
    fi

    info "Auto-discovered stacks:"
    for base in $(_dstack_bases); do
      [[ -d "$base" ]] || continue

      find "$base" -maxdepth "$MAX_DEPTH" -mindepth 1 -type d 2>/dev/null |
      while read -r dir; do
        _dstack_find_compose_file "$dir" >/dev/null || continue
        printf "  %-20s %s\n" "${dir#"$base"/}" "$dir"
      done
    done
    return 0
  fi

  if [[ "$cmd" == "add" ]]; then
    [[ -n "$name" && -n "$path" ]] || {
      err "Usage: dstack add <name> <path>"
      return 0
    }

    path="$(cd "$path" 2>/dev/null && pwd -P)" || {
      err "Invalid path"
      return 0
    }

    _dstack_find_compose_file "$path" >/dev/null || {
      err "No Docker Compose file found in $path"
      return 0
    }

    grep -v "^$name=" "$REGISTRY" 2>/dev/null >"$REGISTRY.tmp"
    echo "$name=$path" >>"$REGISTRY.tmp"
    mv "$REGISTRY.tmp" "$REGISTRY"

    ok "Registered stack '$name' -> $path"
    return 0
  fi

  local DIR
  DIR="$(_dstack_resolve "$cmd")" || {
    err "Stack not found: $cmd"
    return 0
  }

  export DSTACK="$DIR"

  if [[ -n "$compose_file" ]]; then
    export DSTACK_COMPOSE_FILE="$compose_file"
    ok "Docker stack set: $cmd -> $DIR ($compose_file)"
  else
    unset DSTACK_COMPOSE_FILE 2>/dev/null || true
    ok "Docker stack set: $cmd -> $DIR"
  fi
  return 0
}

## Unregister a docker compose stack
dstackunset() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dstackunset <stack>

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  dstackunset myapp     # Unregister the 'myapp' stack

NOTES:
  - Only removes the stack from the registry
  - Does not affect the actual project files
  - Clears DSTACK context if the unregistered stack was active
EOF
    return 0
  fi

  local name="$1"
  local REGISTRY="$HOME/.config/dstack/registry"

  [[ -n "$name" ]] || {
    err "Usage: dstackunset <stack>"
    return 0
  }

  [[ -f "$REGISTRY" ]] || {
    err "No stack registry found"
    return 0
  }

  if ! awk -F= -v s="$name" '$1 == s {found=1} END{exit !found}' "$REGISTRY"; then
    err "Stack '$name' is not registered"
    return 0
  fi

  awk -F= -v s="$name" '$1 != s' "$REGISTRY" >"$REGISTRY.tmp" &&
    mv "$REGISTRY.tmp" "$REGISTRY"

  if [[ -n "${DSTACK:-}" ]]; then
    local resolved
    resolved="$(_dstack_resolve "$name" 2>/dev/null || true)"
    [[ "$DSTACK" == "$resolved" ]] && {
      unset DSTACK
      info "Docker stack context cleared"
    }
  fi

  ok "Unregistered docker stack '$name'"
}

## Start docker compose services
dstart() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dstart [stack]

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  dstart              # Start services in current context
  dstart myapp        # Start services in the 'myapp' stack
EOF
    return 0
  fi

  _dcompose "$@" start
}

## Stop docker compose services
dstop() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dstop [stack]

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  dstop               # Stop services in current context
  dstop myapp         # Stop services in the 'myapp' stack
EOF
    return 0
  fi

  _dcompose --no-select "$@" stop
}

## Run Docker Compose commands
dcompose() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dcompose [stack] [subcommand] [args...]

OPTIONS:
  -h, --help    Show this help message
  -f, --file    Specify compose file(s) (can be used multiple times or with comma-separated list)

EXAMPLES:
  dcompose                                 # Run: up -d --build --remove-orphans in current context
  dcompose -f compose.prod.yaml            # Use specific compose file and start services
  dcompose down -v                         # Stop and remove containers + volumes in current context
  dcompose myapp                           # Run: up -d --build --remove-orphans in 'myapp' stack
  dcompose myapp -f compose.prod.yaml      # Run 'up' in the 'myapp' stack with specific compose file
  dcompose myapp ps                        # Run 'ps' in the 'myapp' stack
  dcompose logs -f                         # Run 'logs -f' in current context
  dcompose -f compose.prod.yaml logs -f    # Use specific compose file and follow logs

NOTES:
  - When called with no subcommand, defaults to: up -d --build --remove-orphans
EOF
    return 0
  fi

  if [[ $# -eq 0 ]]; then
    info "No arguments given, running: up -d --build --remove-orphans"
    _dcompose up -d --build --remove-orphans || true
    return 0
  fi

  if [[ $# -eq 1 ]] && _dstack_resolve "$1" >/dev/null 2>&1; then
    info "No subcommand given, running: up -d --build --remove-orphans"
    _dcompose "$1" up -d --build --remove-orphans || true
    return 0
  fi

  if [[ $# -gt 0 ]]; then
    local -a remaining=("$@")

    if _dstack_resolve "${remaining[0]}" >/dev/null 2>&1; then
      remaining=("${remaining[@]:1}")
    fi

    while ((${#remaining[@]} > 0)); do
      case "${remaining[0]}" in
        -f|--file|--compose-file)
          ((${#remaining[@]} >= 2)) || {
            err "Missing compose file after ${remaining[0]}"
            return 0
          }
          remaining=("${remaining[@]:2}")
          ;;
        *)
          break
          ;;
      esac
    done

    if ((${#remaining[@]} == 0)); then
      info "No subcommand given, running: up -d --build --remove-orphans"
      _dcompose "$@" up -d --build --remove-orphans || true
      return 0
    fi
  fi

  if [[ $# -gt 1 ]]; then
    local first_arg="$1"
    if _dstack_resolve "$first_arg" >/dev/null 2>&1; then
      shift
      _dcompose "$first_arg" "$@" || true
      return 0
    fi
  fi

  _dcompose "$@" || true
  return 0
}

## Stop and remove containers (preserve volumes)
ddown() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  ddown [stack]

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  ddown               # Stop and remove containers in current context
  ddown myapp         # Stop and remove containers in the 'myapp' stack

NOTES:
  - Volumes are preserved. Use ddownv to also remove volumes.
EOF
    return 0
  fi

  _dcompose --no-select "$@" down
}

## Stop and remove containers + volumes (destructive)
ddownv() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  ddownv [stack]

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  ddownv              # Stop and remove containers + volumes in current context
  ddownv myapp        # Stop and remove containers + volumes in the 'myapp' stack

NOTES:
  - This is destructive. All volumes for the stack will be removed.
  - Use ddown to preserve volumes.
EOF
    return 0
  fi

  warn "This will remove containers + volumes"
  confirm "Continue?" || return 0
  _dcompose --no-select "$@" down -v || true
  return 0
}

## Stop all running containers (system-wide)
dstopall() {
  docker ps -q | xargs docker stop 2>/dev/null || true
}

## Recreate docker stack with volume removal
drecompose() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  drecompose [stack]

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  drecompose              # Recreate stack in current context
  drecompose myapp        # Recreate the 'myapp' stack

NOTES:
  - Removes volumes before recreating. This is destructive.
  - Use drebootstack to restart without removing volumes.
EOF
    return 0
  fi

  if [[ $# -gt 1 ]]; then
    err "Usage: drecompose [stack]"
    return 0
  fi

  local dir
  local selector="${DSTACK_COMPOSE_FILE:-}"
  local -a compose_flags=()

  if [[ $# -eq 1 ]]; then
    dir="$(_dstack_resolve "$1")" || {
      err "Stack not found: $1"
      return 0
    }
  elif [[ -n "${DSTACK:-}" ]]; then
    dir="$DSTACK"
  else
    dir="$(pwd)"
  fi

  mapfile -t compose_flags < <(_dstack_resolve_compose_flags "$dir" "$selector") || {
    err "No Docker Compose file found in $dir"
    return 0
  }

  info "Recreating docker stack with volume removal"
  docker compose "${compose_flags[@]}" down -v || true
  docker compose "${compose_flags[@]}" up -d || true
  ok "Stack recreated"
  return 0
}


## Restart docker stack with status messages
drebootstack() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  drebootstack [stack]

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  drebootstack            # Reboot stack in current context
  drebootstack myapp      # Reboot the 'myapp' stack

NOTES:
  - Runs down then up. Containers are recreated but volumes are preserved.
  - Use drecompose to also remove volumes.
  - Use drestart to restart without recreating containers.
EOF
    return 0
  fi

  if [[ $# -gt 1 ]]; then
    err "Usage: drebootstack [stack]"
    return 0
  fi

  local dir
  local selector="${DSTACK_COMPOSE_FILE:-}"
  local -a compose_flags=()

  if [[ $# -eq 1 ]]; then
    dir="$(_dstack_resolve "$1")" || {
      err "Stack not found: $1"
      return 0
    }
  elif [[ -n "${DSTACK:-}" ]]; then
    dir="$DSTACK"
  else
    dir="$(pwd)"
  fi

  mapfile -t compose_flags < <(_dstack_resolve_compose_flags "$dir" "$selector") || {
    err "No Docker Compose file found in $dir"
    return 0
  }

  info "Restarting docker stack"
  docker compose "${compose_flags[@]}" down || true
  docker compose "${compose_flags[@]}" up -d || true
  ok "Stack restarted"
  return 0
}


## Restart one or all services in a stack
drestart() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  drestart [stack] [service]

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  drestart                      # Restart all services in current context
  drestart <service>            # Restart one service in current context
  drestart <stack>              # Restart all services in a named stack
  drestart <stack> <service>    # Restart one service in a named stack

NOTES:
  - drestart does not recreate containers (use drebootstack, drecompose, dcompose or drebuild for that)
  - Stack resolution follows standard DStack rules
EOF
    return 0
  fi

  if [[ $# -gt 2 ]]; then
    err "Usage: drestart [stack] [service]"
    return 0
  fi

  if [[ $# -eq 2 ]] && _dstack_resolve "$1" >/dev/null 2>&1; then
    local stack="$1"
    local service="$2"
    info "Restarting service '$service' in stack '$stack'"
    _dcompose --no-select "$stack" restart "$service"
  elif [[ $# -eq 1 ]] && _dstack_resolve "$1" >/dev/null 2>&1; then
    info "Restarting all services in stack '$1'"
    _dcompose "$1" restart
  elif [[ $# -eq 1 ]]; then
    info "Restarting service '$1'"
    _dcompose --no-select restart "$1"
  else
    info "Restarting all services"
    _dcompose --no-select restart
  fi

  ok "Done"
}

## Remove the current docker compose stack and prune unused Docker resources system-wide
dstackpurge() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dstackpurge [stack]

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  dstackpurge             # Purge stack in current context and prune Docker resources
  dstackpurge myapp       # Purge the 'myapp' stack and prune Docker resources

NOTES:
  - Removes the compose stack including volumes.
  - Also runs docker system prune to remove unused system-wide resources.
  - Images and volumes still in use will NOT be removed.
EOF
    return 0
  fi

  if [[ $# -gt 1 ]]; then
    err "Usage: dstackpurge [stack]"
    return 0
  fi

  warn "This will remove the CURRENT compose stack and prune UNUSED Docker resources system-wide"
  warn "Images and volumes still in use will NOT be removed"
  confirm "Continue?" || return 0
  _dcompose --no-select "$@" down -v || true
  docker system prune -f || true
  ok "Docker stack purged and unused resources pruned"
  return 0
}


# ==================================================
# Docker logs & debugging
# ==================================================

## Follow logs for all services with optional line count (Ctrl+C to exit)
dlogs() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dlogs [lines] [stack]

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  dlogs                   # Follow logs in current context (last 100 lines)
  dlogs 50                # Follow logs with last 50 lines
  dlogs myapp             # Follow logs in the 'myapp' stack
  dlogs 50 myapp          # Follow logs in 'myapp' with last 50 lines

NOTES:
  - Press Ctrl+C to exit
EOF
    return 0
  fi

  local lines=100

  if [[ $# -gt 0 && "$1" =~ ^[0-9]+$ ]]; then
    lines="$1"
    shift
  fi

  _dcompose --no-select "$@" logs -f --tail="$lines"
}

## Follow logs for a single service with optional line count (Ctrl+C to exit)
dlog() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dlog [stack] <service>

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  dlog nginx              # Follow logs for the 'nginx' service in current context
  dlog myapp nginx        # Follow logs for 'nginx' in the 'myapp' stack

NOTES:
  - Press Ctrl+C to exit
EOF
    return 0
  fi

  if [[ $# -lt 1 ]]; then
    err "Usage: dlog [stack] <service>"
    return 0
  fi

  local service="${!#}"
  local args=("${@:1:$#-1}")

  _dcompose --no-select "${args[@]}" logs -f --tail=100 "$service"
}

## Show last logs for all services (paged)
dllogs() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dllogs [lines] [stack]

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  dllogs                  # Show last 100 lines for all services in current context
  dllogs 50               # Show last 50 lines for all services
  dllogs myapp            # Show last 100 lines in the 'myapp' stack
  dllogs 50 myapp         # Show last 50 lines in 'myapp'

NOTES:
  - Output is paged with less
EOF
    return 0
  fi

  local lines=100

  if [[ $# -gt 0 && "$1" =~ ^[0-9]+$ ]]; then
    lines="$1"
    shift
  fi

  _dcompose --no-select "$@" logs --tail="$lines" | less
}

## Show last logs for a single service (paged)
dllog() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dllog [stack] <service> [lines]

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  dllog nginx             # Show last 100 lines for 'nginx' in current context
  dllog nginx 50          # Show last 50 lines for 'nginx'
  dllog myapp nginx       # Show last 100 lines for 'nginx' in 'myapp' stack
  dllog myapp nginx 50    # Show last 50 lines for 'nginx' in 'myapp' stack

NOTES:
  - Output is paged with less
EOF
    return 0
  fi

  if [[ $# -lt 1 ]]; then
    err "Usage: dllog [stack] <service> [lines]"
    return 0
  fi

  local argc=$#
  local last_index=$argc
  local prev_index=$((argc-1))
  local lines="${!last_index}"
  local service="${!prev_index}"

  [[ "$lines" =~ ^[0-9]+$ ]] || lines=100

  local args=("${@:1:$#-2}")

  _dcompose --no-select "${args[@]}" logs --tail="$lines" "$service" | less
}

## Live container resource usage (Ctrl+C to exit)
dstats() {
  docker stats
}

## Inspect a container (JSON output)
dinspect() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dinspect <container-name>

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  dinspect my-container       # Inspect 'my-container' (paged JSON output)

NOTES:
  - Output is paged with less
EOF
    return 0
  fi

  if [ -z "$1" ]; then
    err "Usage: dinspect <container-name>"
    return 0
  fi
  docker inspect "$1" | less || true
}

# ==================================================
# Docker exec & run
# ==================================================

## Exec into a running container
dexec() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dexec [stack] <service>

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  dexec nginx             # Exec into 'nginx' in current context
  dexec myapp nginx       # Exec into 'nginx' in the 'myapp' stack

NOTES:
  - Opens a shell (sh) inside the container
EOF
    return 0
  fi

  if [[ $# -lt 1 ]]; then
    err "Usage: dexec [stack] <service>"
    return 0
  fi

  local service="${!#}"
  local args=("${@:1:$#-1}")

  _dcompose --no-select "${args[@]}" exec "$service" sh

}

## Run one-off commands in a service
drun() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  drun [stack] <service> <command>

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  drun app bash                     # Run bash in 'app' in current context
  drun myapp app bash               # Run bash in 'app' in the 'myapp' stack
  drun app "ls -la /data"           # Run 'ls -la /data' in 'app' in current context

NOTES:
  - Container is removed after the command exits (--rm)
EOF
    return 0
  fi

  if [[ $# -lt 2 ]]; then
    err "Usage: drun [stack] <service> <command>"
    return 0
  fi

  local service
  local args=()

  if _dstack_resolve "$1" >/dev/null 2>&1; then
    args+=("$1")
    service="$2"
    shift 2
  else
    service="$1"
    shift 1
  fi

  _dcompose "${args[@]}" run --rm "$service" "$@"
}

# ==================================================
# Docker images & volumes
# ==================================================

## List images with size
dimg() {
  docker images --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}"
}

## List docker volumes
dvol() {
  docker volume ls
}

## Remove a docker volume
dvolrm() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dvolrm <volume-name>

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  dvolrm myapp_data       # Remove the 'myapp_data' volume

NOTES:
  - The volume must not be in use by any container
EOF
    return 0
  fi

  if [ -z "$1" ]; then
    err "Usage: dvolrm <volume-name>"
    return 0
  fi
  docker volume rm "$1" || true
}

## Inspect a docker volume
dvolinspect() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dvolinspect <volume-name>

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  dvolinspect myapp_data      # Inspect the 'myapp_data' volume
EOF
    return 0
  fi

  if [ -z "$1" ]; then
    err "Usage: dvolinspect <volume-name>"
    return 0
  fi
  docker volume inspect "$1" || true
}

# ==================================================
# Docker cleanup
# ==================================================

## Remove stopped containers
dclean() {
  docker container prune -f
}

## Remove dangling images
dcleani() {
  docker image prune -f
}

## Full cleanup (destructive)
dcleanall() {
  warn "Removing unused containers, images, and networks"
  confirm "Continue?" || return 0
  docker system prune -a || true
  return 0
}

## Show what prune would remove
dprunewhat() {
  docker system prune --dry-run
}

# ==================================================
# Docker updates & rebuilds
# ==================================================

## Pull latest images
dpull() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dpull [stack]

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  dpull               # Pull latest images in current context
  dpull myapp         # Pull latest images for the 'myapp' stack
EOF
    return 0
  fi

  if [[ $# -gt 1 ]]; then
    err "Usage: dpull [stack]"
    return 0
  fi

   _dcompose "$@" pull || true
  return 0
}

## Pull images and recreate containers
dupdate() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dupdate [stack]

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  dupdate             # Pull and recreate containers in current context
  dupdate myapp       # Pull and recreate containers in the 'myapp' stack
EOF
    return 0
  fi

  if [[ $# -gt 1 ]]; then
    err "Usage: dupdate [stack]"
    return 0
  fi

  local dir
  local selector="${DSTACK_COMPOSE_FILE:-}"
  local -a compose_flags=()

  if [[ $# -eq 1 ]]; then
    dir="$(_dstack_resolve "$1")" || {
      err "Stack not found: $1"
      return 0
    }
  elif [[ -n "${DSTACK:-}" ]]; then
    dir="$DSTACK"
  else
    dir="$(pwd)"
  fi

  mapfile -t compose_flags < <(_dstack_resolve_compose_flags "$dir" "$selector") || {
    err "No Docker Compose file found in $dir"
    return 0
  }

  docker compose "${compose_flags[@]}" pull || true
  docker compose "${compose_flags[@]}" up -d || true
  return 0
}

## Rebuild and restart a single service
drebuild() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  drebuild [stack] <service>

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  drebuild app            # Rebuild and restart 'app' in current context
  drebuild myapp app      # Rebuild and restart 'app' in the 'myapp' stack
EOF
    return 0
  fi

  if [[ $# -lt 1 || $# -gt 2 ]]; then
    echo "Usage: drebuild [stack] <service-name>"
    return 0
  fi

  local dir
  local service
  local selector="${DSTACK_COMPOSE_FILE:-}"
  local -a compose_flags=()

  if [[ $# -eq 2 ]] && _dstack_resolve "$1" >/dev/null 2>&1; then
    dir="$(_dstack_resolve "$1")"
    service="$2"
  else
    service="$1"
    if [[ -n "${DSTACK:-}" ]]; then
      dir="$DSTACK"
    else
      dir="$(pwd)"
    fi
  fi

  mapfile -t compose_flags < <(_dstack_resolve_compose_flags "$dir" "$selector") || {
    err "No Docker Compose file found in $dir"
    return 0
  }

  docker compose "${compose_flags[@]}" build "$service" 2>/dev/null || true
  docker compose "${compose_flags[@]}" up -d "$service" 2>/dev/null || true
  return 0
}


## Rebuild all services without using cache
drebuildnocache() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  drebuildnocache [stack]

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  drebuildnocache             # Rebuild all services in current context without cache
  drebuildnocache myapp       # Rebuild all services in 'myapp' without cache
EOF
    return 0
  fi

  if [[ $# -gt 1 ]]; then
    err "Usage: drebuildnocache [stack]"
    return 0
  fi

  local dir
  local selector="${DSTACK_COMPOSE_FILE:-}"
  local -a compose_flags=()

  if [[ $# -eq 1 ]]; then
    dir="$(_dstack_resolve "$1")" || {
      err "Stack not found: $1"
      return 0
    }
  elif [[ -n "${DSTACK:-}" ]]; then
    dir="$DSTACK"
  else
    dir="$(pwd)"
  fi

  mapfile -t compose_flags < <(_dstack_resolve_compose_flags "$dir" "$selector") || {
    err "No Docker Compose file found in $dir"
    return 0
  }

  docker compose "${compose_flags[@]}" build --no-cache 2>/dev/null || true
  docker compose "${compose_flags[@]}" up -d 2>/dev/null || true
  return 0
}

# ==================================================
# Docker networking
# ==================================================

## List docker networks
dnet() {
  docker network ls
}

## Inspect a docker network
dnetinspect() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dnetinspect <network-name>

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  dnetinspect myapp_default       # Inspect the 'myapp_default' network
EOF
    return 0
  fi

  if [ -z "$1" ]; then
    err "Usage: dnetinspect [stack] <network-name>"
    return 0
  fi
  docker network inspect "$1" || true
}

# ==================================================
# Docker compose utilities
# ==================================================

## Show resolved docker compose config
dconfig() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
USAGE:
  dconfig [stack]

OPTIONS:
  -h, --help    Show this help message

EXAMPLES:
  dconfig             # Show resolved config in current context
  dconfig myapp       # Show resolved config for the 'myapp' stack

NOTES:
  - Outputs the fully resolved docker compose configuration
  - Useful for debugging variable substitution and extension merges
EOF
    return 0
  fi

  if [[ $# -gt 1 ]]; then
    err "Usage: dconfig [stack]"
    return 0
  fi

  _dcompose --no-select "$@" config || true
  return 0
}
