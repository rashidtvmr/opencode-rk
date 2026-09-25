# BRIDGE-PAR-278
claim: ses_par278
source: packages/tui/src/ui/dialog-confirm.tsx:19 DialogConfirm + DialogConfirmResult
target: crates/opentui-bridge/src/dialog_confirm_full.rs ConfirmDialog{title cap128,confirmed} ask/answer/result
tests: 5 (cap-reset,true,false,unicode,default)
decisions: trunc chars, tri-state Option<bool>
