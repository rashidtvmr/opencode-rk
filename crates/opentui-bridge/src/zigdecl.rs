//! Audited `extern C` declaration inventory: TS-bridge-called surface of `lib.zig`.
//!
//! SOURCE EVIDENCE (upstream, read-only):
//! - Zig exports: `packages/native/src/lib.zig` (4003 lines, 369 unique `export fn`).
//! - TS bridge table: `packages/core/src/zig.ts` (399 symbol entries, bun:ffi arities).
//! - Intersection (TS-bridge-called surface): **337** decls (this file's inventory).
//! - TS-only 62 (yoga ~60 + stream/spanFeed ~12 via yoga.zig/native-span-feed,
//!   not lib.zig exports). Zig-only 32 (image* ~17, kitty-image-transport,
//!   hyperlink caps; not called via the zig.ts table).
//! - Key Zig lines: createRenderer:1154, setTerminalEnvVar:1210, setUseThread:1217,
//!   setClearOnShutdown:1222, destroyRenderer:1227, setBackgroundColor:1257,
//!   setRenderOffset:1262, getNextBuffer:1337, getCurrentBuffer:1342,
//!   getBufferWidth:1371, getBufferHeight:1376, render:1394 (`renderer.RenderStatus`,
//!   renderer.zig:29-33: rendered=0, skipped=1, failed=2),
//!   createOptimizedBuffer:1444, destroyOptimizedBuffer:1472, setCursorPosition:1493,
//!   setCursorColor:1604, getCursorState:1667, clearTerminal:1708,
//!   setTerminalTitle:1713, bufferClear:1740, bufferWriteResolvedChars:1791,
//!   bufferDrawText:1800 (fg non-null, bg nullable), bufferSetCell:1817,
//!   bufferFillRect:1827, bufferDrawGrid:1990, bufferDrawBox:2015,
//!   bufferResize:2064, resizeRenderer:2069, enableMouse:2134, disableMouse:2139,
//!   queryPixelResolution:2144, enableKittyKeyboard:2154, disableKittyKeyboard:2159,
//!   setupTerminal:2174, suspendRenderer:2179, resumeRenderer:2184,
//!   createTextBuffer:2208..textBufferGetPlainText:2344,
//!   editBufferSetText:2771..editBufferReplaceText:2782.
//!
//! ABI FACTS: `NativeHandle = handles.Handle = u32` (0 = invalid); Zig `bool`
//! is 1 byte (Rust `bool`); nullable `?[*]` maps to nullable raw pointers;
//! `color: [*]const u16` is RGBA `[u16; 4]`; TS `buffer` args are borrowed
//! TypedArray pointers (length passed separately or fixed RGBA4).
//!
//! BOUNDARY: standalone file (`rustc --edition 2021 --test`, no crate deps).
//! `#[cfg(feature = "native")]` gates `#[link]` extern blocks so the test
//! binary links without libopentui. This file declares + documents; it does
//! NOT port Zig logic. Full 337 names are a data inventory (names + domains);
//! exact `extern C` signatures cover the audited renderer/buffer/text core —
//! remaining domains are documented coarse (TS bun:ffi arity) pending re-audit.
//!
//! DECL_COVERAGE (domain -> count; sum = 337 = ZIGDECL_TOTAL):
//! | domain            | count | contents                                              |
//! |-------------------|-------|-------------------------------------------------------|
//! | audio             |    41 | audio* engine/stream/device/mixer fns                 |
//! | clipboard         |    18 | clipboard* service/operation/result fns               |
//! | embedded_terminal |    15 | embeddedTerminal* encode/resize/scroll fns            |
//! | edit_buffer       |    39 | editBuffer* cursor/insert/delete/undo fns             |
//! | editor_view       |    38 | editorView* viewport/selection/wrap fns               |
//! | text_buffer       |    55 | textBuffer* incl. textBufferView* fns                 |
//! | buffer            |    33 | buffer* draw/fill/cell/opacity/scissor fns            |
//! | hitgrid           |     3 | hitGrid* scissor fns                                  |
//! | syntax_style      |     3 | syntaxStyle* register/resolve fns                     |
//! | link              |     2 | linkAlloc, linkGetUrl                                 |
//! | attributes        |     2 | attributesGetLinkId, attributesWithLink               |
//! | render            |     2 | render, rendererSetPaletteState                       |
//! | other             |    86 | create*/destroy*/terminal/mouse/stats/misc fns        |
//!
//! #![forbid(unsafe_code)]
//!
//! NOTE: `#![forbid(unsafe_code)]` covers the pure inventory below. The audited
//! `unsafe extern C` declaration blocks are gated behind `feature = "native"`
//! (off for the standalone test build); enabling that feature requires
//! removing this forbid (declarations are unsafe by nature).

/// Zig `NativeHandle` (= `handles.Handle`): 32-bit slot. 0 = invalid.
pub type NativeHandle = u32;
/// Zig `INVALID_HANDLE`.
pub const INVALID_HANDLE: NativeHandle = 0;

/// `bufferedDestinationKind`: process stdout.
pub const DEST_STDOUT: u8 = 0;
/// `bufferedDestinationKind`: in-memory output.
pub const DEST_MEMORY: u8 = 1;

/// `remoteModeValue`: auto-detect.
pub const REMOTE_AUTO: u8 = 0;
/// `remoteModeValue`: force local.
pub const REMOTE_LOCAL: u8 = 1;
/// `remoteModeValue`: force remote.
pub const REMOTE_REMOTE: u8 = 2;

/// Zig `renderer.RenderStatus` discriminant, returned as `u8` by `render`.
pub type RenderStatus = u8;
/// Render produced output.
pub const NATIVE_RENDER_STATUS_RENDERED: RenderStatus = 0;
/// Render skipped (nothing changed).
pub const NATIVE_RENDER_STATUS_SKIPPED: RenderStatus = 1;
/// Render failed (e.g. invalid handle).
pub const NATIVE_RENDER_STATUS_FAILED: RenderStatus = 2;

/// Owned render-status discriminant (fail-closed: unknown bytes map to Failed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeRenderStatus {
    Rendered,
    Skipped,
    Failed,
}

impl NativeRenderStatus {
    /// Map a raw status byte; unknown values fail closed to [`Self::Failed`].
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Rendered,
            1 => Self::Skipped,
            _ => Self::Failed,
        }
    }

    /// Back to the Zig discriminant.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Rendered => NATIVE_RENDER_STATUS_RENDERED,
            Self::Skipped => NATIVE_RENDER_STATUS_SKIPPED,
            Self::Failed => NATIVE_RENDER_STATUS_FAILED,
        }
    }
}

/// Validation failures for the pure marker helpers below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclError {
    ZeroSize,
    BadDestination,
    InvalidHandle,
}

/// Border sides bitmask (buffer.ts:18-52 `packDrawOptions`, lib.zig:2032-2041:
/// bit0 left, bit1 bottom, bit2 right, bit3 top).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorderSides(pub u32);

impl BorderSides {
    pub const NONE: BorderSides = BorderSides(0);
    pub const LEFT: BorderSides = BorderSides(1 << 0);
    pub const BOTTOM: BorderSides = BorderSides(1 << 1);
    pub const RIGHT: BorderSides = BorderSides(1 << 2);
    pub const TOP: BorderSides = BorderSides(1 << 3);
    pub const ALL: BorderSides = BorderSides(0b1111);
}

/// Title alignment selector (bits 5-6 title, bits 7-8 bottom title).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TitleAlign {
    Left = 0,
    Center = 1,
    Right = 2,
}

/// Pack draw options exactly like TS `packDrawOptions`:
/// bits 0-3 sides, bit 4 fill, bits 5-6 title align, bits 7-8 bottom-title align.
#[must_use]
pub const fn pack_options(
    sides: BorderSides,
    fill: bool,
    title: TitleAlign,
    bottom_title: TitleAlign,
) -> u32 {
    (sides.0 & 0xF) | ((fill as u32) << 4) | ((title as u32) << 5) | ((bottom_title as u32) << 7)
}

/// Domain counts; sum must equal `ALL_SYMBOLS.len()` (checked by frozen test).
pub const DOMAIN_AUDIO: usize = 41;
pub const DOMAIN_CLIPBOARD: usize = 18;
pub const DOMAIN_EMBEDDED_TERMINAL: usize = 15;
pub const DOMAIN_EDIT_BUFFER: usize = 39;
pub const DOMAIN_EDITOR_VIEW: usize = 38;
pub const DOMAIN_TEXT_BUFFER: usize = 55;
pub const DOMAIN_BUFFER: usize = 33;
pub const DOMAIN_HITGRID: usize = 3;
pub const DOMAIN_SYNTAX_STYLE: usize = 3;
pub const DOMAIN_LINK: usize = 2;
pub const DOMAIN_ATTRIBUTES: usize = 2;
pub const DOMAIN_RENDER: usize = 2;
pub const DOMAIN_OTHER: usize = 86;

/// Total TS-bridge-called declarations.
pub const ZIGDECL_TOTAL: usize = 337;

/// Full TS-bridge-called inventory, sorted unique (domain-grouped counts above).
pub const ALL_SYMBOLS: &[&str] = &[
    "addToCurrentHitGridClipped",
    "addToHitGrid",
    "attributesGetLinkId",
    "attributesWithLink",
    "audioClearCaptureDeviceSelection",
    "audioClearPlaybackDeviceSelection",
    "audioCloseStream",
    "audioCreateGroup",
    "audioCreateStream",
    "audioEnableTap",
    "audioEndStream",
    "audioGetCaptureDeviceCount",
    "audioGetCaptureDeviceName",
    "audioGetCaptureStats",
    "audioGetPlaybackDeviceCount",
    "audioGetPlaybackDeviceName",
    "audioGetStats",
    "audioGetStreamStats",
    "audioIsCaptureDeviceDefault",
    "audioIsCaptureRunning",
    "audioIsPlaybackDeviceDefault",
    "audioLoad",
    "audioMixToBuffer",
    "audioPlay",
    "audioReadCapture",
    "audioReadTap",
    "audioRefreshCaptureDevices",
    "audioRefreshPlaybackDevices",
    "audioRestartStream",
    "audioSelectCaptureDevice",
    "audioSelectPlaybackDevice",
    "audioSetGroupVolume",
    "audioSetMasterVolume",
    "audioSetStreamGroup",
    "audioSetStreamPan",
    "audioSetStreamVolume",
    "audioSetVoiceGroup",
    "audioStart",
    "audioStartCapture",
    "audioStartMixer",
    "audioStop",
    "audioStopCapture",
    "audioStopVoice",
    "audioUnload",
    "audioWriteStream",
    "bufferClear",
    "bufferClearOpacity",
    "bufferClearScissorRects",
    "bufferColorMatrix",
    "bufferColorMatrixUniform",
    "bufferDrawBox",
    "bufferDrawChar",
    "bufferDrawEditorView",
    "bufferDrawGrayscaleBuffer",
    "bufferDrawGrayscaleBufferSupersampled",
    "bufferDrawGrid",
    "bufferDrawImage",
    "bufferDrawPackedBuffer",
    "bufferDrawSuperSampleBuffer",
    "bufferDrawText",
    "bufferDrawTextBufferView",
    "bufferFillRect",
    "bufferGetAttributesPtr",
    "bufferGetBgPtr",
    "bufferGetCharPtr",
    "bufferGetCurrentOpacity",
    "bufferGetFgPtr",
    "bufferGetId",
    "bufferGetRealCharSize",
    "bufferGetRespectAlpha",
    "bufferPopOpacity",
    "bufferPopScissorRect",
    "bufferPushOpacity",
    "bufferResize",
    "bufferSetCell",
    "bufferSetCellWithAlphaBlending",
    "bufferSetRespectAlpha",
    "bufferWriteResolvedChars",
    "checkHit",
    "clearClipboardOSC52",
    "clearCurrentHitGrid",
    "clearPendingSplitFooterTransition",
    "clearTerminal",
    "clipboardClearOperationStart",
    "clipboardOperationCancel",
    "clipboardOperationDestroy",
    "clipboardOperationPoll",
    "clipboardOperationResultDataCopy",
    "clipboardOperationResultDataLength",
    "clipboardOperationResultDiagnosticCopy",
    "clipboardOperationResultDiagnosticLength",
    "clipboardOperationResultErrorCode",
    "clipboardOperationResultMimeCopy",
    "clipboardOperationResultMimeLength",
    "clipboardReadOperationStart",
    "clipboardServiceBeginShutdown",
    "clipboardServiceCreate",
    "clipboardServiceDestroy",
    "clipboardServiceDrain",
    "clipboardServicePollShutdown",
    "clipboardWriteOperationStart",
    "commitSplitFooterSnapshot",
    "copyToClipboardOSC52",
    "createAudioEngine",
    "createEditBuffer",
    "createEditorView",
    "createEmbeddedTerminal",
    "createEventSink",
    "createNativeRenderable",
    "createNativeSpanFeed",
    "createOptimizedBuffer",
    "createRenderer",
    "createSyntaxStyle",
    "createTextBuffer",
    "createTextBufferView",
    "destroyAudioEngine",
    "destroyEditBuffer",
    "destroyEditorView",
    "destroyEmbeddedTerminal",
    "destroyEventSink",
    "destroyNativeRenderable",
    "destroyOptimizedBuffer",
    "destroyRenderer",
    "destroySyntaxStyle",
    "destroyTextBuffer",
    "destroyTextBufferView",
    "disableKittyKeyboard",
    "disableMouse",
    "drawFrameBuffer",
    "dumpBuffers",
    "dumpHitGrid",
    "dumpOutputBuffer",
    "editBufferCanRedo",
    "editBufferCanUndo",
    "editBufferClear",
    "editBufferClearHistory",
    "editBufferDebugLogRope",
    "editBufferDeleteChar",
    "editBufferDeleteCharBackward",
    "editBufferDeleteLine",
    "editBufferDeleteRange",
    "editBufferGetCursorPosition",
    "editBufferGetEOL",
    "editBufferGetId",
    "editBufferGetLineStartOffset",
    "editBufferGetNextWordBoundary",
    "editBufferGetPrevWordBoundary",
    "editBufferGetText",
    "editBufferGetTextBuffer",
    "editBufferGetTextRange",
    "editBufferGetTextRangeByCoords",
    "editBufferGotoLine",
    "editBufferInsertChar",
    "editBufferInsertText",
    "editBufferMoveCursorDown",
    "editBufferMoveCursorLeft",
    "editBufferMoveCursorRight",
    "editBufferMoveCursorUp",
    "editBufferNewLine",
    "editBufferOffsetToPosition",
    "editBufferPositionToOffset",
    "editBufferRedo",
    "editBufferReplaceText",
    "editBufferReplaceTextFromMem",
    "editBufferSetCursor",
    "editBufferSetCursorByOffset",
    "editBufferSetCursorToLineCol",
    "editBufferSetTabWidth",
    "editBufferSetText",
    "editBufferSetTextFromMem",
    "editBufferUndo",
    "editorViewConvertSelectionToCell",
    "editorViewDeleteSelectedText",
    "editorViewGetCursor",
    "editorViewGetEOL",
    "editorViewGetLineInfoDirect",
    "editorViewGetLogicalLineInfoDirect",
    "editorViewGetNextWordBoundary",
    "editorViewGetPrevWordBoundary",
    "editorViewGetSelectedTextBytes",
    "editorViewGetSelection",
    "editorViewGetText",
    "editorViewGetTextBufferView",
    "editorViewGetTotalVirtualLineCount",
    "editorViewGetViewport",
    "editorViewGetVirtualLineCount",
    "editorViewGetVisualCursor",
    "editorViewGetVisualEOL",
    "editorViewGetVisualSOL",
    "editorViewGotoVisualLineEnd",
    "editorViewMoveDownVisual",
    "editorViewMoveUpVisual",
    "editorViewResetLocalSelection",
    "editorViewResetSelection",
    "editorViewSetCursorByOffset",
    "editorViewSetLocalSelection",
    "editorViewSetPlaceholderStyledText",
    "editorViewSetScrollMargin",
    "editorViewSetSelection",
    "editorViewSetSelectionColors",
    "editorViewSetSelectionInclusive",
    "editorViewSetSelectionOccupancy",
    "editorViewSetTabIndicator",
    "editorViewSetTabIndicatorColor",
    "editorViewSetViewport",
    "editorViewSetViewportSize",
    "editorViewSetWrapMode",
    "editorViewUpdateLocalSelection",
    "editorViewUpdateSelection",
    "embeddedTerminalClearSelection",
    "embeddedTerminalCompose",
    "embeddedTerminalCursor",
    "embeddedTerminalDrainResponses",
    "embeddedTerminalEncodeFocus",
    "embeddedTerminalEncodeKey",
    "embeddedTerminalEncodeMouse",
    "embeddedTerminalEncodePaste",
    "embeddedTerminalGetSelectedText",
    "embeddedTerminalInvalidate",
    "embeddedTerminalResize",
    "embeddedTerminalScroll",
    "embeddedTerminalSetSelection",
    "embeddedTerminalSetTransparentBackground",
    "embeddedTerminalWrite",
    "enableKittyKeyboard",
    "enableMouse",
    "encodeUnicode",
    "freeUnicode",
    "getAllocatorStats",
    "getArenaAllocatedBytes",
    "getBufferHeight",
    "getBufferWidth",
    "getBufferWidthMethod",
    "getBuildOptions",
    "getCurrentBuffer",
    "getCursorState",
    "getHitGridDirty",
    "getKittyKeyboardFlags",
    "getNextBuffer",
    "getRenderStats",
    "getSplitOutputOffset",
    "getTerminalCapabilities",
    "hitGridClearScissorRects",
    "hitGridPopScissorRect",
    "hitGridPushScissorRect",
    "linkAlloc",
    "linkGetUrl",
    "nativeRenderableAttachYogaNode",
    "nativeRenderableSetMeasureTarget",
    "processCapabilityResponse",
    "queryPixelResolution",
    "queryThemeColors",
    "render",
    "rendererSetPaletteState",
    "repaintSplitFooter",
    "resetSplitScrollback",
    "resizeRenderer",
    "restoreTerminalModes",
    "resumeRenderer",
    "setBackgroundColor",
    "setClearOnShutdown",
    "setCursorColor",
    "setCursorPosition",
    "setCursorStyleOptions",
    "setDebugOverlay",
    "setKittyKeyboardFlags",
    "setLogCallback",
    "setPendingSplitFooterTransition",
    "setRenderOffset",
    "setTerminalEnvVar",
    "setTerminalTitle",
    "setUseThread",
    "setupTerminal",
    "suspendRenderer",
    "syncSplitScrollback",
    "syntaxStyleGetStyleCount",
    "syntaxStyleRegister",
    "syntaxStyleResolveByName",
    "textBufferAddHighlight",
    "textBufferAddHighlightByCharRange",
    "textBufferAppend",
    "textBufferAppendFromMemId",
    "textBufferClear",
    "textBufferClearAllHighlights",
    "textBufferClearLineHighlights",
    "textBufferClearMemRegistry",
    "textBufferFreeLineHighlights",
    "textBufferGetByteSize",
    "textBufferGetHighlightCount",
    "textBufferGetLength",
    "textBufferGetLineCount",
    "textBufferGetLineHighlightsPtr",
    "textBufferGetPlainText",
    "textBufferGetTabWidth",
    "textBufferGetTextRange",
    "textBufferGetTextRangeByCoords",
    "textBufferLoadFile",
    "textBufferRegisterMemBuffer",
    "textBufferRemoveHighlightsByRef",
    "textBufferReplaceMemBuffer",
    "textBufferReset",
    "textBufferResetDefaults",
    "textBufferSetDefaultAttributes",
    "textBufferSetDefaultBg",
    "textBufferSetDefaultFg",
    "textBufferSetStyledText",
    "textBufferSetSyntaxStyle",
    "textBufferSetTabWidth",
    "textBufferSetTextFromMem",
    "textBufferViewGetLineInfoDirect",
    "textBufferViewGetLogicalLineInfoDirect",
    "textBufferViewGetPlainText",
    "textBufferViewGetSelectedText",
    "textBufferViewGetSelectionInfo",
    "textBufferViewGetSelectionOccupancy",
    "textBufferViewGetVirtualLineCount",
    "textBufferViewMeasureForDimensions",
    "textBufferViewResetLocalSelection",
    "textBufferViewResetSelection",
    "textBufferViewSetFirstLineOffset",
    "textBufferViewSetLocalSelection",
    "textBufferViewSetSelection",
    "textBufferViewSetSelectionOccupancy",
    "textBufferViewSetTabIndicator",
    "textBufferViewSetTabIndicatorColor",
    "textBufferViewSetTextAlign",
    "textBufferViewSetTruncate",
    "textBufferViewSetViewport",
    "textBufferViewSetViewportSize",
    "textBufferViewSetWrapMode",
    "textBufferViewSetWrapWidth",
    "textBufferViewUpdateLocalSelection",
    "textBufferViewUpdateSelection",
    "triggerNotification",
    "updateMemoryStats",
    "updateStats",
    "writeOut"];

/// Pure validator mirroring `createRenderer` arg checks (lib.zig:1154,
/// renderer.ts:3766-3783): nonzero size, destination 0=stdout/1=memory.
pub fn create_renderer(width: u32, height: u32, dest: u8) -> Result<(), DeclError> {
    if width == 0 || height == 0 {
        return Err(DeclError::ZeroSize);
    }
    if dest != DEST_STDOUT && dest != DEST_MEMORY {
        return Err(DeclError::BadDestination);
    }
    Ok(())
}

/// Pure validator mirroring `resizeRenderer` (lib.zig:2069): nonzero size.
pub fn resize_renderer(width: u32, height: u32) -> Result<(), DeclError> {
    if width == 0 || height == 0 {
        return Err(DeclError::ZeroSize);
    }
    Ok(())
}

/// Map a raw `render()` status byte to [`NativeRenderStatus`] (fail-closed).
#[must_use]
pub fn render_native(status: u8) -> NativeRenderStatus {
    NativeRenderStatus::from_u8(status)
}

/// Pure validator mirroring `setupTerminal` (lib.zig:2174): live handle required.
pub fn setup_terminal(handle: NativeHandle, _use_alternate_screen: bool) -> Result<(), DeclError> {
    if handle == INVALID_HANDLE {
        return Err(DeclError::InvalidHandle);
    }
    Ok(())
}

/// Pure validator mirroring `destroyRenderer` (lib.zig:1227): live handle required.
pub fn destroy_renderer(handle: NativeHandle) -> Result<(), DeclError> {
    if handle == INVALID_HANDLE {
        return Err(DeclError::InvalidHandle);
    }
    Ok(())
}

// ---- Audited `unsafe extern C` declarations (renderer/buffer/text core) ----
// SAFETY: each symbol below is an audited `export fn` of lib.zig with the cited
// line; callers must pass live handles, nullable pointers only where Zig takes
// `?[*]`, and RGBA color pointers to 4 valid u16s. Gated on `feature="native"`
// so the standalone test build links without libopentui.

/// Renderer lifecycle + terminal core (lib.zig:1154-2179).
#[cfg(feature = "native")]
#[link(name = "opentui")]
unsafe extern "C" {
    /// Safety: null feed_ptr selects the buffered backend.
    fn createRenderer(
        width: u32,
        height: u32,
        bufferedDestinationKind: u8,
        remoteModeValue: u8,
        feedPtr: *const core::ffi::c_void,
    ) -> NativeHandle;
    /// Safety: handle must be a live owned renderer; must not be used afterwards.
    fn destroyRenderer(renderer_handle: NativeHandle, flush_input: bool);
    /// Safety: handle must be a live renderer.
    fn setupTerminal(renderer_handle: NativeHandle, useAlternateScreen: bool);
    /// Safety: handle must be a live renderer.
    fn suspendRenderer(renderer_handle: NativeHandle);
    /// Safety: handle must be a live renderer.
    fn resumeRenderer(renderer_handle: NativeHandle);
    /// Safety: handle must be a live renderer.
    fn restoreTerminalModes(renderer_handle: NativeHandle);
    /// Safety: handle must be a live renderer.
    fn resizeRenderer(renderer_handle: NativeHandle, width: u32, height: u32);
    /// Safety: handle must be a live renderer.
    fn setUseThread(renderer_handle: NativeHandle, useThread: bool);
    /// Safety: handle must be a live renderer.
    fn setClearOnShutdown(renderer_handle: NativeHandle, clear: bool);
    /// Safety: handle must be a live renderer.
    fn setCursorPosition(renderer_handle: NativeHandle, x: i32, y: i32, visible: bool);
    /// Safety: handle must be a live renderer; titlePtr null iff titleLen is 0.
    fn setTerminalTitle(renderer_handle: NativeHandle, titlePtr: *const u8, titleLen: u32);
    /// Safety: handle must be a live renderer; ptrs null iff matching len is 0.
    fn setTerminalEnvVar(
        renderer_handle: NativeHandle,
        keyPtr: *const u8,
        keyLen: u32,
        valuePtr: *const u8,
        valueLen: u32,
    ) -> bool;
    /// Safety: handle must be a live renderer.
    fn getNextBuffer(renderer_handle: NativeHandle) -> NativeHandle;
    /// Safety: handle must be a live renderer.
    fn getCurrentBuffer(renderer_handle: NativeHandle) -> NativeHandle;
    /// Safety: handle must be a live buffer.
    fn getBufferWidth(buffer_handle: NativeHandle) -> u32;
    /// Safety: handle must be a live buffer.
    fn getBufferHeight(buffer_handle: NativeHandle) -> u32;
    /// Safety: handle must be a live renderer; returns RenderStatus discriminant.
    fn render(renderer_handle: NativeHandle, force: bool) -> u8;
    /// Safety: handle must be a live renderer; color must point to 4 valid u16s.
    fn setBackgroundColor(renderer_handle: NativeHandle, color: *const u16);
    /// Safety: handle must be a live renderer; color must point to 4 valid u16s.
    fn setCursorColor(renderer_handle: NativeHandle, color: *const u16);
    /// Safety: handle must be a live renderer.
    fn setRenderOffset(renderer_handle: NativeHandle, x: i32, y: i32);
    /// Safety: handle must be a live renderer.
    fn getCursorState(renderer_handle: NativeHandle) -> u8;
    /// Safety: handle must be a live renderer.
    fn clearTerminal(renderer_handle: NativeHandle);
    /// Safety: handle must be a live renderer.
    fn enableMouse(renderer_handle: NativeHandle, enableMovement: bool);
    /// Safety: handle must be a live renderer.
    fn disableMouse(renderer_handle: NativeHandle);
    /// Safety: handle must be a live renderer.
    fn enableKittyKeyboard(renderer_handle: NativeHandle, flags: u8);
    /// Safety: handle must be a live renderer.
    fn disableKittyKeyboard(renderer_handle: NativeHandle);
    /// Safety: handle must be a live renderer.
    fn queryPixelResolution(renderer_handle: NativeHandle);
}

/// Buffer core (lib.zig:1444-2064).
#[cfg(feature = "native")]
#[link(name = "opentui")]
unsafe extern "C" {
    /// Safety: width/height must be nonzero.
    fn createOptimizedBuffer(width: u32, height: u32) -> NativeHandle;
    /// Safety: handle must be a live owned buffer; must not be used afterwards.
    fn destroyOptimizedBuffer(buffer_handle: NativeHandle);
    /// Safety: handle must be a live buffer.
    fn bufferClear(buffer_handle: NativeHandle);
    /// Safety: handle must be a live buffer; outputPtr must fit outputLen bytes.
    fn bufferWriteResolvedChars(
        buffer_handle: NativeHandle,
        outputPtr: *mut u8,
        outputLen: u32,
        addLineBreaks: bool,
    ) -> u32;
    /// Safety: handle must be a live buffer; fg non-null RGBA[4], bg nullable RGBA[4].
    fn bufferDrawText(
        buffer_handle: NativeHandle,
        text: *const u8,
        textLen: u32,
        x: u32,
        y: u32,
        fg: *const u16,
        bg: *const u16,
        attributes: u32,
    );
    /// Safety: handle must be a live buffer.
    fn bufferSetCell(
        buffer_handle: NativeHandle,
        x: u32,
        y: u32,
        ch: u32,
        fg: *const u16,
        bg: *const u16,
        attributes: u32,
    );
    /// Safety: handle must be a live buffer; bg must point to 4 valid u16s.
    fn bufferFillRect(
        buffer_handle: NativeHandle,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        bg: *const u16,
    );
    /// Safety: handle must be a live buffer.
    fn bufferResize(buffer_handle: NativeHandle, width: u32, height: u32);
    /// Safety: handle must be a live buffer; char/fg/bg arrays must match lib.zig:1990 arity.
    fn bufferDrawGrid(
        buffer_handle: NativeHandle,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        borderChars: *const u32,
        borderFg: *const u16,
        borderBg: *const u16,
        options: u32,
    );
    /// Safety: handle must be a live buffer; titlePtr null iff titleLen is 0.
    fn bufferDrawBox(
        buffer_handle: NativeHandle,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        titlePtr: *const u8,
        titleLen: u32,
        options: u32,
    );
}

/// Text/edit-buffer core (lib.zig:2208-2782).
#[cfg(feature = "native")]
#[link(name = "opentui")]
unsafe extern "C" {
    /// Safety: returned handle is owned; destroy with destroyTextBuffer.
    fn createTextBuffer() -> NativeHandle;
    /// Safety: handle must be a live owned text buffer; must not be used afterwards.
    fn destroyTextBuffer(buffer_handle: NativeHandle);
    /// Safety: handle must be a live text buffer; out pointers need matching capacity.
    fn textBufferGetPlainText(
        buffer_handle: NativeHandle,
        outPtr: *mut u8,
        outLen: u32,
    ) -> u32;
    /// Safety: handle must be a live edit buffer.
    fn editBufferSetText(buffer_handle: NativeHandle, textPtr: *const u8, textLen: u32);
    /// Safety: handle must be a live edit buffer.
    fn editBufferReplaceText(
        buffer_handle: NativeHandle,
        start: u32,
        end: u32,
        textPtr: *const u8,
        textLen: u32,
    );
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_options_match_renderer_values() {
        assert_eq!(
            pack_options(BorderSides::ALL, false, TitleAlign::Left, TitleAlign::Left),
            0b1111
        );
        assert_eq!(
            pack_options(BorderSides::ALL, true, TitleAlign::Center, TitleAlign::Right),
            0b1111 | (1 << 4) | (1 << 5) | (2 << 7)
        );
    }

    #[test]
    fn handle_widths() {
        assert_eq!(core::mem::size_of::<NativeHandle>(), 4);
        assert_eq!(INVALID_HANDLE, 0);
    }

    #[test]
    fn coverage_table_self_consistent() {
        let sum = DOMAIN_AUDIO
            + DOMAIN_CLIPBOARD
            + DOMAIN_EMBEDDED_TERMINAL
            + DOMAIN_EDIT_BUFFER
            + DOMAIN_EDITOR_VIEW
            + DOMAIN_TEXT_BUFFER
            + DOMAIN_BUFFER
            + DOMAIN_HITGRID
            + DOMAIN_SYNTAX_STYLE
            + DOMAIN_LINK
            + DOMAIN_ATTRIBUTES
            + DOMAIN_RENDER
            + DOMAIN_OTHER;
        assert_eq!(sum, ALL_SYMBOLS.len());
        assert_eq!(ALL_SYMBOLS.len(), ZIGDECL_TOTAL);
        assert_eq!(ZIGDECL_TOTAL, 337);
    }

    #[test]
    fn status_enum_has_skipped_failed() {
        assert_eq!(NATIVE_RENDER_STATUS_RENDERED, 0);
        assert_eq!(NATIVE_RENDER_STATUS_SKIPPED, 1);
        assert_eq!(NATIVE_RENDER_STATUS_FAILED, 2);
        assert_eq!(NativeRenderStatus::from_u8(1), NativeRenderStatus::Skipped);
        assert_eq!(NativeRenderStatus::from_u8(9), NativeRenderStatus::Failed);
    }

    #[test]
    fn marker_validation() {
        assert_eq!(DEST_STDOUT, 0);
        assert_eq!(DEST_MEMORY, 1);
        assert_eq!(REMOTE_AUTO, 0);
        assert!(create_renderer(80, 24, DEST_STDOUT).is_ok());
        assert_eq!(create_renderer(0, 24, DEST_STDOUT), Err(DeclError::ZeroSize));
        assert_eq!(create_renderer(80, 24, 9), Err(DeclError::BadDestination));
        assert!(resize_renderer(80, 24).is_ok());
        assert_eq!(resize_renderer(0, 24), Err(DeclError::ZeroSize));
    }

    #[test]
    fn render_native_mapping() {
        assert_eq!(render_native(0), NativeRenderStatus::Rendered);
        assert_eq!(render_native(1), NativeRenderStatus::Skipped);
        assert_eq!(render_native(2), NativeRenderStatus::Failed);
        assert_eq!(render_native(255), NativeRenderStatus::Failed);
    }

    #[test]
    fn all_symbols_sorted_unique() {
        let mut sorted = ALL_SYMBOLS.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, ALL_SYMBOLS);
        sorted.dedup();
        assert_eq!(sorted.len(), ALL_SYMBOLS.len());
    }
}
