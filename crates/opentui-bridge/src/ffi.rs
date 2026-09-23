#![forbid(unsafe_code)]
//! Portable FFI surface mirror (`platform/ffi.ts` @ opentui 4954312d)
//! + render-library path resolution (`zig.ts` setRenderLibPath/resolveRenderLib).
//!
//! Hot path is const-evaluatable and heap-free: type tags, pointer-range
//! checks and callback invalidation borrow statics only.

/// JS `Number.MAX_SAFE_INTEGER`: largest exactly-representable pointer.
pub const MAX_SAFE_POINTER: u64 = (1 << 53) - 1;

/// Opaque native pointer (mirrors branded `Pointer = number | bigint`).
/// Bun uses the numeric half, Node the bigint half; normalize at the
/// backend boundary, never in portable code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pointer(u64);

impl Pointer {
    /// Null pointer (mirrors Node `0n` for empty buffers / null args).
    pub const NULL: Pointer = Pointer(0);

    /// Wrap a raw address; `None` when it exceeds the safe-integer range
    /// (mirrors `POINTER_UNSAFE`).
    #[must_use]
    pub const fn from_raw(value: u64) -> Option<Pointer> {
        if value > MAX_SAFE_POINTER {
            return None;
        }
        Some(Pointer(value))
    }

    /// Wrap a signed address; `None` on negative (mirrors `POINTER_NEGATIVE`)
    /// or unsafe (mirrors `POINTER_UNSAFE`) input.
    #[must_use]
    pub const fn from_i64(value: i64) -> Option<Pointer> {
        if value < 0 {
            return None;
        }
        Pointer::from_raw(value as u64)
    }

    /// Byte address.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Add a byte offset; `None` on negative (mirrors
    /// `POINTER_OFFSET_NEGATIVE`) or overflowing/unsafe result.
    #[must_use]
    pub const fn offset_by(self, offset: i64) -> Option<Pointer> {
        if offset < 0 {
            return None;
        }
        let next = self.0 + offset as u64;
        if next < self.0 {
            return None;
        }
        Pointer::from_raw(next)
    }
}

/// Portable FFI type tags (mirrors the `FFIType` const map in `ffi.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FfiType {
    Char,
    I8,
    U8,
    I16,
    U16,
    I32,
    Int,
    U32,
    I64,
    U64,
    F64,
    F32,
    Bool,
    Ptr,
    Pointer,
    Void,
    CString,
    Function,
    Usize,
    Callback,
    NapiEnv,
    NapiValue,
    Buffer,
}

impl FfiType {
    /// Bun FFI type string, evaluatable at compile time.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            FfiType::Char => "char",
            FfiType::I8 => "int8_t",
            FfiType::U8 => "uint8_t",
            FfiType::I16 => "int16_t",
            FfiType::U16 => "uint16_t",
            FfiType::I32 => "int32_t",
            FfiType::Int => "int",
            FfiType::U32 => "uint32_t",
            FfiType::I64 => "int64_t",
            FfiType::U64 => "uint64_t",
            FfiType::F64 => "double",
            FfiType::F32 => "float",
            FfiType::Bool => "bool",
            FfiType::Ptr => "ptr",
            FfiType::Pointer => "pointer",
            FfiType::Void => "void",
            FfiType::CString => "cstring",
            FfiType::Function => "function",
            FfiType::Usize => "usize",
            FfiType::Callback => "callback",
            FfiType::NapiEnv => "napi_env",
            FfiType::NapiValue => "napi_value",
            FfiType::Buffer => "buffer",
        }
    }

    /// Pointer-like argument kinds needing normalization on the Node path
    /// (mirrors `isNodePointerArgumentType`: ptr/pointer/function/callback).
    #[must_use]
    pub const fn is_pointer_arg(self) -> bool {
        matches!(
            self,
            FfiType::Ptr | FfiType::Pointer | FfiType::Function | FfiType::Callback
        )
    }

    /// Node FFI ABI name (mirrors `toNodeFFIType`); `is_result` selects the
    /// result position where `cstring` is rejected (`NODE_STRING_RETURN`).
    pub const fn node_abi_name(self, is_result: bool) -> Result<&'static str, FfiError> {
        match self {
            FfiType::Char => Ok("char"),
            FfiType::I8 => Ok("i8"),
            FfiType::U8 => Ok("u8"),
            FfiType::I16 => Ok("i16"),
            FfiType::U16 => Ok("u16"),
            FfiType::I32 | FfiType::Int => Ok("i32"),
            FfiType::U32 => Ok("u32"),
            FfiType::I64 => Ok("i64"),
            FfiType::U64 => Ok("u64"),
            FfiType::F64 => Ok("f64"),
            FfiType::F32 => Ok("f32"),
            FfiType::Bool => Ok("bool"),
            FfiType::Ptr | FfiType::Pointer => Ok("pointer"),
            FfiType::Void => Ok("void"),
            FfiType::CString => {
                if is_result {
                    return Err(FfiError::StringReturn);
                }
                Ok("string")
            }
            // Pointer-like kinds all cross node:ffi as raw pointers.
            FfiType::Function | FfiType::Callback | FfiType::Buffer => Ok("pointer"),
            // Needs an ABI audit before Node support; u64 would force BigInt
            // call sites and u32 could truncate pointers.
            FfiType::Usize => Err(FfiError::UsizeUnsupported),
            // Bun N-API bridge types are not raw Node FFI pointers.
            FfiType::NapiEnv | FfiType::NapiValue => Err(FfiError::NapiUnsupported),
        }
    }
}

/// Portable FFI failures (mirrors the `*_MESSAGE` constants in `ffi.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiError {
    NegativePointer,
    UnsafePointer,
    NegativeOffset,
    UnsafeOffset,
    PointerOverride,
    ThreadsafeCallback,
    LibraryClosed,
    UsizeUnsupported,
    NapiUnsupported,
    StringReturn,
}

impl FfiError {
    /// Stable message (mirrors the exported message constants).
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            FfiError::NegativePointer => "Pointer must be non-negative",
            FfiError::UnsafePointer => "Pointer exceeds safe integer range",
            FfiError::NegativeOffset => "Pointer offset must be non-negative",
            FfiError::UnsafeOffset => "Pointer offset must be a safe integer",
            FfiError::PointerOverride => "Node FFI backend does not support FFIFunction.ptr overrides",
            FfiError::ThreadsafeCallback => {
                "Node FFI callbacks are same-thread only and do not support threadsafe callbacks"
            }
            FfiError::LibraryClosed => "Cannot create FFI callback after library.close() has been called",
            FfiError::UsizeUnsupported => "Node FFI backend does not support usize yet",
            FfiError::NapiUnsupported => "Node FFI backend does not support Bun N-API FFI types",
            FfiError::StringReturn => "Node FFI backend does not normalize string return values (yet)",
        }
    }
}

/// One native symbol (mirrors `FFIFunction`; `ptr` overrides the symbol
/// address, `threadsafe` requests a threadsafe trampoline).
#[derive(Debug, Clone, Copy)]
pub struct FfiFunction {
    pub args: Option<&'static [FfiType]>,
    pub returns: FfiType,
    pub ptr_override: Option<Pointer>,
    pub threadsafe: bool,
}

impl FfiFunction {
    /// Portable definition; returns default to `void` (mirrors
    /// `definition.returns ?? FFIType.void`).
    #[must_use]
    pub const fn new(args: Option<&'static [FfiType]>, returns: FfiType) -> FfiFunction {
        FfiFunction {
            args,
            returns,
            ptr_override: None,
            threadsafe: false,
        }
    }

    /// Effective return type.
    #[must_use]
    pub const fn return_type(self) -> FfiType {
        self.returns
    }

    /// Node-backend compatibility (mirrors `normalizeNodeDefinition`:
    /// `ptr` overrides rejected, every arg/result mapped).
    pub fn check_node_compat(self) -> Result<(), FfiError> {
        if self.ptr_override.is_some() {
            return Err(FfiError::PointerOverride);
        }
        let args: &[FfiType] = self.args.unwrap_or(&[]);
        for arg in args {
            arg.node_abi_name(false)?;
        }
        self.returns.node_abi_name(true)?;
        Ok(())
    }
}

/// Owned native trampoline (mirrors `FFICallbackInstance`).
/// `close()` invalidates `ptr`; the pointer must not cross to native code
/// afterwards. Idempotent; the pointer clears even on repeated close.
#[derive(Debug, Clone, Copy)]
pub struct CallbackHandle {
    ptr: Option<Pointer>,
    threadsafe: bool,
    closed: bool,
}

impl CallbackHandle {
    /// Wrap a fresh trampoline. The Node backend rejects threadsafe
    /// callbacks (mirrors `NODE_CALLBACK_THREADSAFE`); pass
    /// `node_backend = false` for the Bun path which allows them.
    pub fn open(ptr: Pointer, threadsafe: bool, node_backend: bool) -> Result<CallbackHandle, FfiError> {
        if node_backend && threadsafe {
            return Err(FfiError::ThreadsafeCallback);
        }
        Ok(CallbackHandle {
            ptr: Some(ptr),
            threadsafe,
            closed: false,
        })
    }

    /// Live trampoline address, or `None` once closed.
    #[must_use]
    pub const fn ptr(self) -> Option<Pointer> {
        self.ptr
    }

    /// Whether the trampoline was created threadsafe.
    #[must_use]
    pub const fn is_threadsafe(self) -> bool {
        self.threadsafe
    }

    /// Invalidate the trampoline (mirrors the managed `close()` that clears
    /// `ptr` in a `finally`).
    pub fn close(&mut self) {
        self.closed = true;
        self.ptr = None;
    }

    /// Already closed.
    #[must_use]
    pub const fn is_closed(self) -> bool {
        self.closed
    }
}

/// Render-library load failure (mirrors the `resolveRenderLib` wrapper
/// `"Failed to initialize OpenTUI render library: …"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderLibError {
    /// `setRenderLibPath()` with a new path after resolution.
    AlreadyResolved,
    /// Native construction failed (eager load swallows this).
    InitFailed,
}

impl RenderLibError {
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            RenderLibError::AlreadyResolved => "setRenderLibPath() must be called before resolveRenderLib()",
            RenderLibError::InitFailed => "Failed to initialize OpenTUI render library",
        }
    }
}

/// Render-library path cell (mirrors the `opentuiLibPath` / `opentuiLib` /
/// `renderLibResolved` trio in `zig.ts`). Heap-free: borrows the path.
#[derive(Debug, Clone, Copy)]
pub struct RenderLibConfig {
    path: Option<&'static str>,
    resolved: bool,
}

impl RenderLibConfig {
    /// Fresh cell: no path, nothing resolved.
    #[must_use]
    pub const fn new() -> RenderLibConfig {
        RenderLibConfig {
            path: None,
            resolved: false,
        }
    }

    /// Set the library path (mirrors `setRenderLibPath`): same-path is a
    /// no-op, a new path after resolution fails `AlreadyResolved`.
    pub fn set_path(&mut self, path: &'static str) -> Result<(), RenderLibError> {
        if self.path == Some(path) {
            return Ok(());
        }
        if self.resolved {
            return Err(RenderLibError::AlreadyResolved);
        }
        self.path = Some(path);
        Ok(())
    }

    /// Resolve the library (mirrors `resolveRenderLib`): marks resolved.
    pub fn resolve(&mut self) -> Result<(), RenderLibError> {
        self.resolved = true;
        Ok(())
    }

    /// Eager load at startup (mirrors the trailing `try { new FFIRenderLib }
    /// catch {}`): failures are swallowed, never propagated.
    pub fn eager_resolve(&mut self) {
        let _ = self.resolve();
    }

    /// Current path, if any.
    #[must_use]
    pub const fn path(self) -> Option<&'static str> {
        self.path
    }

    /// Whether the library has been resolved.
    #[must_use]
    pub const fn is_resolved(self) -> bool {
        self.resolved
    }
}

const _: () = assert!(MAX_SAFE_POINTER == (1 << 53) - 1);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_safe_pointer_value() {
        assert_eq!(MAX_SAFE_POINTER, (1u64 << 53) - 1);
        assert!(Pointer::from_raw(MAX_SAFE_POINTER).is_some());
        assert_eq!(Pointer::from_raw(MAX_SAFE_POINTER + 1), None);
    }

    #[test]
    fn pointer_rejects_negative_and_unsafe() {
        assert_eq!(Pointer::from_i64(-1), None);
        assert_eq!(Pointer::NULL.get(), 0);
        assert_eq!(Pointer::from_raw(u64::MAX), None);
        let p = Pointer::from_raw(42).expect("in range");
        assert_eq!(p.get(), 42);
        assert_eq!(p.offset_by(-1), None);
        assert_eq!(p.offset_by(8).expect("offset").get(), 50);
        assert_eq!(Pointer::from_raw(MAX_SAFE_POINTER).expect("max").offset_by(1), None);
    }

    #[test]
    fn ffi_type_tags_match_bun_strings() {
        assert_eq!(FfiType::I32.tag(), "int32_t");
        assert_eq!(FfiType::Int.tag(), "int");
        assert_eq!(FfiType::U32.tag(), "uint32_t");
        assert_eq!(FfiType::F64.tag(), "double");
        assert_eq!(FfiType::Bool.tag(), "bool");
        assert_eq!(FfiType::Ptr.tag(), "ptr");
        assert_eq!(FfiType::Void.tag(), "void");
        assert_eq!(FfiType::CString.tag(), "cstring");
        assert_eq!(FfiType::Usize.tag(), "usize");
        assert_eq!(FfiType::Buffer.tag(), "buffer");
    }

    #[test]
    fn pointer_arg_kinds_are_ptr_family_only() {
        for t in [FfiType::Ptr, FfiType::Pointer, FfiType::Function, FfiType::Callback] {
            assert!(t.is_pointer_arg(), "{t:?} must be a pointer arg");
        }
        for t in [
            FfiType::I32,
            FfiType::U64,
            FfiType::F64,
            FfiType::Bool,
            FfiType::Void,
            FfiType::CString,
            FfiType::Usize,
            FfiType::Buffer,
        ] {
            assert!(!t.is_pointer_arg(), "{t:?} must not be a pointer arg");
        }
    }

    #[test]
    fn node_abi_mapping_and_rejections() {
        assert_eq!(FfiType::I32.node_abi_name(false), Ok("i32"));
        assert_eq!(FfiType::Int.node_abi_name(false), Ok("i32"));
        assert_eq!(FfiType::CString.node_abi_name(false), Ok("string"));
        assert_eq!(FfiType::CString.node_abi_name(true), Err(FfiError::StringReturn));
        assert_eq!(FfiType::Function.node_abi_name(false), Ok("pointer"));
        assert_eq!(FfiType::Buffer.node_abi_name(false), Ok("pointer"));
        assert_eq!(FfiType::Usize.node_abi_name(false), Err(FfiError::UsizeUnsupported));
        assert_eq!(FfiType::NapiEnv.node_abi_name(false), Err(FfiError::NapiUnsupported));
        assert_eq!(FfiType::NapiValue.node_abi_name(true), Err(FfiError::NapiUnsupported));
    }

    #[test]
    fn ffi_function_defaults_and_node_compat() {
        static ARGS: &[FfiType] = &[FfiType::Ptr, FfiType::I32];
        let f = FfiFunction::new(Some(ARGS), FfiType::Void);
        assert_eq!(f.return_type(), FfiType::Void);
        assert!(f.check_node_compat().is_ok());

        let with_ptr = FfiFunction {
            ptr_override: Pointer::from_raw(1),
            ..f
        };
        assert_eq!(with_ptr.check_node_compat(), Err(FfiError::PointerOverride));

        let bad_result = FfiFunction::new(None, FfiType::CString);
        assert_eq!(bad_result.check_node_compat(), Err(FfiError::StringReturn));

        let no_args = FfiFunction::new(None, FfiType::Void);
        assert!(no_args.check_node_compat().is_ok());
    }

    #[test]
    fn callback_close_clears_ptr_idempotent() {
        let ptr = Pointer::from_raw(0x1000).expect("addr");
        let mut cb = CallbackHandle::open(ptr, false, true).expect("open");
        assert_eq!(cb.ptr(), Some(ptr));
        assert!(!cb.is_closed());
        cb.close();
        assert_eq!(cb.ptr(), None);
        assert!(cb.is_closed());
        cb.close();
        assert_eq!(cb.ptr(), None);
    }

    #[test]
    fn node_threadsafe_callback_rejected_bun_allowed() {
        let ptr = Pointer::from_raw(8).expect("addr");
        assert_eq!(
            CallbackHandle::open(ptr, true, true).unwrap_err(),
            FfiError::ThreadsafeCallback
        );
        let bun = CallbackHandle::open(ptr, true, false).expect("bun threadsafe");
        assert!(bun.is_threadsafe());
        assert_eq!(bun.ptr(), Some(ptr));
    }

    #[test]
    fn render_lib_path_lifecycle() {
        let mut cfg = RenderLibConfig::new();
        assert!(!cfg.is_resolved());
        assert_eq!(cfg.path(), None);
        cfg.set_path("/lib/render.so").expect("set");
        assert_eq!(cfg.path(), Some("/lib/render.so"));
        cfg.set_path("/lib/render.so").expect("same path no-op");
        cfg.resolve().expect("resolve");
        assert!(cfg.is_resolved());
        assert_eq!(
            cfg.set_path("/lib/other.so").unwrap_err(),
            RenderLibError::AlreadyResolved
        );
        // Same path still a no-op after resolution.
        cfg.set_path("/lib/render.so").expect("same path ok");
    }

    #[test]
    fn eager_resolve_swallows_failure() {
        let mut cfg = RenderLibConfig::new();
        cfg.eager_resolve();
        assert!(cfg.is_resolved());
        assert_eq!(
            RenderLibError::AlreadyResolved.message(),
            "setRenderLibPath() must be called before resolveRenderLib()"
        );
    }
}
