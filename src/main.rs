// Copyright 2024 Qi, Yadong.
// SPDX-License-Identifier: Apache-2.0

use std::io;
use std::path::PathBuf;
use std::process;
use std::time::Duration;

use clap::Parser;

use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::windows::named_pipe;
use tokio::time;

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

#[tokio::main]
async fn aync_pipe_io(pipe: &str, wait: bool, redir: Option<PathBuf>) -> io::Result<()> {
    println!("Pipe connecting: {}", pipe);
    let client = loop {
        match named_pipe::ClientOptions::new().open(pipe) {
            Ok(client) => break client,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound && wait {
                    time::sleep(Duration::from_millis(50)).await;
                } else {
                    return Err(e);
                }
            }
        };
    };
    println!("Pipe connected: {}", pipe);

    let (mut reader, mut writer) = tokio::io::split(client);

    let stdin_to_pipe = async {
        let mut stdin = tokio::io::stdin();
        let mut buf = [0; 1024];
        loop {
            let n = stdin.read(&mut buf).await?;

            if n == 0 {
                break;
            }
            writer.write_all(&buf[..n]).await?;
            writer.flush().await?;
        }
        io::Result::Ok(())
    };

    let pipe_to_stdout = async {
        let mut stdout = tokio::io::stdout();
        let mut buf = vec![0; 1024];

        let mut redir_file = match redir {
            Some(path) => {
                match tokio::fs::File::options()
                    .append(true)
                    .create(true)
                    .open(path.clone())
                    .await
                {
                    Ok(file) => Some(file),
                    Err(e) => {
                        eprintln!("Invalid file to redirect: {:?}, err={}", path, e);
                        None
                    }
                }
            }
            None => None,
        };

        loop {
            let n = reader.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            stdout.write_all(&buf[..n]).await?;
            stdout.flush().await?;
            if let Some(ref mut file) = redir_file {
                file.write_all(&buf[..n]).await?
            }
        }
        io::Result::Ok(())
    };

    tokio::select! {
        r = stdin_to_pipe => {
            match r {
                Ok(_) => (),
                Err(e) => {
                    eprintln!("stdin_to_pipe: Error: {}", e);
                }
            }
        },
        r = pipe_to_stdout => {
            match r {
                Ok(_) => (),
                Err(e) => {
                    eprintln!("pipe_to_stdout: Error: {}", e);
                    // From comments of tokio::io::Stdin, for technical reasons, the
                    // shutdown of the runtime hang until user presses enter.
                    // So here force to exit whole process to workaround this issue.
                    // Reference: https://docs.rs/tokio/latest/tokio/io/struct.Stdin.html
                    process::exit(1);
                }
            }
        }
    };

    Ok(())
}

fn main() {
    let args = Args::parse();

    match aync_pipe_io(&args.path, args.wait, args.redir) {
        Ok(_) => (),
        Err(e) => eprintln!("Error: {}", e),
    }
}
