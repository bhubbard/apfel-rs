// ============================================================================
// foundation_models.rs — Native macOS FoundationModels engine
// Part of apfel-rs
// ============================================================================

use crate::backend::engine::{BackendEngine, GenerateRequest, GenerateResponse, StreamChunk};
use crate::core::error::ApfelError;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::sync::mpsc as std_mpsc;
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

    // SAFETY: The bridge guarantees `user_data` is a valid pointer to a `std_mpsc::Sender<StreamChunk>`.
    // The sender is allocated on the stack of the calling thread, which blocks on `apfel_bridge_generate_json`
    // until all callbacks complete, ensuring the pointer is valid and outlives this callback. (No Use-After-Free)
    let sender = unsafe { &*(user_data as *const std_mpsc::Sender<StreamChunk>) };

    if !error.is_null() {
        // SAFETY: `error` is provided by the Swift bridge, which guarantees it is a valid null-terminated UTF-8 C string.
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
        let _ = sender.send(StreamChunk::Error(err));
        return;
    }

    if !chunk.is_null() {
        // SAFETY: `chunk` is provided by the Swift bridge, which guarantees it is a valid null-terminated UTF-8 C string.
        let text = unsafe { CStr::from_ptr(chunk).to_str().unwrap_or("").to_string() };
        let _ = sender.send(StreamChunk::Delta(text));
    }

    if is_done {
        let reason = if !finish_reason.is_null() {
            // SAFETY: `finish_reason` is provided by the Swift bridge, which guarantees it is a valid null-terminated UTF-8 C string.
            unsafe {
                CStr::from_ptr(finish_reason)
                    .to_str()
                    .unwrap_or("stop")
                    .to_string()
            }
        } else {
            "stop".to_string()
        };
        let _ = sender.send(StreamChunk::Done {
            finish_reason: reason,
        });
    }
}

/// Native macOS FoundationModels engine leveraging Apple Intelligence on Apple Silicon.
#[derive(Debug)]
pub struct FoundationModelsEngine;

impl FoundationModelsEngine {
    pub fn new() -> Self {
        Self
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

    fn context_size(&self) -> usize {
        // SAFETY: Calling `apfel_bridge_context_size` from the Swift bridge static library linked at build time.
        // Takes no arguments and has no side effects on Rust memory.
        let size = unsafe { apfel_bridge_context_size() };
        if size > 0 {
            size as usize
        } else {
            4096
        }
    }

    fn count_tokens(&self, text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        if let Ok(c_text) = CString::new(text) {
            // SAFETY: Calling `apfel_bridge_token_count` from the Swift bridge static library linked at build time.
            // `c_text` is a valid null-terminated C string whose pointer remains valid for the duration of the call.
            let n = unsafe { apfel_bridge_token_count(c_text.as_ptr()) };
            if n > 0 {
                return n as usize;
            }
        }
        (text.chars().count() / 4).max(1)
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

        let (std_tx, std_rx) = std_mpsc::channel::<StreamChunk>();
        let (tokio_tx, tokio_rx) = tokio_mpsc::channel::<StreamChunk>(128);

        // Native bridge thread
        std::thread::spawn(move || {
            // Elevate thread QoS so macOS schedules this on P-cores (performance cores)
            // rather than E-cores (efficiency cores) on Apple Silicon.
            // SAFETY: pthread_set_qos_class_self_np is safe to call on the current thread.
            // QOS_CLASS_USER_INITIATED (0x19) ensures high priority without starving UI apps.
            unsafe {
                libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INITIATED, 0);
            }
            let tx_ptr = &std_tx as *const std_mpsc::Sender<StreamChunk> as *mut c_void;
            // SAFETY: Calling `apfel_bridge_generate_json` from the Swift bridge static library linked at build time.
            // `c_json` is a valid null-terminated C string whose pointer remains valid for the duration of the call.
            // `tx_ptr` points to `std_tx` which is allocated on the stack of this thread. Since `apfel_bridge_generate_json`
            // blocks until all callbacks are complete, `std_tx` remains valid and in scope for the entire FFI call.
            let code =
                unsafe { apfel_bridge_generate_json(c_json.as_ptr(), tx_ptr, on_bridge_chunk) };
            if code != 0 {
                tracing::debug!("FoundationModels bridge exited with code {}", code);
            }
        });

        // Pump from std_mpsc to tokio_mpsc in blocking thread pool
        tokio::task::spawn_blocking(move || {
            while let Ok(chunk) = std_rx.recv() {
                let is_terminal = matches!(chunk, StreamChunk::Done { .. } | StreamChunk::Error(_));
                if tokio_tx.blocking_send(chunk).is_err() {
                    break;
                }
                if is_terminal {
                    break;
                }
            }
        });

        Ok(tokio_rx)
    }

    fn generate(&self, req: &GenerateRequest) -> Result<GenerateResponse, ApfelError> {
        let json_str = serde_json::to_string(req)
            .map_err(|e| ApfelError::Usage(format!("Failed to serialize request: {}", e)))?;
        let c_json = CString::new(json_str)
            .map_err(|e| ApfelError::Usage(format!("Request contains null byte: {}", e)))?;

        let (std_tx, std_rx) = std_mpsc::channel::<StreamChunk>();

        std::thread::spawn(move || {
            let tx_ptr = &std_tx as *const std_mpsc::Sender<StreamChunk> as *mut c_void;
            // SAFETY: Calling `apfel_bridge_generate_json` from the Swift bridge static library linked at build time.
            // `c_json` is a valid null-terminated C string whose pointer remains valid for the duration of the call.
            // `tx_ptr` points to `std_tx` which is allocated on the stack of this thread. Since `apfel_bridge_generate_json`
            // blocks until all callbacks are complete, `std_tx` remains valid and in scope for the entire FFI call.
            let code =
                unsafe { apfel_bridge_generate_json(c_json.as_ptr(), tx_ptr, on_bridge_chunk) };
            if code != 0 {
                tracing::debug!("FoundationModels bridge exited with code {}", code);
            }
        });

        let mut content = String::new();
        let mut finish_reason = "stop".to_string();

        while let Ok(chunk) = std_rx.recv() {
            match chunk {
                StreamChunk::Delta(delta) => {
                    content.push_str(&delta);
                }
                StreamChunk::Done { finish_reason: r } => {
                    finish_reason = r;
                    break;
                }
                StreamChunk::Error(e) => return Err(e),
            }
        }

        Ok(GenerateResponse {
            content,
            finish_reason,
        })
    }
}
