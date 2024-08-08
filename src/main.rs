// Copyright 2024 Qi, Yadong.
// SPDX-License-Identifier: Apache-2.0

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use clap::Parser;

use log::error;
use log::info;
use log::warn;

use logger::setup_logger;

use windows::Win32::Foundation::ERROR_OPERATION_ABORTED;
use windows::Win32::Foundation::ERROR_PIPE_NOT_CONNECTED;
use windows::Win32::Foundation::STATUS_INTERRUPTED;

pub mod console;
pub mod logger;
pub mod named_pipe;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// path of named pipe
    #[arg(short, long)]
    path: String,

    /// whether to wait for the pipe be ready
    #[arg(short, long, default_value_t = false)]
    wait: bool,

    /// path of file to redirect
    #[arg(short, long, value_name="PATH", value_hint = clap::ValueHint::FilePath)]
    redir: Option<PathBuf>,
}

fn stdin_to_pipe(
    pipe: named_pipe::NamedPipe,
    con: Arc<console::Console>,
) -> Result<(), std::io::Error> {
    loop {
        let mut buf: Vec<u8> = vec![0u8; 1024];
        let n = match con.read(&mut buf) {
            Ok(n) => n,
            Err(e) if e.code() == STATUS_INTERRUPTED.into() => {
                info!("interrupted!");
                thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(e) if e.code() == ERROR_OPERATION_ABORTED.into() => {
                warn!("Operation aborted!");
                break Ok(());
            }
            Err(e) => {
                error!("Failed to read from stdin: {:?}", e);
                break Err(e.into());
            }
        };
        buf.truncate(n as usize);

        pipe.write(&buf)?;
    }
}

fn pipe_to_stdout(
    pipe: named_pipe::NamedPipe,
    con: Arc<console::Console>,
    path: &Option<PathBuf>,
) -> windows::core::Result<()> {
    let mut redir_file = match path {
        Some(path) => {
            let f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;
            Some(f)
        }
        None => None,
    };

    loop {
        let mut buffer: Vec<u8> = Vec::new();
        match pipe.read(&mut buffer) {
            Ok(n) => {
                if n == 0 {
                    thread::sleep(Duration::from_millis(100));
                    continue;
                }
            }
            Err(e) if e.code() == ERROR_PIPE_NOT_CONNECTED.into() => {
                warn!("Pipe disconnected: {:?}, hresult={}", e.message(), e.code());
                con.cancel_read()?;
                break Ok(());
            }
            Err(e) => {
                error!("Failed to read from pipe: {:?}", e);
                break Err(e);
            }
        }

        con.write(buffer.as_slice())?;
        if let Some(ref mut file) = redir_file {
            file.write_all(buffer.as_slice())?;
        }
    }
}

fn main() {
    let args = Args::parse();

    let _ = setup_logger(&args.redir);

    let con = Arc::new(match console::Console::new() {
        Ok(con) => con,
        Err(e) => {
            error!("Failed to create console: {:?}", e);
            return;
        }
    });

    match con.setup() {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to setup console: {:?}", e);
            return;
        }
    }

    info!("Pipe connecting: {:?}", args.path);
    let pipe_stp = {
        match named_pipe::NamedPipe::try_open(&args.path, args.wait) {
            Ok(pipe) => pipe,
            Err(e) => {
                error!("Failed to open pipe: {:?}", e);
                return;
            }
        }
    };

    info!("Pipe connected: {:?}", args.path);
    let pipe_pts = pipe_stp.clone();

    let arc_con_r = Arc::clone(&con);
    let th_stdin_to_pipe = std::thread::spawn(move || match stdin_to_pipe(pipe_stp, arc_con_r) {
        Ok(_) => {}
        Err(e) => {
            error!("Error in stdin_to_pipe: {:?}", e);
        }
    });

    let arc_con_w = Arc::clone(&con);
    let th_pipe_to_stdout =
        std::thread::spawn(
            move || match pipe_to_stdout(pipe_pts, arc_con_w, &args.redir) {
                Ok(_) => {}
                Err(e) => {
                    error!("Error in pipe_to_stdout: {:?}", e);
                }
            },
        );

    th_pipe_to_stdout.join().unwrap();
    th_stdin_to_pipe.join().unwrap();

    match con.restore() {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to restore console: {:?}", e);
        }
    }
}
