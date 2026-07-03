// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Named Pipe client for communicating with the easysearch daemon.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

/// Request sent to the daemon.
#[derive(Debug, Serialize)]
pub struct DaemonRequest {
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

/// Response from the daemon.
#[derive(Debug, Deserialize)]
pub struct DaemonResponse {
    pub id: u64,
    pub ok: bool,
    pub ready: Option<bool>,
    pub items: Option<Vec<DaemonItem>>,
    pub error: Option<String>,
}

/// A single search result from the daemon.
#[derive(Debug, Clone, Deserialize)]
pub struct DaemonItem {
    pub path: String,
    pub name: String,
    pub is_directory: bool,
    pub score: u32,
    pub highlight: Vec<[u32; 2]>,
}

/// IPC client that connects to the easysearch daemon.
pub struct IpcClient {
    pipe_name: String,
    #[cfg(windows)]
    handle: Option<windows::Win32::Foundation::HANDLE>,
}

impl IpcClient {
    /// Create a new IPC client. Does not connect immediately.
    pub fn new(pipe_name: String) -> Self {
        Self {
            pipe_name,
            #[cfg(windows)]
            handle: None,
        }
    }

    /// Attempt to connect to the daemon pipe.
    #[cfg(windows)]
    pub fn connect(&mut self) -> Result<(), String> {
        use windows::Win32::Storage::FileSystem::{
            CreateFileW, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_NONE, OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
        };
        use windows::core::PCWSTR;

        let pipe_wide: Vec<u16> = self
            .pipe_name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let handle = unsafe {
            CreateFileW(
                PCWSTR(pipe_wide.as_ptr()),
                (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }
        .map_err(|e| format!("Failed to connect to daemon: {e}"))?;

        self.handle = Some(handle);
        Ok(())
    }

    /// Send a search query and get results.
    #[cfg(windows)]
    pub fn search(&mut self, query: &str, limit: usize) -> Result<Vec<DaemonItem>, String> {
        if self.handle.is_none() {
            self.connect()?;
        }

        let req = DaemonRequest {
            id: REQUEST_ID.fetch_add(1, Ordering::Relaxed),
            method: "search".to_string(),
            query: Some(query.to_string()),
            limit: Some(limit),
        };

        let mut req_json = serde_json::to_vec(&req).map_err(|e| e.to_string())?;
        req_json.push(b'\n');

        self.write_all(&req_json)?;
        let response_line = self.read_line()?;

        let resp: DaemonResponse =
            serde_json::from_str(&response_line).map_err(|e| e.to_string())?;

        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "unknown error".to_string()));
        }

        Ok(resp.items.unwrap_or_default())
    }

    #[cfg(windows)]
    fn write_all(&self, data: &[u8]) -> Result<(), String> {
        use windows::Win32::Storage::FileSystem::WriteFile;

        let handle = self.handle.ok_or("not connected")?;
        let mut offset = 0;
        while offset < data.len() {
            let mut written = 0u32;
            unsafe {
                WriteFile(
                    handle,
                    Some(&data[offset..]),
                    Some(std::ptr::from_mut(&mut written)),
                    None,
                )
            }
            .map_err(|e| format!("write failed: {e}"))?;
            offset += written as usize;
        }
        Ok(())
    }

    #[cfg(windows)]
    fn read_line(&self) -> Result<String, String> {
        use windows::Win32::Storage::FileSystem::ReadFile;

        let handle = self.handle.ok_or("not connected")?;
        let mut buf = Vec::with_capacity(4096);
        let mut byte = [0u8; 1];

        loop {
            let mut read = 0u32;
            let result = unsafe {
                ReadFile(
                    handle,
                    Some(&mut byte),
                    Some(std::ptr::from_mut(&mut read)),
                    None,
                )
            };

            match result {
                Ok(()) if read == 1 => {
                    if byte[0] == b'\n' {
                        break;
                    }
                    buf.push(byte[0]);
                }
                Ok(()) => break, // EOF
                Err(e) => return Err(format!("read failed: {e}")),
            }
        }

        String::from_utf8(buf).map_err(|e| e.to_string())
    }

    /// Disconnect from the pipe.
    #[cfg(windows)]
    pub fn disconnect(&mut self) {
        if let Some(handle) = self.handle.take() {
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(handle);
            }
        }
    }

    /// Check if connected.
    #[cfg(windows)]
    pub fn is_connected(&self) -> bool {
        self.handle.is_some()
    }
}

#[cfg(windows)]
impl Drop for IpcClient {
    fn drop(&mut self) {
        self.disconnect();
    }
}
