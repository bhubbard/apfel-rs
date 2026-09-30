// ============================================================================
// foundation_models.rs — Native macOS FoundationModels engine
// Part of apfel-rs
// ============================================================================

use crate::backend::engine::{BackendEngine, GenerateRequest, GenerateResponse, StreamChunk};
use crate::core::error::ApfelError;
use crate::core::token_counter::TokenCounter;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use tokio::sync::mpsc as tokio_mpsc;

type BridgeCallback = extern "C" fn(
    chunk: *const c_char,
    is_done: bool,
    finish_reason: *const c_char,
    error: *const c_char,
    user_data: *mut c_void,
);

extern "C" {
    fn apfel_bridge_is_available() -> bool;
    fn apfel_bridge_context_size() -> i32;
    fn apfel_bridge_token_count(text: *const c_char) -> i32;
    fn apfel_bridge_supported_languages() -> *const c_char;
    fn apfel_bridge_generate_json(
        request_json: *const c_char,
        user_data: *mut c_void,
        callback: BridgeCallback,
    ) -> i32;
}

/// Callback for streaming generation delivering deltas directly into Tokio channel.
extern "C" fn on_bridge_chunk(
    chunk: *const c_char,
    is_done: bool,
    finish_reason: *const c_char,
    error: *const c_char,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }

    // SAFETY: The bridge guarantees `user_data` is a valid pointer to a `tokio_mpsc::Sender<StreamChunk>`.
    // The sender is allocated on the stack of the thread calling `apfel_bridge_generate_json`,
    // which blocks until all callbacks complete. (No Use-After-Free)
    let sender = unsafe { &*(user_data as *const tokio_mpsc::Sender<StreamChunk>) };

    if !error.is_null() {
        let err_msg = unsafe { CStr::from_ptr(error).to_string_lossy().to_string() };
        let err = if err_msg.to_lowercase().contains("guardrail") {
            ApfelError::Guardrail(err_msg)
        } else if err_msg.to_lowercase().contains("context")
            || err_msg.to_lowercase().contains("token")
        {
            ApfelError::ContextOverflow(err_msg)
        } else {
            ApfelError::Runtime(err_msg)
        };
        let _ = sender.blocking_send(StreamChunk::Error(err));
        return;
    }

    if !chunk.is_null() {
        let text = unsafe { CStr::from_ptr(chunk).to_str().unwrap_or("").to_string() };
        let _ = sender.blocking_send(StreamChunk::Delta(text));
    }

    if is_done {
        let reason = if !finish_reason.is_null() {
            unsafe {
                CStr::from_ptr(finish_reason)
                    .to_str()
                    .unwrap_or("stop")
                    .to_string()
            }
        } else {
            "stop".to_string()
        };
        let _ = sender.blocking_send(StreamChunk::Done {
            finish_reason: reason,
        });
    }
}

/// Accumulation context for non-streaming direct generation (zero channels, zero thread hops).
struct DirectGenerateContext {
    content: String,
    finish_reason: String,
    error: Option<ApfelError>,
}

extern "C" fn on_bridge_direct_generate(
    chunk: *const c_char,
    is_done: bool,
    finish_reason: *const c_char,
    error: *const c_char,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    let ctx = unsafe { &mut *(user_data as *mut DirectGenerateContext) };

    if !error.is_null() {
        let err_msg = unsafe { CStr::from_ptr(error).to_string_lossy().to_string() };
        let err = if err_msg.to_lowercase().contains("guardrail") {
            ApfelError::Guardrail(err_msg)
        } else if err_msg.to_lowercase().contains("context")
            || err_msg.to_lowercase().contains("token")
        {
            ApfelError::ContextOverflow(err_msg)
        } else {
            ApfelError::Runtime(err_msg)
        };
        ctx.error = Some(err);
        return;
    }

    if !chunk.is_null() {
        let text = unsafe { CStr::from_ptr(chunk).to_str().unwrap_or("") };
        ctx.content.push_str(text);
    }

    if is_done && !finish_reason.is_null() {
        ctx.finish_reason = unsafe {
            CStr::from_ptr(finish_reason)
                .to_str()
                .unwrap_or("stop")
                .to_string()
        };
    }
}

/// Native macOS FoundationModels engine leveraging Apple Intelligence on Apple Silicon.
#[derive(Debug)]
pub struct FoundationModelsEngine {
    pub adapter_path: Option<String>,
    token_counter: TokenCounter,
}

impl FoundationModelsEngine {
    pub fn new() -> Self {
        Self {
            adapter_path: None,
            token_counter: TokenCounter::new(),
        }
    }

    pub fn with_adapter(adapter: impl Into<String>) -> Self {
        Self {
            adapter_path: Some(adapter.into()),
            token_counter: TokenCounter::new(),
        }
    }
}

impl Default for FoundationModelsEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl BackendEngine for FoundationModelsEngine {
    fn is_available(&self) -> bool {
        // SAFETY: Calling `apfel_bridge_is_available` from the Swift bridge static library linked at build time.
        // Takes no arguments and has no side effects on Rust memory.
        unsafe { apfel_bridge_is_available() }
    }

    fn adapter_path(&self) -> Option<&str> {
        self.adapter_path.as_deref()
    }

    fn context_size(&self) -> usize {
        // SAFETY: Calling `apfel_bridge_context_size` from the Swift bridge static library linked at build time.
        let size = unsafe { apfel_bridge_context_size() };
        let measured = if size > 0 { size as usize } else { 4096 };
        self.token_counter.observe_context_size(measured)
    }

    fn context_window_measured(&self) -> bool {
        // SAFETY: Calling `apfel_bridge_context_size` from the Swift bridge static library linked at build time.
        let size = unsafe { apfel_bridge_context_size() };
        size > 4096
    }

    fn count_tokens(&self, text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }

        self.token_counter.count_cached(text, |s| {
            if let Ok(c_text) = CString::new(s) {
                // SAFETY: Calling `apfel_bridge_token_count` from the Swift bridge static library linked at build time.
                let n = unsafe { apfel_bridge_token_count(c_text.as_ptr()) };
                if n > 0 {
                    return n as usize;
                }
            }
            (s.chars().count() / 4).max(1)
        })
    }

    fn supported_languages(&self) -> Vec<String> {
        // SAFETY: Calling `apfel_bridge_supported_languages` from the Swift bridge static library linked at build time.
        let ptr = unsafe { apfel_bridge_supported_languages() };
        if ptr.is_null() {
            return vec!["en".to_string()];
        }
        // SAFETY: The bridge guarantees the returned pointer is a valid null-terminated C string.
        let s = unsafe { CStr::from_ptr(ptr).to_string_lossy() };
        s.split(',')
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect()
    }

    fn stream_generate(
        &self,
        req: &GenerateRequest,
    ) -> Result<tokio_mpsc::Receiver<StreamChunk>, ApfelError> {
        if !self.is_available() {
            return Err(ApfelError::ModelUnavailable(
                "Apple Intelligence on-device model is unavailable".to_string(),
            ));
        }

        let json_str = serde_json::to_string(req)
            .map_err(|e| ApfelError::Usage(format!("Failed to serialize request: {}", e)))?;
        let c_json = CString::new(json_str)
            .map_err(|e| ApfelError::Usage(format!("Request contains null byte: {}", e)))?;

        let (tokio_tx, tokio_rx) = tokio_mpsc::channel::<StreamChunk>(128);

        // Native bridge execution on Tokio blocking thread pool with P-core QoS priority.
        // Delivers chunks directly into the Tokio channel with zero intermediate threads or channel hops.
        tokio::task::spawn_blocking(move || {
            // Elevate thread QoS so macOS schedules this on P-cores (performance cores)
            // SAFETY: pthread_set_qos_class_self_np is safe to call on the current thread.
            unsafe {
                libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INITIATED, 0);
            }
            let tx_ptr = &tokio_tx as *const tokio_mpsc::Sender<StreamChunk> as *mut c_void;
            let code =
                unsafe { apfel_bridge_generate_json(c_json.as_ptr(), tx_ptr, on_bridge_chunk) };
            if code != 0 {
                tracing::debug!("FoundationModels bridge exited with code {}", code);
            }
        });

        Ok(tokio_rx)
    }

    fn generate(&self, req: &GenerateRequest) -> Result<GenerateResponse, ApfelError> {
        if !self.is_available() {
            return Err(ApfelError::ModelUnavailable(
                "Apple Intelligence on-device model is unavailable".to_string(),
            ));
        }

        let json_str = serde_json::to_string(req)
            .map_err(|e| ApfelError::Usage(format!("Failed to serialize request: {}", e)))?;
        let c_json = CString::new(json_str)
            .map_err(|e| ApfelError::Usage(format!("Request contains null byte: {}", e)))?;

        let mut ctx = DirectGenerateContext {
            content: String::with_capacity(512),
            finish_reason: "stop".to_string(),
            error: None,
        };

        // Execute directly on calling thread with elevated QoS. Zero thread spawns, zero channel allocations.
        unsafe {
            libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INITIATED, 0);
            let ctx_ptr = &mut ctx as *mut DirectGenerateContext as *mut c_void;
            let code =
                apfel_bridge_generate_json(c_json.as_ptr(), ctx_ptr, on_bridge_direct_generate);
            if code != 0 {
                tracing::debug!("FoundationModels bridge exited with code {}", code);
            }
        }

        if let Some(err) = ctx.error {
            return Err(err);
        }

        Ok(GenerateResponse {
            content: ctx.content,
            finish_reason: ctx.finish_reason,
        })
    }
}
