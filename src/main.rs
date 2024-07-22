// Copyright 2024 Qi, Yadong.
// SPDX-License-Identifier: Apache-2.0

use std::io;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;

use clap::Parser;

use windows::Win32::Foundation::ERROR_PIPE_NOT_CONNECTED;

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

fn stdin_to_pipe(pipe: named_pipe::NamedPipe) -> Result<(), std::io::Error> {
    let stdin = io::stdin();

    loop {
        let mut buf: Vec<u8> = vec![0u8; 1024];
        let n = match stdin.lock().read(&mut buf) {
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => {
                continue;
            }
            Err(e) => {
                eprintln!("Failed to read from stdin: {:?}", e);
                break Err(e);
            }
        };
        if n == 0 {
            break Ok(());
        }

        if n >= 2 && b'\n' == buf[n - 1] && b'\r' == buf[n - 2] {
            buf.pop();
        }

        pipe.write(&buf)?;
    }
}

fn pipe_to_stdout(
    pipe: named_pipe::NamedPipe,
    path: &Option<PathBuf>,
) -> windows::core::Result<()> {
    let mut stdout = io::stdout();
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
        pipe.read(&mut buffer)?;
        let s = String::from_utf8_lossy(&buffer);

        stdout.lock().write_all(s.as_bytes())?;
        stdout.flush()?;
        if let Some(ref mut file) = redir_file {
            file.write_all(s.as_bytes())?;
        }
    }
}

fn main() {
    let args = Args::parse();

    println!("Pipe connecting: {:?}", args.path);
    let pipe_stp = {
        match named_pipe::NamedPipe::try_open(&args.path, args.wait) {
            Ok(pipe) => pipe,
            Err(e) => {
                eprintln!("Failed to open pipe: {:?}", e);
                return;
            }
        }
    };

    println!("Pipe connected: {:?}", args.path);
    let pipe_pts = pipe_stp.clone();

    let _th_stdin_to_pipe = std::thread::spawn(move || match stdin_to_pipe(pipe_stp) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Error in stdin_to_pipe: {:?}", e);
        }
    });

    let th_pipe_to_stdout =
        std::thread::spawn(move || match pipe_to_stdout(pipe_pts, &args.redir) {
            Ok(_) => {}
            Err(e) if e == ERROR_PIPE_NOT_CONNECTED.into() => {
                eprintln!("Pipe disconnected: {:?}, hresult={}", e.message(), e.code());
            }
            Err(e) => {
                eprintln!("Error in pipe_to_stdout: {:?}", e);
            }
        });

    //th_stdin_to_pipe.join().unwrap();
    th_pipe_to_stdout.join().unwrap();
}
