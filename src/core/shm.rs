// ============================================================================
// shm.rs — Apple Silicon Zero-Copy Unified Memory IPC for apfel-rs
// Enables direct memory pointer exchange between clients and on-device models
// eliminating JSON serialization overhead and CPU allocations.
// ============================================================================

use std::ffi::CString;
use std::ptr;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use thiserror::Error;
use tracing::debug;

const SHM_MAGIC: u32 = 0x4150_4645; // "APFE" in ASCII
const SHM_VERSION: u32 = 1;

#[derive(Error, Debug)]
pub enum ShmError {
    #[error("Failed to open POSIX shared memory object '{0}': {1}")]
    OpenFailed(String, std::io::Error),

    #[error("Failed to truncate shared memory to size {0}: {1}")]
    TruncateFailed(usize, std::io::Error),

    #[error("mmap failed: {0}")]
    MmapFailed(std::io::Error),

    #[error("Payload size {0} exceeds allocated capacity {1}")]
    CapacityExceeded(usize, usize),

    #[error("Invalid SHM header or magic mismatch (expected {0:#x}, found {1:#x})")]
    InvalidHeader(u32, u32),
}

/// Header layout at byte 0 of the shared memory region.
#[repr(C)]
pub struct ShmHeader {
    pub magic: u32,
    pub version: u32,
    pub capacity: u32,
    pub flags: AtomicU32,
    pub sequence: AtomicU64,
    pub payload_size: AtomicU32,
    pub reserved: [u32; 8],
}

/// High-throughput zero-copy shared memory buffer.
pub struct ShmBuffer {
    name: String,
    fd: libc::c_int,
    ptr: *mut u8,
    total_size: usize,
    is_owner: bool,
}

unsafe impl Send for ShmBuffer {}
unsafe impl Sync for ShmBuffer {}

impl ShmBuffer {
    /// Sanitizes and bounds the shared memory segment name for POSIX/macOS compatibility.
    pub fn sanitize_name(name: &str) -> String {
        let trimmed = name.trim_start_matches('/');
        // macOS limits shm_open names to 30 characters (including leading '/')
        let max_len = 29;
        let safe_core = if trimmed.len() > max_len {
            &trimmed[..max_len]
        } else {
            trimmed
        };
        format!("/{}", safe_core)
    }

    /// Creates and initializes a new shared memory segment.
    pub fn create(name: &str, payload_capacity: usize) -> Result<Self, ShmError> {
        let safe_name = Self::sanitize_name(name);
        let total_size = std::mem::size_of::<ShmHeader>() + payload_capacity;
        let c_name = CString::new(safe_name.as_str()).map_err(|e| ShmError::OpenFailed(safe_name.clone(), std::io::Error::new(std::io::ErrorKind::InvalidInput, e)))?;

        let fd = unsafe {
            libc::shm_open(
                c_name.as_ptr(),
                libc::O_RDWR | libc::O_CREAT | libc::O_TRUNC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(ShmError::OpenFailed(safe_name, std::io::Error::last_os_error()));
        }

        if unsafe { libc::ftruncate(fd, total_size as libc::off_t) } != 0 {
            unsafe { libc::close(fd) };
            return Err(ShmError::TruncateFailed(total_size, std::io::Error::last_os_error()));
        }

        let map_ptr = unsafe {
            libc::mmap(
                ptr::null_mut(),
                total_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };

        if map_ptr == libc::MAP_FAILED {
            unsafe { libc::close(fd) };
            return Err(ShmError::MmapFailed(std::io::Error::last_os_error()));
        }

        // Initialize header
        let header_ptr = map_ptr as *mut ShmHeader;
        unsafe {
            (*header_ptr).magic = SHM_MAGIC;
            (*header_ptr).version = SHM_VERSION;
            (*header_ptr).capacity = payload_capacity as u32;
            (*header_ptr).flags.store(0, Ordering::Release);
            (*header_ptr).sequence.store(0, Ordering::Release);
            (*header_ptr).payload_size.store(0, Ordering::Release);
        }

        debug!(shm_name = %name, total_size, "Created POSIX shared memory buffer");

        Ok(Self {
            name: name.to_string(),
            fd,
            ptr: map_ptr as *mut u8,
            total_size,
            is_owner: true,
        })
    }

    /// Attaches to an existing shared memory segment.
    pub fn open(name: &str) -> Result<Self, ShmError> {
        let safe_name = Self::sanitize_name(name);
        let c_name = CString::new(safe_name.as_str()).map_err(|e| ShmError::OpenFailed(safe_name.clone(), std::io::Error::new(std::io::ErrorKind::InvalidInput, e)))?;

        let fd = unsafe { libc::shm_open(c_name.as_ptr(), libc::O_RDWR, 0) };
        if fd < 0 {
            return Err(ShmError::OpenFailed(safe_name, std::io::Error::last_os_error()));
        }

        // Determine size via fstat
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(fd, &mut stat) } != 0 {
            unsafe { libc::close(fd) };
            return Err(ShmError::OpenFailed(name.to_string(), std::io::Error::last_os_error()));
        }
        let total_size = stat.st_size as usize;

        let map_ptr = unsafe {
            libc::mmap(
                ptr::null_mut(),
                total_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };

        if map_ptr == libc::MAP_FAILED {
            unsafe { libc::close(fd) };
            return Err(ShmError::MmapFailed(std::io::Error::last_os_error()));
        }

        let header = unsafe { &*(map_ptr as *const ShmHeader) };
        if header.magic != SHM_MAGIC {
            unsafe {
                libc::munmap(map_ptr, total_size);
                libc::close(fd);
            }
            return Err(ShmError::InvalidHeader(SHM_MAGIC, header.magic));
        }

        Ok(Self {
            name: name.to_string(),
            fd,
            ptr: map_ptr as *mut u8,
            total_size,
            is_owner: false,
        })
    }

    /// Writes raw payload data directly into the shared memory region.
    pub fn write_payload(&self, data: &[u8]) -> Result<u64, ShmError> {
        let header = self.header();
        let cap = header.capacity as usize;
        if data.len() > cap {
            return Err(ShmError::CapacityExceeded(data.len(), cap));
        }

        let payload_dest = unsafe { self.ptr.add(std::mem::size_of::<ShmHeader>()) };
        unsafe {
            ptr::copy_nonoverlapping(data.as_ptr(), payload_dest, data.len());
        }

        header.payload_size.store(data.len() as u32, Ordering::Release);
        let seq = header.sequence.fetch_add(1, Ordering::AcqRel) + 1;
        Ok(seq)
    }

    /// Reads raw payload bytes directly from shared memory.
    pub fn read_payload(&self) -> &[u8] {
        let header = self.header();
        let size = header.payload_size.load(Ordering::Acquire) as usize;
        let payload_src = unsafe { self.ptr.add(std::mem::size_of::<ShmHeader>()) };
        unsafe { std::slice::from_raw_parts(payload_src, size) }
    }

    pub fn header(&self) -> &ShmHeader {
        unsafe { &*(self.ptr as *const ShmHeader) }
    }

    /// Reads the current sequence number atomically.
    pub fn sequence(&self) -> u64 {
        self.header().sequence.load(Ordering::Acquire)
    }
}

impl Drop for ShmBuffer {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.ptr as *mut libc::c_void, self.total_size);
            libc::close(self.fd);
            if self.is_owner {
                if let Ok(c_name) = CString::new(self.name.as_str()) {
                    libc::shm_unlink(c_name.as_ptr());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shm_create_write_read() {
        let shm_name = format!("/apfel_{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
        let writer = ShmBuffer::create(&shm_name, 1024).unwrap();

        let message = b"Hello from Apple Silicon Unified Memory Zero-Copy IPC!";
        let seq = writer.write_payload(message).unwrap();
        assert_eq!(seq, 1);

        // Open as reader
        let reader = ShmBuffer::open(&shm_name).unwrap();
        let read_data = reader.read_payload();
        assert_eq!(read_data, message);
    }
}
