#!/usr/bin/env bash

# ==================================================
# Logging and confirmation helpers (internal)
# ==================================================

### Basic logging function (internal)
log() {
  printf '%b\n' "${1:-}"
}

if [[ -t 1 ]]; then
  INFO="\033[0;34m\033[1m[INFO]\033[0m"
  OK="\033[0;32m\033[1m[OK]\033[0m"
  WARN="\033[0;33m\033[1m[WARN]\033[0m"
  ERR="\033[0;31m\033[1m[ERROR]\033[0m"
else
  INFO="[INFO]"
  OK="[OK]"
  WARN="[WARN]"
  ERR="[ERROR]"
fi

### Logging shortcuts (internal)
info() { log "$INFO $*"; }
ok()   { log "$OK $*"; }
warn() { log "$WARN $*"; }
err()  { log "$ERR $*"; }

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

  if [[ -f "$REGISTRY" ]]; then
    local path
    path="$(awk -F= -v s="$STACK" '$1 == s {print $2}' "$REGISTRY")"
    if [[ -n "$path" && -f "$path/docker-compose.yml" ]]; then
      echo "$path"
      return 0
    fi
  fi

  for base in $(_dstack_bases); do
    [[ -d "$base/$STACK" ]] || continue
    [[ -f "$base/$STACK/docker-compose.yml" ]] || continue
    echo "$base/$STACK"
    return 0
  done

  return 1
}

### Check if argument is a docker compose verb (internal)
_is_compose_verb() {
  case "$1" in
    up|down|start|stop|restart|logs|ps|pull|build|config|exec|run)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

### Docker compose command wrapper (internal)
_dcompose() {
  local dir
  local first_arg="$1"

  if [[ $# -gt 0 ]]; then
    if dir="$(_dstack_resolve "$first_arg")"; then
      shift
      docker compose -f "$dir/docker-compose.yml" "$@"
      return
    fi
  fi

  if [[ -n "${DSTACK:-}" ]]; then
    if [[ -f "$DSTACK/docker-compose.yml" ]]; then
      docker compose -f "$DSTACK/docker-compose.yml" "$@"
      return
    else
      warn "DSTACK invalid, clearing"
      unset DSTACK
    fi
  fi

  if [[ -f docker-compose.yml ]]; then
    docker compose "$@"
    return
  fi

  err "No Docker Compose context available"

  if [[ -n "$first_arg" ]] && ! _is_compose_verb "$first_arg"; then
    err "No Compose stack named '$first_arg' was found"
    err "Use 'dstack' to list available stacks"
    return 1
  fi

  err "Provide a stack name, select a stack, or run this inside a Compose project"
  err "Examples:"
  err "  <command> <stack>"
  err "  dstack <stack>"
  err "  cd <project> && <command>"
  return 1
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
    "$DSTACK_ROOT"/dstack.sh
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
  if [ -z "$1" ]; then
    err "Usage: dpsg <pattern>"
    return 1
  fi
  docker ps --format "table {{.ID}}\t{{.Names}}\t{{.Status}}" | grep -i "$1"
}

## List docker compose services
dsvc() {
   _dcompose "$@" ps --services
}

## Show container port mappings
dport() {
  docker ps --format "table {{.Names}}\t{{.Ports}}"
}

## Show IP address of a container
dip() {
  if [ -z "$1" ]; then
    err "Usage: dip <container-name>"
    return 1
  fi
  docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$1"
}

# ==================================================
# Docker compose stack management
# ==================================================

## Show available docker compose stacks or register a new docker compose stack
dstack() {
  local cmd="$1"
  local name="$2"
  local path="$3"
  local REGISTRY="$HOME/.config/dstack/registry"

  mkdir -p "$(dirname "$REGISTRY")"
  touch "$REGISTRY"

  if [[ -z "$cmd" || "$cmd" == "ls" ]]; then
    info "Registered docker stacks:"
    if [[ -s "$REGISTRY" ]]; then
      awk -F= '{printf "  %-20s %s\n", $1, $2}' "$REGISTRY"
    else
      echo "  No registered stacks found."
    fi

    info "Auto-discovered stacks:"
    for base in $(_dstack_bases); do
      [[ -d "$base" ]] || continue

      find "$base" -maxdepth 1 -mindepth 1 -type d 2>/dev/null |
      while read -r dir; do
        [[ -f "$dir/docker-compose.yml" ]] || continue
        printf "  %-20s %s\n" "$(basename "$dir")" "$dir"
      done
    done
    return 0
  fi

  if [[ "$cmd" == "add" ]]; then
    [[ -n "$name" && -n "$path" ]] || {
      err "Usage: dstack add <name> <path>"
      return 1
    }

    path="$(realpath -m "$path")"

    [[ -f "$path/docker-compose.yml" ]] || {
      err "No docker-compose.yml in $path"
      return 1
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
    return 1
  }

  export DSTACK="$DIR"
  ok "Docker stack set: $cmd -> $DIR"
}

## Unregister a docker compose stack
dstackunset() {
  local name="$1"
  local REGISTRY="$HOME/.config/dstack/registry"

  [[ -n "$name" ]] || {
    err "Usage: dstackunset <stack>"
    return 1
  }

  [[ -f "$REGISTRY" ]] || {
    err "No stack registry found"
    return 1
  }

  if ! awk -F= -v s="$name" '$1 == s {found=1} END{exit !found}' "$REGISTRY"; then
    err "Stack '$name' is not registered"
    return 1
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
  _dcompose "$@" start
}

## Stop docker compose services
dstop() {
  _dcompose "$@" stop
}

## Build and start docker stack
dcompose() {
  if [[ $# -eq 0 ]]; then
    _dcompose up -d --build --remove-orphans
    return
  fi

  if [[ $# -eq 1 ]] && _dstack_resolve "$1" >/dev/null 2>&1; then
    _dcompose "$1" up -d --build --remove-orphans
    return
  fi

  _dcompose "$@"
}

## Stop and remove containers + volumes
ddown() {
  _dcompose "$@" down -v
}

## Stop all running containers (system-wide)
dstopall() {
  docker ps -q | xargs -r docker stop
}

## Recreate docker stack with volume removal
drecompose() {
  info "Recreating docker stack with volume removal"
  _dcompose "$@" down -v || return 1
  _dcompose "$@" up -d
  ok "Stack recreated"
}

## Restart docker stack with status messages
drebootstack() {
  info "Restarting docker stack"
  _dcompose "$@" down || return 1
  _dcompose "$@" up -d || return 1
  ok "Stack restarted"
}

## Remove the current docker compose stack and prune unused Docker resources system-wide
dstackpurge() {
  warn "This will remove the CURRENT compose stack and prune UNUSED Docker resources system-wide"
  warn "Images and volumes still in use will NOT be removed"
  confirm "Continue?" || return 1
  _dcompose "$@" down -v || return 1
  docker system prune -f
  ok "Docker stack purged and unused resources pruned"
}

# ==================================================
# Docker logs & debugging
# ==================================================

## Follow logs for all services with optional line count (Ctrl+C to exit)
dlogs() {
  local lines=100

  if [[ $# -gt 0 && "$1" =~ ^[0-9]+$ ]]; then
    lines="$1"
    shift
  fi

  _dcompose "$@" logs -f --tail="$lines"
}

dlog() {
  if [[ $# -lt 1 ]]; then
    err "Usage: dlog [stack] <service>"
    return 1
  fi

  local service="${@: -1}"
  local args=("${@:1:$#-1}")

  _dcompose "${args[@]}" logs -f --tail=100 "$service"
}

## Show last logs for all services (paged)
dllogs() {
  local lines=100

  if [[ $# -gt 0 && "$1" =~ ^[0-9]+$ ]]; then
    lines="$1"
    shift
  fi

  _dcompose "$@" logs --tail="$lines" | less
}

## Show last logs for a single service (paged)
dllog() {
  if [[ $# -lt 1 ]]; then
    err "Usage: dllog [stack] <service> [lines]"
    return 1
  fi

  local service="${@: -2:1}"
  local lines="${@: -1}"

  [[ "$lines" =~ ^[0-9]+$ ]] || lines=100

  local args=("${@:1:$#-2}")

  _dcompose "${args[@]}" logs --tail="$lines" "$service" | less
}

## Live container resource usage (Ctrl+C to exit)
dstats() {
  docker stats
}

## Inspect a container (JSON output)
dinspect() {
  if [ -z "$1" ]; then
    err "Usage: dinspect <container-name>"
    return 1
  fi
  docker inspect "$1" | less
}

# ==================================================
# Docker exec & run
# ==================================================

## Exec into a running container
dexec() {
  if [[ $# -lt 1 ]]; then
    err "Usage: dexec [stack] <service>"
    return 1
  fi

  local service="${@: -1}"
  local args=("${@:1:$#-1}")

  _dcompose "${args[@]}" exec "$service" sh
}

## Run one-off commands in a service
drun() {
  if [[ $# -lt 2 ]]; then
    err "Usage: drun [stack] <service> <command>"
    return 1
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
  if [ -z "$1" ]; then
    err "Usage: dvolrm <volume-name>"
    return 1
  fi
  docker volume rm "$1"
}

## Inspect a docker volume
dvolinspect() {
  if [ -z "$1" ]; then
    err "Usage: dvolinspect <volume-name>"
    return 1
  fi
  docker volume inspect "$1"
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
  confirm "Continue?" || return 1
  docker system prune -a
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
   _dcompose "$@" pull
}

## Pull images and recreate containers
dupdate() {
  _dcompose "$@" pull && _dcompose "$@" up -d
}

## Rebuild and restart a single service
drebuild() {
  if [ -z "$1" ]; then
    echo "Usage: drebuild <service-name>"
    return 1
  fi
  _dcompose "$@" build "$1" && _dcompose "$@" up -d "$1"
}

## Rebuild all services without using cache
drebuildnocache() {
  _dcompose "$@" build --no-cache && _dcompose "$@" up -d
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
  if [ -z "$1" ]; then
    err "Usage: dnetinspect <network-name>"
    return 1
  fi
  docker network inspect "$1"
}

# ==================================================
# Docker compose utilities
# ==================================================

## Show resolved docker compose config
dconfig() {
  _dcompose "$@" config
}
