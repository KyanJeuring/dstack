use super::ShellKind;

const BASH: &str = r#"dstack() {
  local __dstack_arg __dstack_mutation __dstack_status
  if [[ "$#" -gt 0 && ( "$1" == "use" || "$1" == "unset" ) ]]; then
    for __dstack_arg in "$@"; do
      if [[ "$__dstack_arg" == "--help" || "$__dstack_arg" == "-h" ]]; then
        command dstack "$@"
        return $?
      fi
    done
    if [[ "$1" == "use" ]]; then
      __dstack_mutation="$(command dstack shell __activate bash "${@:2}")"
    else
      __dstack_mutation="$(command dstack shell __clear bash "${@:2}")"
    fi
    __dstack_status=$?
    if [[ "$__dstack_status" -ne 0 ]]; then
      return "$__dstack_status"
    fi
    eval "$__dstack_mutation"
    return $?
  fi
  command dstack "$@"
}"#;

const ZSH: &str = r#"dstack() {
  local __dstack_arg __dstack_mutation __dstack_status
  if (( $# > 0 )) && [[ "$1" == "use" || "$1" == "unset" ]]; then
    for __dstack_arg in "$@"; do
      if [[ "$__dstack_arg" == "--help" || "$__dstack_arg" == "-h" ]]; then
        command dstack "$@"
        return $?
      fi
    done
    if [[ "$1" == "use" ]]; then
      shift
      __dstack_mutation="$(command dstack shell __activate zsh "$@")"
    else
      shift
      __dstack_mutation="$(command dstack shell __clear zsh "$@")"
    fi
    __dstack_status=$?
    if (( __dstack_status != 0 )); then
      return $__dstack_status
    fi
    eval "$__dstack_mutation"
    return $?
  fi
  command dstack "$@"
}"#;

const FISH: &str = r#"function dstack
  if test (count $argv) -gt 0
    if test "$argv[1]" = use; or test "$argv[1]" = unset
      if contains -- -h $argv; or contains -- --help $argv
        command dstack $argv
        return $status
      end
      if test "$argv[1]" = use
        set -l __dstack_mutation (command dstack shell __activate fish $argv[2..-1])
      else
        set -l __dstack_mutation (command dstack shell __clear fish $argv[2..-1])
      end
      set -l __dstack_status $status
      if test $__dstack_status -ne 0
        return $__dstack_status
      end
      eval "$__dstack_mutation"
      return $status
    end
  end
  command dstack $argv
end"#;

const POWERSHELL: &str = r#"$global:__DStackExecutable = (Get-Command dstack -CommandType Application -ErrorAction Stop).Source
function global:dstack {
  [CmdletBinding(PositionalBinding = $false)]
  param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [object[]] $DStackArguments
  )

  if ($DStackArguments.Count -gt 0) {
    $commandName = [string] $DStackArguments[0]
    $hasHelp = ($DStackArguments -contains '--help') -or ($DStackArguments -contains '-h')
    if (($commandName -eq 'use' -or $commandName -eq 'unset') -and -not $hasHelp) {
      if ($DStackArguments.Count -gt 1) {
        $remaining = @($DStackArguments[1..($DStackArguments.Count - 1)])
      } else {
        $remaining = @()
      }
      if ($commandName -eq 'use') {
        $mutationArguments = @('shell', '__activate', 'powershell') + $remaining
      } else {
        $mutationArguments = @('shell', '__clear', 'powershell') + $remaining
      }
      $mutation = @(& $global:__DStackExecutable @mutationArguments)
      $nativeStatus = $LASTEXITCODE
      $global:LASTEXITCODE = $nativeStatus
      if ($nativeStatus -ne 0) {
        return
      }
      Invoke-Expression ($mutation -join "`n")
      $global:LASTEXITCODE = 0
      return
    }
  }

  & $global:__DStackExecutable @DStackArguments
  $nativeStatus = $LASTEXITCODE
  $global:LASTEXITCODE = $nativeStatus
}"#;

pub fn init_script(shell: ShellKind) -> &'static str {
    match shell {
        ShellKind::Bash => BASH,
        ShellKind::Zsh => ZSH,
        ShellKind::Fish => FISH,
        ShellKind::Powershell => POWERSHELL,
    }
}
