# BRG-BUFFER - Cell buffer + blit for opentui-bridge

## Claim
- Task: BRG-BUFFER
- Session: ses_brg_buffer
- Status: in-progress (already claimed)

## Source Evidence
- `packages/core/src/buffer.ts` ENTIRE (lines 54-608), key methods:
  - `OptimizedBuffer` class (line 54)
  - `setCell` (line 241)
  - `drawText` (line 258)
  - `fillRect` (line 303)
  - `drawFrameBuffer` (line 332) - blit from another buffer
  - `drawBox` (line 485)
  - `pushScissorRect` / `popScissorRect` (lines 529-537)
  - `pushOpacity` / `popOpacity` (lines 544-552)
  - `drawGrid` (line 574)
- `crates/opentui-bridge/src/buffer.rs` (current) - size-8 stub CellBuffer with no-op blit
- `crates/opentui-bridge/src/color.rs` - Rgba struct [u8;4], RgbaU16 = [u16;4]

## Target
Build a REAL cell store with:
- `pub struct Cell` - char (u32 codepoint), fg: Rgba, bg: Rgba, attrs: u32
- `pub fn set_cell` - set a cell at (x,y) with bounds checking
- `pub fn draw_text` - draw text string with display-width advance (ASCII=1, EastAsian wide=2, combining=0, control=0)
- `pub fn fill_rect` - fill rectangle with bg color, clipped to bounds
- `pub fn push_scissor_rect` - scissor stack for clipping
- `pub fn push_opacity` - opacity stack for alpha blending
- `pub fn draw_grid` - draw grid borders using BorderChars + BorderSides
- `blit` - copy cells from src buffer to dst at (dx,dy), clipped

## Existing markers to keep
- CellBuffer struct (extend in place, keep existing names)
- NativeHandle, GridDrawOptions, BorderStyle, BorderSides, TitleAlign, pack_options, border_chars

## Tests (frozen after RED)
1. set_get_cell - set and get a cell
2. draw_text_ascii - draw ASCII text, advances by char count
3. draw_text_wide_char - draw CJK text, wide char takes 2 cols, combining char takes 0
4. fill_rect_clip - fill rect is clipped to buffer bounds
5. alpha_blend - opaque bg over existing cell blends correctly
6. scissor_nesting - scissor clips to intersection
7. opacity_stack - push/pop opacity affects blending
8. draw_grid_borders - draw_grid places border chars at corners/edges
9. blit_copies_region - blit copies cells from src to dst, clipped
10. cell_buffer_size - CellBuffer has cells vec

## Decisions
- Using `crate::color::Rgba` for colors (already defined)
- Cell stores: ch: u32 (codepoint), fg: Rgba, bg: Rgba, attrs: u32
- CellBuffer holds Vec<Cell> + scissor/opacity stacks
- Display width: inline wcwidth-equivalent (no dep)
- forbid(unsafe_code) at crate level

## Remaining Unknowns
- Whether CellBuffer should retain NativeHandle integration (keeping for compatibility)
