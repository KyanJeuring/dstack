#!/usr/bin/env bash

_direct_run() {
  local name="$1"
  shift

  _compose_run "$name" "$@"
}

_direct_container_helpers_exact_argv() {
  _status_install_cat_utility column
  _forward_install_pager

  _direct_run dps dps
  _direct_run dpsa dpsa
  _direct_run dpsg dpsg needle
  _direct_run dport dport
  _direct_run dip dip container-one
  _direct_run dstats dstats
  _direct_run dinspect dinspect container-one

  assert_call_count 7
  assert_argv 1 ps --format 'table {{.ID}}\t{{.Label "com.docker.compose.service"}}\t{{.Names}}\t{{.Status}}\t{{.Ports}}'
  assert_argv 2 ps -a --format 'table {{.ID}}\t{{.Label "com.docker.compose.service"}}\t{{.Names}}\t{{.Status}}\t{{.Ports}}'
  assert_argv 3 ps --format 'table {{.ID}}\t{{.Names}}\t{{.Status}}'
  assert_argv 4 ps --format 'table {{.Names}}\t{{.Ports}}'
  assert_argv 5 inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' container-one
  assert_argv 6 stats
  assert_argv 7 inspect container-one
}

_direct_image_volume_network_helpers_exact_argv() {
  _direct_run dimg dimg
  _direct_run dvol dvol
  _direct_run dvolrm dvolrm volume-one
  _direct_run dvolinspect dvolinspect volume-one
  _direct_run dnet dnet
  _direct_run dnetinspect dnetinspect network-one

  assert_call_count 6
  assert_argv 1 images --format 'table {{.Repository}}\t{{.Tag}}\t{{.Size}}'
  assert_argv 2 volume ls
  assert_argv 3 volume rm volume-one
  assert_argv 4 volume inspect volume-one
  assert_argv 5 network ls
  assert_argv 6 network inspect network-one
}

_direct_cleanup_helpers_exact_argv() {
  _direct_run dclean dclean
  _direct_run dcleani dcleani
  _direct_run dprunewhat dprunewhat

  assert_call_count 3
  assert_argv 1 container prune -f
  assert_argv 2 image prune -f
  assert_argv 3 system prune --dry-run
}

_direct_dstopall_forwards_all_ids_to_fake_only() {
  _status_configure_response 1 0 $'one\ntwo\n'
  _direct_run stop-all dstopall

  assert_status 0 "$CAPTURE_STATUS"
  assert_call_count 2
  assert_argv 1 ps -q
  assert_argv 2 stop one two
}

_direct_dcleanall_cancel_and_accept() {
  set_child_state_vars
  run_dstack_function "$CASE_CAPTURE_DIR/cleanall-cancel" dcleanall </dev/null
  assert_status 0 "$CAPTURE_STATUS"
  assert_call_count 0

  run_dstack_function_confirmed "$CASE_CAPTURE_DIR/cleanall-accept" dcleanall
  assert_status 0 "$CAPTURE_STATUS"
  assert_call_count 1
  assert_argv 1 system prune -a
}

_direct_ddownv_cancel_and_accept() {
  _forward_prepare_local
  set_child_state_vars
  run_dstack_function "$CASE_CAPTURE_DIR/downv-cancel" ddownv </dev/null
  assert_status 0 "$CAPTURE_STATUS"
  assert_call_count 0

  run_dstack_function_confirmed "$CASE_CAPTURE_DIR/downv-accept" ddownv
  assert_status 0 "$CAPTURE_STATUS"
  assert_call_count 1
  assert_argv 1 compose -f "$FORWARD_COMPOSE" down -v
}

register_case multi direct::container_information_helpers_exact_argv _direct_container_helpers_exact_argv
register_case multi direct::image_volume_network_helpers_exact_argv _direct_image_volume_network_helpers_exact_argv
register_case multi direct::cleanup_helpers_exact_argv _direct_cleanup_helpers_exact_argv
register_case multi direct::dstopall_sends_discovered_ids_only_to_fake_docker _direct_dstopall_forwards_all_ids_to_fake_only
register_case multi destructive::dcleanall_cancel_and_accept _direct_dcleanall_cancel_and_accept
register_case multi destructive::ddownv_cancel_and_accept _direct_ddownv_cancel_and_accept
