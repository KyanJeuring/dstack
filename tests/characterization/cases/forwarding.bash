#!/usr/bin/env bash

# Exact Docker argv is an intentional contract candidate. Cases named quirk
# record current parser behavior without deciding whether it should be kept.

_forward_prepare_local() {
  FORWARD_COMPOSE="$CASE_CURRENT_DIR/compose.yml"
  : >"$FORWARD_COMPOSE"
}

_forward_prepare_stack() {
  local name="${1:-app}"

  fixture_create_discovered_stack primary "$name" compose.yml 'services: {}'
  FORWARD_STACK="$FIXTURE_STACK"
  DSTACK_BASES="$FIXTURE_BASE"
  export DSTACK_BASES
}

_forward_run() {
  local name="$1"
  shift

  _compose_run "$name" "$@"
  assert_status 0 "$CAPTURE_STATUS"
}

_forward_install_pager() {
  cat >"$CASE_FAKE_BIN/less" <<'EOF'
#!/usr/bin/env bash
cat
EOF
  chmod +x "$CASE_FAKE_BIN/less"
  hash -r
}

_forward_dcompose_defaults() {
  _forward_prepare_local
  _forward_run dcompose-zero dcompose
  assert_call_count 1
  assert_argv 1 compose -f "$FORWARD_COMPOSE" up -d --build --remove-orphans

  reset_fake_docker_records
  _forward_prepare_stack app
  _forward_run dcompose-stack dcompose app
  assert_call_count 1
  assert_argv 1 compose -f "$FORWARD_STACK/compose.yml" up -d --build --remove-orphans
}

_forward_dcompose_preserves_hostile_arguments() {
  _forward_prepare_local
  _forward_run hostile dcompose "" "two words" -leading '*' '$HOME' ';' '|' '&&'
  assert_call_count 1
  assert_argv 1 compose -f "$FORWARD_COMPOSE" \
    "" "two words" -leading '*' '$HOME' ';' '|' '&&'
}

_forward_positional_compose_filename_is_command_quirk() {
  _forward_prepare_stack app
  _forward_run positional dcompose app compose.prod.yml
  assert_call_count 1
  assert_argv 1 compose -f "$FORWARD_STACK/compose.yml" compose.prod.yml
}

_forward_dsvc_appends_ps_services() {
  _forward_prepare_local
  _forward_run dsvc dsvc --all
  assert_call_count 1
  assert_argv 1 compose -f "$FORWARD_COMPOSE" --all ps --services
}

_forward_basic_lifecycle_wrappers() {
  _forward_prepare_local
  _forward_run dstart dstart web
  _forward_run dstop dstop web
  _forward_run ddown ddown web
  _forward_run dpull dpull
  _forward_run dconfig dconfig

  assert_call_count 5
  assert_argv 1 compose -f "$FORWARD_COMPOSE" web start
  assert_argv 2 compose -f "$FORWARD_COMPOSE" web stop
  assert_argv 3 compose -f "$FORWARD_COMPOSE" web down
  assert_argv 4 compose -f "$FORWARD_COMPOSE" pull
  assert_argv 5 compose -f "$FORWARD_COMPOSE" config
}

_forward_ddownv_after_confirmation() {
  _forward_prepare_local
  set_child_state_vars
  run_dstack_function_confirmed "$CASE_CAPTURE_DIR/ddownv" ddownv web
  assert_status 0 "$CAPTURE_STATUS"
  assert_file_empty "$CAPTURE_STDERR"
  assert_call_count 1
  assert_argv 1 compose -f "$FORWARD_COMPOSE" web down -v
}

_forward_drestart_argument_branches() {
  _forward_prepare_local
  _forward_prepare_stack app
  _forward_run restart-zero drestart
  _forward_run restart-service drestart web
  _forward_run restart-stack drestart app
  _forward_run restart-both drestart app worker

  assert_call_count 4
  assert_argv 1 compose -f "$FORWARD_COMPOSE" restart
  assert_argv 2 compose -f "$FORWARD_COMPOSE" restart web
  assert_argv 3 compose -f "$FORWARD_STACK/compose.yml" restart
  assert_argv 4 compose -f "$FORWARD_STACK/compose.yml" restart worker
}

_forward_rebuild_wrappers() {
  _forward_prepare_local
  _forward_prepare_stack app
  _forward_run rebuild-local drebuild web
  _forward_run rebuild-stack drebuild app worker
  _forward_run rebuild-no-cache drebuildnocache app

  assert_call_count 6
  assert_argv 1 compose -f "$FORWARD_COMPOSE" build web
  assert_argv 2 compose -f "$FORWARD_COMPOSE" up -d web
  assert_argv 3 compose -f "$FORWARD_STACK/compose.yml" build worker
  assert_argv 4 compose -f "$FORWARD_STACK/compose.yml" up -d worker
  assert_argv 5 compose -f "$FORWARD_STACK/compose.yml" build --no-cache
  assert_argv 6 compose -f "$FORWARD_STACK/compose.yml" up -d
}

_forward_rebuild_unresolved_two_tokens_ignores_second_quirk() {
  _forward_prepare_local
  _forward_run rebuild-unresolved drebuild missing ignored
  assert_call_count 2
  assert_argv 1 compose -f "$FORWARD_COMPOSE" build missing
  assert_argv 2 compose -f "$FORWARD_COMPOSE" up -d missing
}

_forward_dlogs_count_and_stack() {
  _forward_prepare_local
  _forward_prepare_stack app
  _forward_run dlogs-default dlogs
  _forward_run dlogs-count dlogs 25
  _forward_run dlogs-stack dlogs 7 app

  assert_call_count 3
  assert_argv 1 compose -f "$FORWARD_COMPOSE" logs -f --tail=100
  assert_argv 2 compose -f "$FORWARD_COMPOSE" logs -f --tail=25
  assert_argv 3 compose -f "$FORWARD_STACK/compose.yml" logs -f --tail=7
}

_forward_dlog_uses_final_service_and_fixed_count() {
  _forward_prepare_local
  _forward_prepare_stack app
  _forward_run dlog-local dlog web
  _forward_run dlog-stack dlog app worker

  assert_call_count 2
  assert_argv 1 compose -f "$FORWARD_COMPOSE" logs -f --tail=100 web
  assert_argv 2 compose -f "$FORWARD_STACK/compose.yml" logs -f --tail=100 worker
}

_forward_dllogs_through_pager() {
  _forward_prepare_local
  _forward_prepare_stack app
  _forward_install_pager
  _forward_run dllogs-local dllogs 30
  _forward_run dllogs-stack dllogs 5 app

  assert_call_count 2
  assert_argv 1 compose -f "$FORWARD_COMPOSE" logs --tail=30
  assert_argv 2 compose -f "$FORWARD_STACK/compose.yml" logs --tail=5
}

_forward_dllog_normal_forms() {
  _forward_prepare_local
  _forward_prepare_stack app
  _forward_install_pager
  _forward_run dllog-local dllog web 40
  _forward_run dllog-stack dllog app worker 9

  assert_call_count 2
  assert_argv 1 compose -f "$FORWARD_COMPOSE" logs --tail=40 web
  assert_argv 2 compose -f "$FORWARD_STACK/compose.yml" logs --tail=9 worker
}

_forward_dllog_one_argument_fails_before_docker_quirk() {
  _forward_prepare_local
  _forward_install_pager
  _compose_run dllog-one dllog web
  assert_status 1 "$CAPTURE_STATUS"
  assert_file_empty "$CAPTURE_STDOUT"
  if ! grep -Fq 'substring expression < 0' "$CAPTURE_STDERR"; then
    fail "dllog one-argument stderr did not report its negative slice"
  fi
  assert_call_count 0
}

_forward_dexec_uses_final_service() {
  _forward_prepare_local
  _forward_prepare_stack app
  _forward_run dexec-local dexec web
  _forward_run dexec-stack dexec app worker

  assert_call_count 2
  assert_argv 1 compose -f "$FORWARD_COMPOSE" exec web sh
  assert_argv 2 compose -f "$FORWARD_STACK/compose.yml" exec worker sh
}

_forward_drun_preserves_command_argv() {
  _forward_prepare_local
  _forward_run drun-hostile drun worker sh -c 'printf "%s" "$HOME"; * | cat && true' \
    "two words" "" -leading
  assert_call_count 1
  assert_argv 1 compose -f "$FORWARD_COMPOSE" run --rm worker sh -c \
    'printf "%s" "$HOME"; * | cat && true' "two words" "" -leading
}

_forward_drun_explicit_stack_and_arguments() {
  _forward_prepare_stack app
  _forward_run drun-stack drun app worker echo one two three
  assert_call_count 1
  assert_argv 1 compose -f "$FORWARD_STACK/compose.yml" run --rm worker echo one two three
}

register_case forwarding forwarding::dcompose_zero_and_one_stack_default_to_up _forward_dcompose_defaults
register_case forwarding forwarding::dcompose_preserves_hostile_argument_boundaries _forward_dcompose_preserves_hostile_arguments
register_case forwarding forwarding::positional_compose_filename_reaches_docker_as_command _forward_positional_compose_filename_is_command_quirk
register_case forwarding forwarding::dsvc_appends_ps_services _forward_dsvc_appends_ps_services
register_case forwarding forwarding::basic_lifecycle_wrappers_preserve_argument_order _forward_basic_lifecycle_wrappers
register_case forwarding forwarding::ddownv_forwards_after_confirmation _forward_ddownv_after_confirmation
register_case forwarding forwarding::drestart_argument_branches _forward_drestart_argument_branches
register_case forwarding forwarding::rebuild_wrappers_preserve_service_boundaries _forward_rebuild_wrappers
register_case forwarding forwarding::drebuild_unresolved_two_tokens_ignores_second _forward_rebuild_unresolved_two_tokens_ignores_second_quirk
register_case forwarding forwarding::dlogs_forwards_count_and_stack _forward_dlogs_count_and_stack
register_case forwarding forwarding::dlog_uses_final_service_and_fixed_tail _forward_dlog_uses_final_service_and_fixed_count
register_case forwarding forwarding::dllogs_forwards_through_pager _forward_dllogs_through_pager
register_case forwarding forwarding::dllog_normal_forms _forward_dllog_normal_forms
register_case forwarding forwarding::dllog_one_argument_has_negative_slice_error _forward_dllog_one_argument_fails_before_docker_quirk
register_case forwarding forwarding::dexec_uses_final_service _forward_dexec_uses_final_service
register_case forwarding forwarding::drun_preserves_hostile_command_argv _forward_drun_preserves_command_argv
register_case forwarding forwarding::drun_explicit_stack_preserves_arguments _forward_drun_explicit_stack_and_arguments
