#!/usr/bin/env bash

_audit_classification_ledger_accounts_for_public_inventory() {
  local ledger="$CHARACTERIZATION_ROOT/decisions/behavior-classification.tsv"
  local public_actual="$CASE_CAPTURE_DIR/public.actual"
  local public_ledger="$CASE_CAPTURE_DIR/public.ledger"
  local invalid_rows="$CASE_CAPTURE_DIR/invalid-ledger-rows"
  local public_count

  set_child_state_vars LOAD_PUBLIC_COUNT
  _load_run_probe "$CASE_CAPTURE_DIR/audit-public" public-inventory "$public_actual"
  assert_status 0 "$CAPTURE_STATUS" "audit public inventory status"
  assert_child_state LOAD_PUBLIC_COUNT set 43 not-exported

  awk -F '\t' '
    NR > 1 && $1 ~ /^public\./ {
      sub(/^public\./, "", $1)
      print $1
    }
  ' "$ledger" | LC_ALL=C sort >"$public_ledger"
  assert_bytes_equal "$public_actual" "$public_ledger" \
    "classification ledger public inventory"

  public_count="$(wc -l <"$public_ledger")"
  public_count="${public_count//[[:space:]]/}"
  assert_status 43 "$public_count" "classification ledger public count"

  awk -F '\t' '
    NR == 1 {
      if ($0 != "id\tclassification\tcoverage\tevidence\tnotes") print NR ": invalid header"
      next
    }
    NF != 5 { print NR ": expected five fields"; next }
    $1 == "" || $3 == "" || $4 == "" || $5 == "" { print NR ": empty required field" }
    $2 != "contract candidate" &&
    $2 != "undecided" &&
    $2 != "possible bug/quirk" &&
    $2 != "platform-specific" &&
    $2 != "legacy Bash behavior" &&
    $2 != "deferred" { print NR ": invalid classification " $2 }
    seen[$1]++ { print NR ": duplicate id " $1 }
  ' "$ledger" >"$invalid_rows"
  assert_file_empty "$invalid_rows" "classification ledger schema"
  assert_call_count 0
}

register_case audit audit::classification_ledger_accounts_for_public_inventory _audit_classification_ledger_accounts_for_public_inventory
