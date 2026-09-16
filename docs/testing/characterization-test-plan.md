# DStack characterization test plan

## 1. Purpose and testing goals

This plan defines a safe automated suite for recording the externally observable behavior of the current Bash implementation before a major refactor. The suite is a measuring instrument: a passing characterization test says what DStack does today, not that the behavior is desirable or permanent.

Every case should carry one of these labels in its name or test metadata:

- **intentional contract candidate**: documented behavior that should almost certainly be preserved, subject to maintainer review;
- **compatibility-sensitive / undecided**: observed behavior that must be measured before deciding whether to preserve it;
- **possible bug/quirk**: surprising or contradictory observed behavior that must not silently become an intentional contract;
- **platform-specific**: an expectation owned by one supported environment;
- **unsafe/deferred**: behavior documented by analysis but not automatically exercised yet.

The current Bash implementation remains the initial oracle. Expected results must be reviewed and checked in as explicit assertions or small golden files. A future implementation may intentionally differ from a possible bug/quirk after a human decision; such divergence should be recorded in a decision/allowlist file rather than hidden by broad normalization.

The suite should answer:

1. Which context, Compose files, and arguments reach Docker?
2. What state, files, output streams, and statuses does each command produce?
3. Which results are stable across Linux, macOS, and Windows/Git Bash?
4. Which observations are contract candidates, undecided quirks, or possible bugs?
5. Can the same input fixture be run against current Bash DStack and a future implementation?

## 2. Re-verified current behavior relevant to testing

The repository was re-read before this plan: `AGENTS.md`, all of `dstack.sh`, all of `README.md`, `CONTRIBUTING.md`, and all of `.github/workflows/ci.yml`. Safe local probes used a child Bash, temporary directories, and a shell-level fake Docker; no real Docker command was run.

Verified from code and local probes:

- Sourcing defines public and internal functions, sets `DSTACK_VERSION=v1.11.0`, and preserves the caller's shell flags and `IFS`.
- Direct execution enables strict mode internally but has no dispatcher. `./dstack.sh dversion` exits 0 and prints nothing; `source ./dstack.sh; dversion` prints `DStack v1.11.0` and exits 0.
- `dhelp` leaves `nullglob` disabled even when it was enabled before the call.
- `_dcompose` resolves an explicit first token, then active `DSTACK`, then the exact current directory. It normally supplies absolute `-f` paths and does not change directory.
- `drecompose`, `drebootstack`, `dupdate`, `drebuild`, and `drebuildnocache` duplicate part of context/file resolution and need their own cases.
- Registered stacks precede base discovery. Listing scans directories at depths 1 and 2; name resolution tests only `BASE/NAME`.
- The registry is `$HOME/.config/dstack/registry`; ordinary `dstack` calls create/touch it. Registration uses `pwd -P` and a fixed `registry.tmp`.
- Default Compose candidates and their ordering match `AGENTS.md`. `DSTACK_COMPOSE_FILES` emits whitespace-split values verbatim rather than joining them to the stack path.
- With no selector, one file is automatic; several files prompt only when stdin is a TTY. `_dcompose --no-select` substitutes selector `1`.
- Resolver errors are logged on stdout. Because callers use process substitution with `mapfile`, an invalid selector or non-TTY multi-file error can be captured as a literal Compose filename and passed after `-f`.
- `_dstack_resolve_compose_flags` can return 0 with empty output when there are no candidates.
- Selecting then unregistering a registered-only active stack removes the registry entry but leaves `DSTACK` and `DSTACK_COMPOSE_FILE` set.
- `_dcompose` masks Docker failures with `|| true`; representative direct helpers without it return the Docker status.
- `dllog SERVICE` reaches a negative Bash array-slice length. `dlog` always requests 100 lines despite its source description mentioning an optional count.
- Logging levels write to stdout. Selection menu lines are redirected to stderr; prompts and TTY color require dedicated stream/PTY tests.

Not verified locally and therefore not to be assumed:

- Exact runtime results on GitHub's current macOS and Windows/Git Bash images.
- The Bash 3 failure path; no Bash 3 interpreter was used. The code plainly executes `exit 1` when sourced under a major version below 4.
- Exact interactive prompt bytes, terminal echo, ANSI output, and pager behavior under a real PTY.
- Real Docker output or daemon behavior. The characterization suite must not use either.
- Concurrent registry-update outcomes. The fixed temp filename makes races plausible, but a deterministic cross-platform expectation has not been established.

## 3. Recommended test framework

Use a **plain Bash harness** that emits TAP-like `ok`/`not ok` results and a final count. Keep the harness small, documented, and covered by self-tests before using it as the oracle.

| Option | Strengths | Costs and limitations | Decision |
|---|---|---|---|
| Plain Bash | No package install; naturally sources functions; works with arrays and shell state; easiest path on Git Bash; adapter can also launch a future executable | Assertions, cleanup, capture, and diagnostics must be built carefully | Recommended |
| Bats | Familiar test syntax, setup/teardown, good failure display | New pinned dependency on three OSes; `run`/subshell semantics can obscure environment mutation; PTY still needs another tool; Windows/Git Bash support and installation add risk | Do not require initially |
| ShellSpec or similar | Rich matchers and reporting | Larger unfamiliar dependency and no advantage for this small single-file Bash project | Not justified |

The plain harness should provide `run_case`, `fail`, `assert_status`, `assert_file_empty`, `assert_text_file`, `assert_bytes_equal`, `assert_var_set/unset`, `assert_call_count`, and `assert_argv`. Commands must be invoked as arrays or direct function calls; do not use `eval`. Capture stdout, stderr, and status in separate files so trailing newlines and empty output are not lost through command substitution. Use `cmp` for byte equality and `diff -u` only for readable diagnostics.

The runner should support filters such as:

```text
bash tests/characterization/run.sh noninteractive
bash tests/characterization/run.sh platform
bash tests/characterization/run.sh case resolution::explicit_beats_active_and_local
```

## 4. Test harness architecture

Use one fresh sandbox and one fresh child Bash per test unless the case explicitly needs a sequence of state mutations. The top-level runner must never source `dstack.sh`, because doing so would contaminate later cases.

Each case should produce a result record with:

- command/adapter and input arguments;
- exit status or signal;
- exact stdout and stderr files;
- environment state before and after;
- relevant filesystem state before and after;
- ordered fake-Docker call records;
- platform, Bash version/path, and active shell options;
- classification label and source of expectation (README/help/code/observed probe).

The child invocation wrapper should default to `set +e` around the command under test, capture `$?`, then serialize state. Dedicated `errexit`, `nounset`, and `pipefail` cases should launch a second child with those options enabled and observe whether the post-command marker is reached. This avoids the harness's own strict mode changing ordinary expectations.

Two adapters should be anticipated:

- `bash-functions`: start Bash, source the absolute `dstack.sh`, invoke a named function in that same child, and serialize current-shell state;
- `external-command`: launch a program with the same fixture, argv, environment, cwd, and fake-Docker path.

Only the first exists today. Keeping fixture creation and result comparison outside the adapter makes most cases reusable later.

Representative harness tests:

- `harness::preserves_empty_argv_elements`
- `harness::captures_trailing_newlines_separately`
- `harness::records_nonzero_status_without_aborting`
- `harness::fake_docker_canary_prevents_real_cli`

## 5. Fake Docker strategy

Use an executable file named `docker` prepended to `PATH`, not only a Bash function. An executable covers current shell calls, `xargs docker stop`, child processes, and a future non-Bash implementation. A function is useful for ad hoc probes but is invisible to `xargs` and many future processes.

Give each test a unique control directory. The fake should:

1. require `DSTACK_TEST_DOCKER_GUARD` to match a token stored in that control directory;
2. allocate a monotonically increasing call number within the per-test directory;
3. write `argv.nul` as NUL-delimited arguments, preserving empty values, whitespace, newlines, and shell metacharacters;
4. write `argc`, physical cwd, and a NUL-delimited allowlist of relevant environment variables;
5. derive and record repeated Compose `-f` values without replacing the raw argv record;
6. optionally capture raw stdin only when `DSTACK_FAKE_CAPTURE_STDIN=1` and the test supplies a known EOF, avoiding accidental hangs;
7. read per-call response files such as `responses/001/stdout`, `stderr`, and `status`;
8. default to empty stdout/stderr and status 0 when no response is configured.

Per-call responses allow `dupdate`, for example, to make call 1 fail with status 17/stderr A and call 2 succeed with stdout B. Response files preserve exact bytes better than space-separated environment variables.

Record at least `HOME`, `DSTACK`, `DSTACK_COMPOSE_FILE`, `DSTACK_COMPOSE_FILES`, `DSTACK_BASES`, `PWD`, `USER`, `LC_ALL`, and the guard/control paths. Secrets and the full CI environment should not be recorded.

The fake should understand neither Docker nor Compose semantics. It records any argv beginning with direct Docker commands or `compose` and returns configured data. Tests, not the fake, assert meaning.

## 6. Fixture strategy

Each test creates a unique directory under `RUNNER_TEMP` or `mktemp -d` with:

```text
case-root/
  home/
    .config/dstack/
  bases/
    base-a/
    base-b/
  projects/
    local/
    active/
    registered/
  fake-bin/
  docker-control/
  capture/
```

Use helper functions to create an empty project, add a named Compose file, write a registry entry, make a nested stack, create a symlink when supported, and configure fake-Docker responses. Expected paths should be computed with the same platform's `pwd -P`, not hard-coded `/tmp` syntax.

Every case gets a fresh registry. Preserve a failed sandbox as a CI artifact only when explicitly requested; otherwise remove only the validated case-root path in a trap. Never clean a broad directory or the real home.

Fixture variants should include:

- no file, one standard file, all four standard files, glob-only files, and mixed `.yml`/`.yaml`;
- projects and files with spaces;
- nested `media/jellyfin` stacks;
- stale registry entries and an auto-discovered fallback of the same name;
- candidate and stack names colliding with `up`, `start`, `restart`, `logs`, and a service name;
- a symlinked registration target on platforms where symlink creation is reliable.

## 7. Environment isolation

Launch tests with an explicit environment allowlist. On Linux/macOS, `env -i` is suitable. On Windows/Git Bash, preserve only runner variables needed for Git Bash process startup (for example `SYSTEMROOT`, `COMSPEC`, `TEMP`, and the system path) and explicitly unset all DStack/Docker variables before setting test values.

For every ordinary case:

- set child `HOME` to `case-root/home` and `DSTACK_BASES` to fixture bases;
- set deterministic `USER`, `LC_ALL=C`, `TERM=dumb`, and controlled cwd;
- unset `DSTACK`, `DSTACK_COMPOSE_FILE`, and `DSTACK_COMPOSE_FILES` unless the case needs them;
- set `DOCKER_CONFIG` inside the sandbox and `DOCKER_HOST` to a nonexistent sandbox socket;
- prepend `fake-bin` to a curated system `PATH`, clear Bash's command hash, and verify `type -P docker` equals the fake path;
- prepend pager/formatter stubs only for cases explicitly using them;
- source DStack through an absolute repository path.

Run source-state and environment-mutation assertions inside the same child that invokes the function. Run direct-execution and isolation assertions from the parent against separate children. This provides both current-process observability and protection from cross-test contamination.

## 8. Loading and execution tests

Mandatory cases:

- `load::source_succeeds_and_sets_current_version`
- `load::source_sets_global_version_and_empty_deprecated_array`
- `load::source_leaves_existing_dstack_context_unchanged`
- `load::source_defines_all_43_public_commands`
- `load::source_defines_required_internal_helpers`
- `load::source_preserves_default_flags_and_ifs`
- `load::source_preserves_errexit_nounset_pipefail_matrix`
- `load::source_preserves_unrelated_shopt_state`
- `load::dhelp_disables_preexisting_nullglob` (**possible bug/quirk**)
- `load::direct_execution_no_args_is_silent_success`
- `load::direct_execution_dversion_is_silent_success` (**compatibility-sensitive / undecided**)
- `load::sourced_dversion_prints_version`
- `load::dhelp_is_generated_from_source_comments`
- `load::logging_color_depends_on_stdout_tty` (PTY tier)

Record `DSTACK_VERSION` separately from `dversion` output. The exact version must be updated deliberately with releases; the relationship between the variable and output is the more durable assertion.

The Bash 3 guard should be an `unsafe/deferred` case until CI deliberately provisions Bash 3 in an isolated environment. Do not fake `BASH_VERSINFO`. The supported matrix should assert and print that its selected interpreter is Bash 4+.

## 9. Stack registration tests

Mandatory registration cases:

- `registry::list_creates_empty_registry`
- `registry::help_does_not_create_registry`
- `registry::xdg_config_home_is_ignored`
- `registry::register_valid_stack_writes_name_equals_physical_path`
- `registry::register_symlink_uses_pwd_p` (Unix platform tier)
- `registry::duplicate_registration_replaces_and_moves_entry`
- `registry::same_name_new_path_replaces_old_path`
- `registry::invalid_directory_reports_error_and_returns_zero`
- `registry::directory_without_compose_reports_error_and_returns_zero`
- `registry::list_prints_stale_entries_without_validation`
- `registry::unregister_removes_exact_awk_field_match`
- `registry::unregister_missing_registry_returns_zero`
- `registry::unregister_unknown_name_returns_zero`
- `registry::registered_path_with_spaces_fails_current_file_probe` (**possible bug/quirk**)
- `registry::nested_physical_path_round_trips`

Unusual-name cases should be isolated and classified, not treated as contracts:

- spaces and shell glob characters in names;
- regex characters affecting `grep -v "^$name="`;
- `=` affecting the two-field `awk -F=` representation;
- embedded newline, which is valid to Bash but corrupts the line format.

Test deterministic regex/equals cases after the core suite. Embedded-newline and concurrent-write cases are lower priority. For concurrency, first create an optional stress test that proves no real files escape the sandbox; do not make a race outcome a required cross-platform golden result.

## 10. Stack resolution tests

Use three projects with different Compose filenames so the selected context is visible in fake-Docker argv.

Mandatory precedence cases:

- `resolution::explicit_registered_beats_active_and_local`
- `resolution::explicit_discovered_beats_active_and_local`
- `resolution::active_beats_local`
- `resolution::local_used_without_explicit_or_active`
- `resolution::local_checks_exact_directory_not_parent`
- `resolution::invalid_active_is_unset_then_local_used`
- `resolution::invalid_active_retains_selector_during_fallback` (**possible bug/quirk**)
- `resolution::stale_registry_falls_through_to_discovery`
- `resolution::registered_valid_entry_beats_discovery_same_name`
- `resolution::default_base_order_is_preserved`
- `resolution::custom_bases_replace_defaults_and_are_whitespace_split`
- `resolution::custom_base_with_spaces_is_split` (**possible bug/quirk**)
- `resolution::nested_name_with_separator_resolves_base_relative_path`
- `resolution::leaf_name_does_not_find_nested_stack`
- `resolution::missing_first_token_becomes_compose_argument_when_context_exists`
- `resolution::missing_context_logs_error_and_returns_zero`

Collision cases are high priority because `_is_compose_verb` is unused:

- `resolution::stack_named_up_consumes_dcompose_default_verb`
- `resolution::stack_named_start_consumes_dstart_injected_verb`
- `resolution::stack_named_restart_changes_drestart_one_arg_branch`
- `resolution::service_name_matching_stack_is_consumed_as_context`
- `resolution::drun_requires_second_token_before_first_can_be_stack`

The separate resolver family requires equivalent explicit/active/local and invalid-active cases for `drecompose`, `drebootstack`, `dupdate`, `drebuild`, and `drebuildnocache`. `drestart` also needs all four parser branches, including two arguments whose first token does not resolve.

## 11. Compose-file discovery tests

Test `_dstack_compose_files`, `_dstack_find_compose_file`, resolver output, and at least one public command. Internal cases diagnose where a public result originates; public cases remain the compatibility oracle.

Required cases:

- `compose_discovery::no_candidates_internal_failure`
- `compose_discovery::each_standard_filename_is_recognized`
- `compose_discovery::standard_order_is_docker_yml_docker_yaml_compose_yml_compose_yaml`
- `compose_discovery::glob_yml_precedes_glob_yaml_after_standard_names`
- `compose_discovery::standard_glob_duplicates_are_removed`
- `compose_discovery::glob_order_under_lc_all_c`
- `compose_discovery::one_candidate_is_automatic`
- `compose_discovery::multiple_candidates_need_selector_or_tty`
- `compose_discovery::custom_candidates_replace_defaults`
- `compose_discovery::custom_relative_values_are_not_joined_to_project` (**possible bug/quirk**)
- `compose_discovery::custom_nonexistent_values_enter_array_resolver` (**possible bug/quirk**)
- `compose_discovery::custom_absolute_value_behavior`
- `compose_discovery::candidate_path_with_spaces_array_vs_find_helper` (**possible bug/quirk**)

Run ordering tests with `LC_ALL=C`; add a platform test using each runner's normal locale to expose host differences without making them accidental failures in core tests.

## 12. Compose-selector tests

Use a fixture with at least four ordered candidates. Assert internal stdout/stderr/status and public fake-Docker argv separately.

- `compose_selector::index_one_selects_first`
- `compose_selector::last_index_selects_last`
- `compose_selector::zero_and_out_of_range_error`
- `compose_selector::all_emits_every_candidate_in_order`
- `compose_selector::comma_list_preserves_requested_order_and_deduplicates`
- `compose_selector::basename_matches_candidate`
- `compose_selector::relative_candidate_name_matches`
- `compose_selector::existing_slash_path_outside_candidates_is_accepted`
- `compose_selector::absolute_existing_path_is_accepted`
- `compose_selector::invalid_name_and_path_log_stdout_and_return_one_internal`
- `compose_selector::empty_selector_uses_auto_or_prompt_path`
- `compose_selector::whitespace_is_removed_from_items`
- `compose_selector::filename_containing_spaces_cannot_be_selected_by_name` (**possible bug/quirk**)
- `compose_selector::empty_comma_items_and_duplicate_items` (**compatibility-sensitive / undecided**)
- `compose_selector::numeric_prefix_syntax_behavior` (**compatibility-sensitive / undecided**)
- `compose_selector::public_invalid_selector_can_become_error_text_file_flag` (**possible bug/quirk**)
- `compose_selector::empty_candidates_flag_helper_returns_success_empty` (**possible bug/quirk**)

Do not rewrite error text into stderr in expectations. Capture the current streams exactly.

## 13. Compose-file precedence tests

Create candidates A/B/C and set `DSTACK_COMPOSE_FILE` to B so precedence is unambiguous.

- `compose_precedence::leading_short_file_overrides_environment`
- `compose_precedence::leading_long_file_overrides_environment`
- `compose_precedence::leading_compose_file_alias_overrides_environment`
- `compose_precedence::repeated_leading_flags_create_repeated_docker_f_pairs`
- `compose_precedence::comma_value_creates_repeated_docker_f_pairs`
- `compose_precedence::environment_used_without_leading_flag`
- `compose_precedence::no_environment_one_file_auto_selects`
- `compose_precedence::no_selector_no_select_forces_first`
- `compose_precedence::custom_candidate_list_changes_index_meaning`
- `compose_precedence::flag_before_subcommand_is_selector`
- `compose_precedence::logs_follow_flag_after_subcommand_is_forwarded`
- `compose_precedence::missing_leading_file_value_reports_success_error`
- `compose_precedence::active_selector_also_applies_to_different_explicit_stack` (**compatibility-sensitive / undecided**)

The central exact comparison is:

```text
dcompose -f compose.custom.yml logs  -> Docker gets -f RESOLVED_CUSTOM logs
dcompose logs -f             -> Docker gets -f AUTO_FILE logs -f
```

Assert NUL-delimited argv, not a reconstructed command string.

## 14. Argument-forwarding tests

Build a table-driven set for public wrappers. Every listed function gets at least one exact argv test; ambiguous functions get zero/one/two/many-argument cases.

| Commands | Required observations |
|---|---|
| `dcompose` | zero-argument default; explicit stack; arbitrary subcommand/options; selector flags; positional filename reaches Docker as command |
| `dsvc`, `dstart`, `dstop`, `ddown` | injected verb position, explicit/implicit context, stack-name collision |
| `ddownv` | no Docker before acceptance; exact `down -v` after acceptance |
| `dpull`, `dconfig` | max-argument validation, explicit/active/local context, `pull` or `config` suffix |
| `drestart` | zero args; one stack; one service; stack+service; two non-resolving tokens; more-than-two usage |
| `drebuild` | one service; stack+service; two tokens with non-resolving first token; stderr suppression |
| `drebuildnocache` | zero/one stack; too many arguments; exact `build --no-cache` then `up -d` |
| `dlogs`, `dllogs` | default/leading numeric tail count, explicit stack, pager boundary for `dllogs` |
| `dlog`, `dllog` | last/penultimate positional parsing, optional count claims, one-argument `dllog` failure |
| `dexec` | final token service, preceding context, exact `exec SERVICE sh` |
| `drun` | no service, service only, explicit stack, many command args, configured default command |

At least one `dcompose` and one `drun` case must include all of: `"a b"`, an empty argument, `-leading`, literal `*`, literal `$HOME`, `;`, `|`, `&&`, and a single `sh -c` payload. The expected argv must preserve each original boundary. Do not use `eval` to construct or invoke these inputs.

Representative names:

- `argv::dcompose_preserves_empty_and_space_arguments`
- `argv::drun_preserves_shell_metacharacters_as_literal_arguments`
- `argv::drestart_two_unknown_tokens_restarts_all_observed_quirk`
- `argv::drebuild_two_unknown_tokens_ignores_second_observed_quirk`
- `argv::dllog_one_argument_reports_bash_slice_error`

## 15. Multi-step command tests

For every row, assert call count, order, full argv, cwd, selected environment, outputs, and final status for response sequences `[0,0]`, `[17,0]`, and `[0,23]`.

| Command | Expected ordered Docker calls |
|---|---|
| `drecompose` | `compose ... down -v`; `compose ... up -d` |
| `drebootstack` | `compose ... down`; `compose ... up -d` |
| `dupdate` | `compose ... pull`; `compose ... up -d` |
| `drebuild` | `compose ... build SERVICE`; `compose ... up -d SERVICE`, both Docker stderr redirected away |
| `drebuildnocache` | `compose ... build --no-cache`; `compose ... up -d`, both Docker stderr redirected away |
| `dstackpurge` | after confirmation: `compose ... down -v`; direct `system prune -f` |

Current `|| true` placement means later calls generally continue after earlier failures and final status is normally 0. Record success messages that still appear after failed calls as observed behavior, not endorsement.

Also test no-candidate behavior in the direct flag-resolver family. Because empty flags can appear successful, the current implementation may call `docker compose` without `-f`; this is a possible bug/quirk requiring explicit evidence.

## 16. Direct-Docker helper tests

Exact fake-Docker argv is cheap, so give every direct helper a smoke assertion, then use representative deeper pipeline cases.

- Container/info: `dps`, `dpsa`, `dpsg`, `dport`, `dip`, `dstats`, `dinspect`.
- Image/volume: `dimg`, `dvol`, `dvolrm`, `dvolinspect`.
- Network: `dnet`, `dnetinspect`.
- Cleanup/prune: `dclean`, `dcleani`, `dcleanall`, `dprunewhat`.
- System-wide stop: `dstopall`.
- Compose logs are covered under `dlogs`, `dlog`, `dllogs`, and `dllog` rather than treated as direct Docker.

Required deeper cases:

- `direct::dps_formats_fake_table_through_sed_and_column`
- `direct::dps_pipeline_status_with_and_without_pipefail`
- `direct::dpsg_case_insensitive_match_and_no_match_success`
- `direct::dinspect_pipes_exact_docker_bytes_to_less_stub`
- `direct::dstopall_records_ps_then_xargs_stop_ids`
- `platform::dstopall_empty_ps_xargs_behavior`
- `direct::forced_prune_argv_is_exact`
- `direct::unconfirmed_cleanup_reaches_only_guarded_fake_docker`

Use a fake `less` executable for deterministic core tests; it should record argv/stdin and optionally pass bytes through. Test real `column`, `sed`, `grep`, `xargs`, and `less` only in a platform tier. Provide configurable failing stubs to characterize downstream failure and caller `pipefail` without depending on a utility being absent.

## 17. Return-status matrix

The first implementation pass should confirm this proposed matrix and encode the actual result, even where it is surprising.

| Case | Current expected observation | Classification |
|---|---:|---|
| source and sourced `dversion` | 0 | contract candidate |
| direct `./dstack.sh dversion` | 0, no output | undecided |
| invalid `dstack add`, missing stack, most usage errors | 0 | possible bug/quirk |
| `drun` with no arguments | 1 | observed behavior |
| `_dstack_resolve` missing name | 1 | internal diagnostic |
| `_dstack_resolve_compose_files` invalid selector | 1 with error on stdout | possible bug/quirk |
| `_dstack_resolve_compose_flags` no candidates | 0 with empty output | possible bug/quirk |
| public `_dcompose` missing context | 0 | possible bug/quirk |
| non-TTY multiple files through public command | normally 0; may invoke fake Docker with error text as `-f` | possible bug/quirk |
| cancelled/EOF confirmation | 0 and no Docker call | contract candidate for safety; status undecided |
| fake Docker failure through `_dcompose` wrapper | 0 | compatibility-sensitive / undecided |
| first or second multi-step Docker failure | later call still occurs; final normally 0 | compatibility-sensitive / undecided |
| `drebuild*` Docker failure | 0; Docker stderr suppressed | possible bug/quirk |
| direct `dport`, `dstats`, `dimg`, `dvol`, `dclean`, `dcleani`, `dprunewhat`, `dnet` failure | fake Docker status | observed behavior |
| `dip`, `dvolrm`, `dvolinspect`, `dnetinspect`, `dstopall`, `dcleanall` Docker failure | 0 due to `|| true` | observed behavior |
| `dps`/`dpsa` upstream failure, default caller | downstream status | shell-sensitive |
| `dps`/`dpsa` upstream failure with caller `pipefail` | nonzero pipeline status unless later behavior masks it | shell-sensitive |
| `dllogs`/`dllog` | pager pipeline status, affected by caller `pipefail` | shell-sensitive |
| `dinspect` pipeline failure | 0 due to trailing `|| true` | observed behavior |

For `set -e`, run commands in a child script with a marker after the call. Assert child status and marker presence. Pair each with `set +e`; repeat pipeline representatives with `pipefail` off/on and selected missing-argument functions with `nounset` off/on.

## 18. stdout/stderr characterization

Capture streams independently for every high-priority case. Do not merge `2>&1` except in a test specifically characterizing merged terminal display.

Required stream cases:

- info, OK, warning, and error logger output in non-TTY mode (stdout);
- TTY logger output and ANSI escapes (PTY tier);
- selection list and `all` entry (stderr), selection errors (stdout), and `read -p` prompt bytes;
- confirmation warning, prompt, accepted/rejected/EOF results;
- fake Docker stdout/stderr passthrough for a normal wrapper;
- stderr suppression in `drebuild` and `drebuildnocache`;
- resolution probes whose stderr is redirected to `/dev/null`;
- paged bytes entering the fake `less` stdin versus bytes returned to caller;
- the non-TTY multi-file error captured by `mapfile` and reused in Docker argv.

Representative names:

- `streams::non_tty_logs_have_no_ansi`
- `streams::selector_menu_stderr_error_stdout`
- `streams::docker_stderr_passes_through_dcompose`
- `streams::drebuild_discards_docker_stderr`
- `streams::mapfile_captures_error_as_compose_filename`

## 19. Environment-mutation tests

These cases must call functions and inspect state in the same child Bash:

- `state::dstack_name_exports_physical_dstack_path`
- `state::dstack_name_selector_exports_both_variables`
- `state::dstack_name_f_selector_exports_both_variables`
- `state::select_without_selector_unsets_old_selector`
- `state::changing_stack_replaces_path_and_selector`
- `state::invalid_stack_selection_preserves_previous_state`
- `state::unregister_active_registered_only_stack_leaves_state` (**possible bug/quirk**)
- `state::invalid_active_context_unsets_dstack_only`
- `state::invalid_selector_does_not_clear_active_variables`
- `state::exported_context_is_visible_to_child_process`
- `state::plain_child_execution_cannot_mutate_parent`
- `state::dhelp_mutates_nullglob_only_after_call`

Record both value/presence (`declare -p`) and export attribute (`export -p` or visibility in a grandchild). Avoid testing mutations through command substitution, which would itself create a subshell and hide state.

## 20. Interactive and TTY testing strategy

Split interactive testing into two layers:

1. **Non-TTY input tests on all platforms**: pipes, redirected files, `/dev/null`, exact `y` and `Y` acceptance, `n`, `N`, `yes`, and blank rejection, EOF, and failed reads. These need no PTY. They also prove that piped selection input is ignored when `_dstack_resolve_compose_files` sees `! -t 0`.
2. **Real PTY tests in a separate Linux job initially**: multiple-file menu, numeric/comma/`all` choices, invalid/empty selection, Ctrl-D/EOF, prompt placement, terminal echo, ANSI logging, and a pager smoke case.

Bash alone cannot make `[[ -t 0 ]]` true. `script` is commonly available but has incompatible flags/output behavior across GNU/Linux, BSD/macOS, and Git Bash. `expect` would add another package. Bats does not solve PTY allocation by itself. The simplest controlled initial approach is a small Python 3 standard-library `pty` driver in the interactive tier, with an explicit timeout, scripted byte input, captured raw output, and child status. CI should provision/log a known Python 3 for that job rather than assuming it silently.

Python `pty` is Unix-only, so do not run that driver on Windows. Start with Linux as required. Add macOS as an informational job only after line-ending, echo, and terminal-control differences have explicit expectations. Keep Windows interactive coverage to non-TTY cases unless a reliable Git Bash PTY strategy is deliberately adopted.

PTY assertions should prefer semantic output fragments and fake-Docker records over a single raw transcript: PTYs may echo input and translate `\n` to `\r\n`. Exact ANSI/color bytes can have one focused Linux expectation.

## 21. Compatibility-sensitive behavior matrix

Every concrete item in `AGENTS.md` maps to coverage as follows:

| AGENTS.md observation | Coverage priority/tier | Contract treatment |
|---|---|---|
| sourced current-shell context; source versus execute | Mandatory load/state suite | Sourcing is a contract candidate; silent direct args remain undecided |
| explicit/active/local precedence and array forwarding | Mandatory cross-platform core | Strong contract candidates, supported by README and code |
| widespread success statuses and `|| true`; lone `drun` status 1 | Mandatory status matrix | Observed behavior; maintainer must decide case by case |
| direct statuses, caller `pipefail`, suppressed stderr | Mandatory status/stream tests plus platform pipeline cases | Compatibility-sensitive, not automatically contract |
| log levels on stdout; menus on stderr; process-substitution effect | Mandatory stream tests; prompts also PTY tier | Error-as-filename is a possible bug/quirk |
| formatted/paged output and attached follow/stats calls | Representative core with stubs; real utilities platform tier; pager PTY lower priority | Formatting may be public; terminal mechanics remain platform-specific |
| `dhelp` comment parsing controls discovery | Lower-priority legacy Bash test | Legacy implementation behavior, not cross-implementation contract |

No test label alone promotes an observation. Promotion to intentional contract requires a reviewed decision supported by user documentation or explicit maintainer intent.

## 22. Documentation-caveat coverage

| Caveat | Proposed case | Classification |
|---|---|---|
| README says multiple files default to first | `docs_caveat::selection_enabled_multi_file_tty_vs_non_tty`; compare with `--no-select` | possible bug/quirk or docs mismatch |
| README describes positional `dcompose STACK FILE` | `docs_caveat::positional_file_is_forwarded_as_compose_command` | possible bug/quirk/docs mismatch |
| `DSTACK_COMPOSE_FILES` example uses bare names | `docs_caveat::custom_bare_candidates_are_not_stack_relative` | possible bug/quirk/docs mismatch |
| `dstackunset` claims active state is cleared | `docs_caveat::unregister_registered_only_active_stack_retains_state` | possible bug |
| `dllog` default form and `dlog` optional count claim | `docs_caveat::dllog_one_arg_slice_error`; `docs_caveat::dlog_count_is_fixed_100` | possible bugs/docs mismatch |
| discovery depth wording | `docs_caveat::listing_includes_depth_one_and_two_only` | observed implementation; human decision on wording |
| minimal/POSIX dependency claim | platform smoke inventory for `column`, `less`, and utility flags | platform/documentation issue, not a unit contract |
| CI “Run version command” | static assertion showing direct call is silent plus real sourced version check | CI naming/coverage mismatch |

These tests establish today’s implementation. A later fix may intentionally update a possible-bug expectation and documentation together after review.

## 23. Existing CI analysis

The current workflow has one `lint-and-test` job with an `os` matrix of `ubuntu-latest`, `macos-latest`, and `windows-latest`. It runs on pushes to `main`/`dev` and pull requests targeting `main`. Matrix fail-fast remains the GitHub default.

Current setup and checks:

- Ubuntu uses its preinstalled Bash and installs ShellCheck with `apt`.
- macOS installs Homebrew `shellcheck` and `bash`, then appends the Homebrew prefix to `GITHUB_PATH` for subsequent steps. macOS's system Bash is normally 3.x, but the workflow intends later `bash` resolution to use Homebrew. It does not print or assert the resolved Bash path/version.
- Windows installs ShellCheck with Chocolatey under `shell: bash`. Bash itself comes from the hosted runner's Git for Windows installation; the workflow does not install or pin it.
- Lint runs `shellcheck -x dstack.sh` on all three.
- Syntax runs `bash -n dstack.sh` on all three.
- Executable-bit validation runs on Linux/macOS and is intentionally skipped on Windows.
- The final step is named `Run version command` and runs `./dstack.sh dversion`. Because no dispatcher exists, this only checks direct loading/parsing under the selected Bash; it does not invoke `dversion`, assert output, source the command library, or exercise Docker.

Runner image tags float, so exact Bash versions cannot be promised from this file. Before characterization tests, CI should log `command -v bash` and `bash --version`, then assert major version 4 or newer. No current step installs/configures Docker for tests, which is appropriate for the future fake-Docker suite.

## 24. Proposed CI test tiers

Do not change CI as part of this planning task. When tests are implemented, split feedback into clear jobs/tier commands:

### Static validation

Run on all three existing OSes: print/assert Bash path/version, ShellCheck, `bash -n`, executable bit where meaningful, `bash -c 'source ./dstack.sh; dversion'` with exact output, and the separately named direct-load observation. Keep the current direct call only if named accurately.

### Non-interactive characterization

Run the plain Bash suite on all three OSes with fake Docker and fake pager. Include load/source, fixtures, registry, resolution, Compose selection with non-TTY stdin, exact argv, state, streams, statuses, multi-step commands, direct-helper smoke tests, and non-TTY confirmations.

### Platform-specific characterization

Use the same three-OS matrix but select explicit platform expectations for symlinks/path forms, discovery utilities, formatter/pager availability, empty-input `xargs`, case sensitivity, and executable permissions. Differences should be named assertions or explicit documented skips, not broad `continue-on-error` masking.

### Interactive/PTY characterization

Use a separate Linux job with Python 3 PTY support and hard timeouts. Make it required once stable. Add macOS later as informational until its transcript differences are baselined. Do not run the Unix PTY driver on Windows.

Separate jobs make a syntax/lint failure distinguishable from an observed behavior regression and prevent PTY flakiness from obscuring core results. Upload captures and fake-Docker records only on failure.

## 25. Linux, macOS, and Windows/Git Bash strategy

Run unchanged on all three:

- sourcing/version/function inventory under the selected Bash 4+;
- registry behavior inside temporary `HOME`;
- explicit/active/local precedence using `DSTACK_BASES` fixtures;
- non-interactive Compose discovery/selectors/flag precedence under `LC_ALL=C`;
- exact fake-Docker argv, call ordering, environment, status masking, and stream capture;
- non-TTY confirmation inputs and destructive-command safety;
- most environment mutation and documentation-caveat cases.

Use platform expectations for:

- physical path syntax and Git Bash drive conversion;
- symlink creation/permissions and `pwd -P`;
- filesystem case behavior;
- `find -maxdepth/-mindepth`, `dirname --`, `column`, `less`, and `xargs` behavior;
- executable-bit checks;
- native line endings from PTYs and utility output;
- default base paths containing `C:/Users/...` under Git Bash.

GNU-sensitive behavior includes GNU `find`/`xargs` details and util-linux `column` formatting. macOS may use BSD variants. Git Bash supplies its own Unix tools and path translation. Core tests should stub pager/formatter behavior when the utility is not what is being measured, while the platform tier measures the real dependency explicitly.

Initially Linux-only: Python PTY interactions, exact ANSI transcript, real pager smoke, and any timing-sensitive concurrency experiment. Windows cannot reliably run Unix `pty`, and symlink tests may require privileges; report explicit skips with reasons.

## 26. Docker safety strategy

Safety is a suite invariant and a blocker for merging the test harness:

1. Create the fake executable before launching any test child.
2. Set a per-case guard token and require the fake to validate it.
3. Prepend the fake directory, run `hash -r`, and assert `type -P docker` equals the expected physical fake path immediately before every command under test.
4. Run a canary fake invocation and require its record before any destructive-command case.
5. Set `DOCKER_HOST` to a nonexistent sandbox socket and `DOCKER_CONFIG` to the sandbox as defense in depth if a real CLI is ever resolved.
6. Abort the entire tier if the fake is absent, non-executable, resolves differently, or cannot record. Never “skip and continue” a destructive case after this failure.
7. Ensure child processes and `xargs` inherit the same controlled `PATH` and guard.
8. Never configure a Docker service, mount a daemon socket, or use the runner's Docker context.

Destructive helpers should first be tested with cancellation and zero fake calls, then acceptance and exact fake calls. The fake must make `ddownv`, `drecompose`, `dstackpurge`, `dstopall`, `dvolrm`, `dclean`, `dcleani`, `dcleanall`, and arbitrary destructive `dcompose` argv harmless.

## 27. Future implementation comparison strategy

Define test cases in terms of inputs and result records, not Bash function bodies. Reuse fixture creation, fake Docker, capture, filesystem snapshots, and comparisons through adapters.

Separate conceptual suites:

- **shell integration / legacy Bash**: sourcing, function inventory, current-shell exports, `IFS`, shopt, caller options, help-comment parsing;
- **CLI/core behavior**: arguments, statuses, stdout/stderr, prompts, registry operations, discovery and selector decisions;
- **Docker argv behavior**: ordered exact calls, cwd, selected files, relevant environment;
- **filesystem/state behavior**: registry contents, canonical paths, created temp files, mutations;
- **environment mutation behavior**: current Bash exports; explicitly legacy unless a future adapter provides equivalent shell integration.

Cross-implementation cases should feed equivalent fixtures and compare the structured result. Legacy tests continue to guard the Bash version but do not automatically constrain a future executable's internals. Possible bugs/quirks should appear in a reviewed divergence ledger with one of `preserve`, `fix intentionally`, or `decision pending`; avoid silently weakening both adapters' assertions.

Never normalize argument boundaries, statuses, or streams. For readable golden files, replace only the known case-root prefix with `<CASE_ROOT>` after retaining a raw record for exact comparison. Compute host path representations explicitly.

## 28. Proposed test directory layout

Do not create this layout until implementation is requested:

```text
tests/
  characterization/
    run.sh
    cases/
      load.bash
      registry.bash
      resolution.bash
      compose-discovery.bash
      compose-selectors.bash
      compose-precedence.bash
      argv.bash
      state.bash
      streams-status.bash
      multi-step.bash
      direct-docker.bash
      documentation-caveats.bash
    helpers/
      assertions.bash
      capture.bash
      environment.bash
      fixtures.bash
      docker-records.bash
      adapters.bash
    stubs/
      docker
      less
      configurable-filter
    platform/
      common.bash
      linux.bash
      macos.bash
      windows-git-bash.bash
    interactive/
      cases.bash
      pty_driver.py
    expected/
      platform/
      transcripts/
    decisions/
      behavior-classification.tsv
      implementation-divergences.tsv
```

Fixtures should be generated per case rather than committed as large trees. Commit only tiny static input/output bytes when generation would obscure the assertion.

## 29. Suggested implementation phases

1. Build and self-test the plain Bash assertion/capture runner.
2. Build fake Docker, response sequencing, NUL-record parser, canary, and daemon-safety checks. Do not add destructive command cases until this passes.
3. Add isolated environment and fixture helpers, including temp `HOME`, bases, cwd, registry, path normalization, and cleanup validation.
4. Add source/direct-load, version, function inventory, shell-option, `IFS`, and `dhelp` cases.
5. Add registry and stack-resolution precedence, including invalid active/stale entries and collision cases.
6. Add Compose discovery, selector, precedence, non-TTY, stream, and mapfile-error cases.
7. Add table-driven exact argv tests for all Compose wrappers and special argument boundaries.
8. Add environment mutation and export-inheritance tests.
9. Add the return-status/options matrix.
10. Add multi-step success/failure sequencing and destructive cancellation/acceptance cases.
11. Add exact direct-helper smoke tests, then formatting/pager/filter stubs.
12. Run non-interactive core on Linux, then macOS and Windows/Git Bash; record explicit platform expectations and skips.
13. Add platform utility integration cases.
14. Add the Linux PTY driver and interactive tier with timeouts; consider macOS only after stabilization.
15. Integrate static, non-interactive, platform, and PTY jobs into CI in a separate task.

Early phases prioritize a trustworthy safety boundary and the resolution/argv behaviors most likely to change during refactoring.

## 30. Criteria for sufficient characterization before refactoring

Blockers before a major architecture refactor:

- fake-Docker safety self-tests pass and no test can resolve the real Docker CLI;
- source/direct behavior and the full public function inventory are covered;
- explicit > active > local precedence and the separate multi-step resolver family are covered;
- registry creation/update/removal, active environment state, stale entries, and nested names are covered;
- default/custom Compose discovery, ordering, selectors, multi-file TTY/non-TTY behavior, and all selector-precedence paths are covered;
- exact Docker argv and boundaries are asserted for every command named in the forwarding matrix and a smoke case exists for every direct helper;
- ordered calls and fail-first/fail-second results are covered for every multi-step command;
- high-priority status and stdout/stderr matrices pass with default options and selected `errexit`/`nounset`/`pipefail` combinations;
- every `AGENTS.md` documentation caveat has a test or an explicit deferred reason;
- cancellation prevents all destructive fake calls, acceptance reaches only the fake, and system-wide helpers are represented;
- non-interactive suites pass on Ubuntu, macOS, and Windows/Git Bash, with reviewed platform-specific expectations rather than silent exclusions;
- required Linux PTY cases cover multi-file selection and confirmations;
- each observed quirk is labeled and has a human decision state; none is implicitly declared permanent.

Useful later, but not blockers:

- exhaustive malformed/newline/regex stack names;
- deterministic concurrent registry-race testing;
- Bash 3 execution in a dedicated legacy environment;
- macOS PTY transcript parity and Windows ConPTY support;
- exhaustive real-utility formatting permutations;
- real Docker integration, which is outside characterization safety and should remain a separate opt-in concern if ever needed.

## 31. Known difficult or unresolved cases

- **Contract decisions:** README support suggests sourcing, resolution precedence, argument preservation, and confirmation safety are contract candidates. Maintainers still need to decide whether silent direct execution, success-masked failures, stdout errors, and first-token collisions should survive a rewrite.
- **Bash 3:** the version guard's `exit` consequence is clear from code but should not be called runtime-verified until an actual Bash 3 child is used.
- **Registry concurrency:** fixed `registry.tmp` has race potential, but reproducible orchestration without instrumenting production code is difficult. Keep it optional and isolated.
- **Path edge cases:** embedded newlines, `=`, regex names, symlinks, case folding, and Git Bash path conversion need narrow platform fixtures. Some are better evidence of an unescaped format than future contracts.
- **Interactive bytes:** PTYs echo input and translate line endings. Assert decision/result and fake-Docker records first; exact transcripts need platform-specific baselines.
- **Pager behavior:** DStack hard-codes `less`; deterministic core tests should stub it. Full-screen interaction is low value and hard to make portable.
- **Long-running commands:** `dlogs`, `dstats`, and real pagers would block with a real CLI. Fake responses must terminate, and every interactive process needs a timeout.
- **External utilities:** `column`, `find`, `xargs`, and option support may expose real supported-platform differences. Do not hide missing commands with the core stubs; surface them in the platform tier.
- **CI Bash selection:** `*-latest` images and `GITHUB_PATH` are not version pins. CI must print/assert the actual interpreter before interpreting platform failures.
- **Future interface:** current-shell environment mutation is inherently a shell-integration test. It should remain a legacy Bash test unless a future implementation deliberately provides an equivalent integration layer.

This plan intentionally does not fix or normalize any current behavior. The first implemented suite should record the Bash baseline, review each classification, and only then become a gate for refactoring.
