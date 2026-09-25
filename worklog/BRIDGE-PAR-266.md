# Claim BRIDGE-PAR-266
- session ses_par266, scratchpad worklog/BRIDGE-PAR-266.md
- owned file: crates/opentui-bridge/src/session_footer_full2.rs (NEW, one file only)

## Source evidence
- crates/opentui-bridge/src/footer_assemble.rs:15 `assemble_footer(slot,&str,items:&[String],width:usize)->Vec<String>`; :26 `footer_height()->usize=1`
- crates/opentui-bridge/src/session_footer_full.rs:9 `SessionFooterFull{slot cap32, items cap16x128, busy}`; :37 `add_item->bool`; :46 `render(&self)->String`
- sibling pattern crates/opentui-bridge/src/footer_menu_full2.rs:8 `MenuFlow{menu,opened:u32}` wrapper + counter

## Observed scenario
- Need FooterFlow render-counting wrapper over assemble_footer. No existing renders counter.

## Target boundary
- ONE new file session_footer_full2.rs. Do NOT edit lib.rs, Cargo.toml, session_footer_full.rs, footer_assemble.rs. No cargo, no commit.
- API: FooterFlow{slot cap64, items cap16, renders:u64} + push(&mut,&str)->bool + render(&mut,width)->Vec<String> via assemble_footer + renders bump.
- std-only, forbid(unsafe_code), under 120 lines, >=4 tests. Verify rustfmt --check only.

## Tests
- in-file: slot_truncates_64, push_caps_16, render_bumps_count, render_empty_is_slot, render_joins_and_clips (5 tests)

## Decisions
- saturating_add for renders (no overflow wrap). Items stored verbatim (parent SessionFooterFull truncates 128; this lane spec caps count only, keep minimal per ponytail).
- render takes &mut self (counter bump), delegates to assemble_footer.

## Unknowns
- None. lib.rs prewire left to integrator.
