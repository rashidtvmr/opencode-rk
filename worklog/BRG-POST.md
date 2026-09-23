# BRG-POST — CPU-exact post filters bridge

## Claim
- Task: BRG-POST, session ses_brg_post, status in-progress (ledger confirmed).
- Owned file ONLY: crates/opentui-bridge/src/post.rs (standalone, rustc --test, no lib.rs touch).

## Source evidence (exact)
- /Users/mymac/Projects/opentui/packages/core/src/post/filters.ts:23 `applyScanlines` (gain matrix s, rows y+=step, bg-only target=2); :72 `applyInvert` (INVERT_MATRIX + colorMatrixUniform); :3 `toU8` = round(clamp01)*255; :7 `channel` = (v&0xff)/255; :11 `setRgb` preserves alpha.
- /Users/mymac/Projects/opentui/packages/core/src/post/matrices.ts:2 SEPIA_MATRIX (0.393/0.769/0.189; 0.349/0.686/0.168; 0.272/0.534/0.131; alpha identity); :251 GRAYSCALE_MATRIX (luminance 0.299/0.587/0.114 all rows); :271 INVERT_MATRIX (-1 diag + alpha column 1).
- /Users/mymac/Projects/opentui/packages/core/src/post/effects.ts — shader/needs-native cuts (NOT ported, documented in file header): DistortionEffect (glitch/shift/flip, :29), VignetteEffect (zero-matrix attenuation, :268), CloudsEffect (Perlin FBM, :494), FlamesEffect (Perlin FBM fire gradient, :605), CRTRollingBarEffect (:727). Vignette comment :369 confirms uniform blend semantics: result = orig + (xformed-orig)*strength.
- buffer.ts:308 colorMatrix (per-cell mask blend), :320 colorMatrixUniform (early return strength==0).

## Target boundary
- Self-contained RGBA Pixel type, std-only, #![forbid(unsafe_code)].
- CPU-exact: apply_scanlines, apply_invert, apply_grayscale, apply_sepia, apply_color_matrix (+masked), ColorMatrix, SEPIA const.
- Native colorMatrix blend: t = cell_strength * global_strength (unclamped; negative extrapolates like noise), output toU8-clamped, alpha via matrix row (identity rows preserve).
- Scanlines TS targets bg plane only; bridge documents caller selects plane.

## Tests (frozen after RED)
1. scanlines_darkens_step_rows 2. invert_exact 3. grayscale_coefficients 4. sepia_matrix_values (+white apply) 5. identity_noop 6. uniform_masked_agree.

## Decisions
- No Perlin/noise, no animated state — out of scope per task (shader-needing cuts).
- No lib.rs edit (owned-file-only rule; rustc --test compiles standalone).

## Remaining
- RED run -> GREEN impl -> cc.update completed + report. No commit/push.
