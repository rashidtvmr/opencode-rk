# BRIDGE-GAP-31 scratchpad
claim: ses_gap31 owns BRIDGE-GAP-31
sources: input_events.rs parse_one/PASTE markers, loop_events.rs MAX_PASTE/HostAction, input.rs InputEvent/Key
target: native_input.rs InputChunk+decode_bytes, std-only forbid(unsafe)
tests: quit_ctrl_c_and_d, arrows_decode_to_noop, paste_fenced, multibyte_coalesced, invalid_fail_closed, resize_passthrough, text_then_enter_submits
decisions: arrows fold Noop; printable coalesce; Enter submits pending; invalid/unterminated fail-closed unconsumed
