// Copyright 2024 Qi, Yadong.
// SPDX-License-Identifier: Apache-2.0

use std::ffi::CString;
use std::fs::OpenOptions;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::IntoRawHandle;
use std::os::windows::io::RawHandle;
use std::thread::sleep;
use std::time;

use log::error;

use windows::core::PCSTR;
use windows::Win32::Foundation::DuplicateHandle;
use windows::Win32::Foundation::DUPLICATE_SAME_ACCESS;
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::Foundation::ERROR_IO_PENDING;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::TRUE;

use windows::Win32::Storage::FileSystem::ReadFile;
use windows::Win32::Storage::FileSystem::WriteFile;
use windows::Win32::Storage::FileSystem::FILE_FLAG_OVERLAPPED;
use windows::Win32::Storage::FileSystem::SECURITY_SQOS_PRESENT;
use windows::Win32::System::Pipes::PeekNamedPipe;
use windows::Win32::System::Pipes::SetNamedPipeHandleState;
use windows::Win32::System::Pipes::WaitNamedPipeA;
use windows::Win32::System::Pipes::NAMED_PIPE_MODE;
use windows::Win32::System::Pipes::PIPE_READMODE_BYTE;
use windows::Win32::System::Threading::GetCurrentProcess;
use windows::Win32::System::IO::GetOverlappedResult;
use windows::Win32::System::IO::OVERLAPPED;

#[derive(Debug)]
pub struct HandleDesc {
    pub handle: isize,
}

impl HandleDesc {
    pub fn as_handle(&self) -> HANDLE {
        HANDLE(self.handle as *mut core::ffi::c_void)
    }

    pub fn from_handle(h: HANDLE) -> Self {
        Self {
            handle: h.0 as isize,
        }
    }

    pub fn try_clone(&self) -> windows::core::Result<Self> {
        let mut cloned_handle: HANDLE = HANDLE::default();
        let ret = unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                self.as_handle(),
                GetCurrentProcess(),
                &mut cloned_handle,
                0,
                TRUE,
                DUPLICATE_SAME_ACCESS,
            )
        };

        match ret {
            Err(e) => Err(e),
            Ok(()) => Ok(Self {
                handle: cloned_handle.0 as isize,
            }),
        }
    }
}

impl Clone for HandleDesc {
    fn clone(&self) -> Self {
        self.try_clone().unwrap()
    }
}

#[derive(Clone)]
pub struct NamedPipe {
    pipe_handle: HandleDesc,
}

unsafe fn set_named_pipe_handle_state(
    pipe_handle: HANDLE,
    client_mode: Option<*const NAMED_PIPE_MODE>,
) -> windows::core::Result<()> {
    if let Err(e) = SetNamedPipeHandleState(pipe_handle, client_mode, None, None) {
        error!("Failed to set pipe handle state: {:?}", e);
        return Err(e);
    }
    Ok(())
}

#[allow(dead_code)]
unsafe fn wait_named_pipe(name: &str, timeout: u32) -> windows::core::Result<()> {
    let pipe_name = CString::new(name).unwrap();
    match WaitNamedPipeA(PCSTR(pipe_name.as_ptr() as *const u8), timeout) {
        Ok(()) => Ok(()),
        Err(e) => {
            error!("Timeout wait pipe: {} milliseconds! e={}", timeout, e);
            Err(e)
        }
    }
}

impl NamedPipe {
    pub fn as_handle(&self) -> HANDLE {
        self.pipe_handle.as_handle()
    }

    pub fn as_raw_handle(&self) -> RawHandle {
        self.as_handle().0 as RawHandle
    }

    pub fn try_open(name: &str, wait: bool) -> windows::core::Result<NamedPipe> {
        if wait {
            Self::open_wait(name)
        } else {
            Self::open(name)
        }
    }

    pub fn open_wait(name: &str) -> windows::core::Result<NamedPipe> {
        loop {
            match Self::open(name) {
                Ok(pipe) => return Ok(pipe),
                Err(e) => {
                    if e == ERROR_FILE_NOT_FOUND.into() {
                        let duration = time::Duration::from_millis(100);
                        sleep(duration);
                        continue;
                    } else {
                        break Err(e);
                    }
                }
            };
        }
    }

    pub fn open(name: &str) -> windows::core::Result<NamedPipe> {
        let raw_handle = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .custom_flags(SECURITY_SQOS_PRESENT.0 | FILE_FLAG_OVERLAPPED.0)
            .open(name)?
            .into_raw_handle();
        let pipe_handle = HANDLE(raw_handle);

        unsafe {
            let client_mode = PIPE_READMODE_BYTE;
            set_named_pipe_handle_state(pipe_handle, Some(&client_mode))?;
        };

        Ok(NamedPipe {
            pipe_handle: HandleDesc::from_handle(pipe_handle),
        })
    }

    pub fn get_available_byte_count(&self) -> windows::core::Result<u32> {
        let mut total_bytes_avail = 0;

        match unsafe {
            PeekNamedPipe(
                self.as_handle(),
                None,
                0,
                None,
                Some(&mut total_bytes_avail),
                None,
            )
        } {
            Err(e) => Err(e),
            Ok(_) => Ok(total_bytes_avail),
        }
    }

    pub fn read(&self, buffer: &mut Vec<u8>) -> windows::core::Result<u32> {
        let mut bytes_read: u32 = 0;
        let mut ov = OVERLAPPED::default();

        let avail_bytes = self.get_available_byte_count()?;
        buffer.resize(avail_bytes as usize, 0);

        match unsafe {
            ReadFile(
                self.as_handle(),
                Some(buffer),
                Some(&mut bytes_read),
                Some(&mut ov),
            )
        } {
            Err(e) => {
                if e.code() == ERROR_IO_PENDING.into() {
                    unsafe {
                        GetOverlappedResult(self.as_handle(), &ov, &mut bytes_read, TRUE)?;
                    }
                    Ok(bytes_read)
                } else {
                    Err(e)
                }
            }
            Ok(_) => Ok(bytes_read),
        }
    }

    pub fn write(&self, buffer: &[u8]) -> windows::core::Result<u32> {
        let mut bytes_written: u32 = buffer.len() as u32;
        let mut ov = OVERLAPPED::default();

        match unsafe {
            WriteFile(
                self.as_handle(),
                Some(buffer),
                Some(&mut bytes_written),
                Some(&mut ov),
            )
        } {
            Err(e) => {
                if e.code() == ERROR_IO_PENDING.into() {
                    unsafe {
                        GetOverlappedResult(self.as_handle(), &ov, &mut bytes_written, TRUE)?;
                    }
                    Ok(bytes_written)
                } else {
                    Err(e)
                }
            }
            Ok(_) => Ok(bytes_written),
        }
    }
}
