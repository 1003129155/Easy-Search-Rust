// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Early Windows process hardening for the single-process GUI.
//!
//! This deliberately uses a compatibility-oriented policy: it narrows DLL
//! lookup, blocks remote/low-integrity images, and disables legacy extension
//! points. Dynamic-code prohibition is intentionally not enabled because the
//! GUI must remain compatible with IMEs, accessibility tools, and graphics
//! components.

#![cfg(windows)]

use core::mem::{size_of, size_of_val};

use windows::Win32::System::LibraryLoader::{
    LOAD_LIBRARY_SEARCH_APPLICATION_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32, SetDefaultDllDirectories,
};
use windows::Win32::System::SystemServices::{
    PROCESS_MITIGATION_EXTENSION_POINT_DISABLE_POLICY,
    PROCESS_MITIGATION_EXTENSION_POINT_DISABLE_POLICY_0, PROCESS_MITIGATION_IMAGE_LOAD_POLICY,
    PROCESS_MITIGATION_IMAGE_LOAD_POLICY_0,
};
use windows::Win32::System::Threading::{
    ProcessExtensionPointDisablePolicy, ProcessImageLoadPolicy, SetProcessMitigationPolicy,
};

const DISABLE_EXTENSION_POINTS: u32 = 1 << 0;
const NO_REMOTE_IMAGES: u32 = 1 << 0;
const NO_LOW_MANDATORY_LABEL_IMAGES: u32 = 1 << 1;

/// Apply the GUI-compatible process policy before application initialization.
///
/// These policies are process-wide and effectively irreversible. Returning an
/// error lets the caller fail closed instead of silently running unprotected.
pub(crate) fn apply_compatible_policy() -> Result<(), HardeningError> {
    // Keep the application directory for EasySearch-owned native components,
    // but remove the current directory and PATH entries from default lookup.
    // SAFETY: flags are a documented combination and contain no pointers.
    unsafe {
        SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_APPLICATION_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32)
    }
    .map_err(|source| HardeningError::new("restrict DLL search directories", source))?;

    let image_load_policy = PROCESS_MITIGATION_IMAGE_LOAD_POLICY {
        Anonymous: PROCESS_MITIGATION_IMAGE_LOAD_POLICY_0 {
            Flags: NO_REMOTE_IMAGES | NO_LOW_MANDATORY_LABEL_IMAGES,
        },
    };
    set_policy(
        ProcessImageLoadPolicy,
        &image_load_policy,
        "block remote and low-integrity images",
    )?;

    let extension_policy = PROCESS_MITIGATION_EXTENSION_POINT_DISABLE_POLICY {
        Anonymous: PROCESS_MITIGATION_EXTENSION_POINT_DISABLE_POLICY_0 {
            Flags: DISABLE_EXTENSION_POINTS,
        },
    };
    set_policy(
        ProcessExtensionPointDisablePolicy,
        &extension_policy,
        "disable legacy extension points",
    )
}

fn set_policy<T>(
    policy: windows::Win32::System::Threading::PROCESS_MITIGATION_POLICY,
    value: &T,
    operation: &'static str,
) -> Result<(), HardeningError> {
    // SAFETY: `value` is the exact Win32 policy structure selected by
    // `policy`; its pointer remains valid for the duration of the call and the
    // byte length is derived from that same value.
    unsafe {
        SetProcessMitigationPolicy(
            policy,
            core::ptr::from_ref(value).cast(),
            size_of_val(value),
        )
    }
    .map_err(|source| HardeningError::new(operation, source))
}

#[derive(Debug)]
pub(crate) struct HardeningError {
    operation: &'static str,
    source: windows::core::Error,
}

impl HardeningError {
    fn new(operation: &'static str, source: windows::core::Error) -> Self {
        Self { operation, source }
    }
}

impl core::fmt::Display for HardeningError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{}: {}", self.operation, self.source)
    }
}

const _: () = {
    assert!(size_of::<PROCESS_MITIGATION_IMAGE_LOAD_POLICY>() == size_of::<u32>());
    assert!(size_of::<PROCESS_MITIGATION_EXTENSION_POINT_DISABLE_POLICY>() == size_of::<u32>());
};
