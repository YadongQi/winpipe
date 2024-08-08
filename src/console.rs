// Copyright 2024 Qi, Yadong.
// SPDX-License-Identifier: Apache-2.0
use std::sync::Arc;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::ReadFile;
use windows::Win32::Storage::FileSystem::WriteFile;
use windows::Win32::System::Console::GetConsoleCP;
use windows::Win32::System::Console::GetConsoleMode;
use windows::Win32::System::Console::GetConsoleOutputCP;
use windows::Win32::System::Console::GetStdHandle;
use windows::Win32::System::Console::SetConsoleCP;
use windows::Win32::System::Console::SetConsoleMode;
use windows::Win32::System::Console::SetConsoleOutputCP;
use windows::Win32::System::Console::CONSOLE_MODE;
use windows::Win32::System::Console::DISABLE_NEWLINE_AUTO_RETURN;
use windows::Win32::System::Console::ENABLE_ECHO_INPUT;
use windows::Win32::System::Console::ENABLE_INSERT_MODE;
use windows::Win32::System::Console::ENABLE_LINE_INPUT;
use windows::Win32::System::Console::ENABLE_PROCESSED_INPUT;
use windows::Win32::System::Console::ENABLE_PROCESSED_OUTPUT;
use windows::Win32::System::Console::ENABLE_VIRTUAL_TERMINAL_INPUT;
use windows::Win32::System::Console::ENABLE_VIRTUAL_TERMINAL_PROCESSING;
use windows::Win32::System::Console::ENABLE_WINDOW_INPUT;
use windows::Win32::System::Console::STD_INPUT_HANDLE;
use windows::Win32::System::Console::STD_OUTPUT_HANDLE;

const UNICODE_UTF8_CP_ID: u32 = 65001;

pub struct Console {
    orig_con_cp: u32,
    orig_con_ocp: u32,
    orig_in_mode: CONSOLE_MODE,
    orig_out_mode: CONSOLE_MODE,
    stdin_handle: Arc<SafeHandle>,
    stdout_handle: Arc<SafeHandle>,
}

unsafe impl Send for Console {}
unsafe impl Sync for Console {}

pub struct SafeHandle(HANDLE);

unsafe impl Send for SafeHandle {}
unsafe impl Sync for SafeHandle {}

impl Console {
    pub fn new() -> windows::core::Result<Self> {
        let orig_con_cp = unsafe { GetConsoleCP() };
        let orig_con_ocp = unsafe { GetConsoleOutputCP() };

        let stdin_handle = unsafe { GetStdHandle(STD_INPUT_HANDLE)? };
        let stdout_handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE)? };

        let mut orig_in_mode = CONSOLE_MODE(0);
        unsafe { GetConsoleMode(stdin_handle, &mut orig_in_mode)? };

        let mut orig_out_mode = CONSOLE_MODE(0);
        unsafe { GetConsoleMode(stdout_handle, &mut orig_out_mode)? };

        Ok(Self {
            orig_con_cp,
            orig_con_ocp,
            orig_in_mode,
            orig_out_mode,
            stdin_handle: Arc::new(SafeHandle(stdin_handle)),
            stdout_handle: Arc::new(SafeHandle(stdout_handle)),
        })
    }

    pub fn restore(&self) -> windows::core::Result<()> {
        unsafe {
            SetConsoleCP(self.orig_con_cp)?;
            SetConsoleOutputCP(self.orig_con_ocp)?;
            SetConsoleMode(self.stdin_handle.0, self.orig_in_mode)?;
            SetConsoleMode(self.stdout_handle.0, self.orig_out_mode)?;
        }
        Ok(())
    }

    pub fn setup(&self) -> windows::core::Result<()> {
        unsafe {
            SetConsoleCP(UNICODE_UTF8_CP_ID)?;
            SetConsoleOutputCP(UNICODE_UTF8_CP_ID)?;
        }
        let mut mode =
            !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_INSERT_MODE | ENABLE_PROCESSED_INPUT)
                | ENABLE_WINDOW_INPUT;
        mode &= self.orig_in_mode | ENABLE_VIRTUAL_TERMINAL_INPUT;
        unsafe {
            match SetConsoleMode(self.stdin_handle.0, mode) {
                Ok(_) => (),
                Err(e) => {
                    eprintln!("Failed to set console in mode: {:?}", e);
                }
            }
        }

        mode = self.orig_out_mode
            | ENABLE_PROCESSED_OUTPUT
            | ENABLE_VIRTUAL_TERMINAL_PROCESSING
            | DISABLE_NEWLINE_AUTO_RETURN;
        unsafe {
            match SetConsoleMode(self.stdout_handle.0, mode) {
                Ok(_) => (),
                Err(e) => {
                    eprintln!("Failed to set console out mode: {:?}", e);
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    pub fn write(&self, buffer: &[u8]) -> windows::core::Result<u32> {
        let mut bytes_written: u32 = buffer.len() as u32;
        match unsafe {
            WriteFile(
                self.stdout_handle.0,
                Some(buffer),
                Some(&mut bytes_written),
                None,
            )
        } {
            Err(e) => Err(e),
            Ok(_) => Ok(bytes_written),
        }
    }

    pub fn read(&self, buffer: &mut Vec<u8>) -> windows::core::Result<u32> {
        let mut bytes_read: u32 = 0;
        match unsafe {
            ReadFile(
                self.stdin_handle.0,
                Some(buffer),
                Some(&mut bytes_read),
                None,
            )
        } {
            Err(e) => Err(e),
            Ok(_) => Ok(bytes_read),
        }
    }
}
