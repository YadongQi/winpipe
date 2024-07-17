// Copyright 2024 Qi, Yadong.
// SPDX-License-Identifier: Apache-2.0

use std::io;
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
}

#[tokio::main]
async fn aync_pipe_io(pipe: &str, wait: bool) -> io::Result<()> {
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
        loop {
            let n = reader.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            stdout.write_all(&buf[..n]).await?;
            stdout.flush().await?;
        }
        io::Result::Ok(())
    };

    tokio::try_join!(stdin_to_pipe, pipe_to_stdout)?;

    Ok(())
}

fn main() {
    let args = Args::parse();

    match aync_pipe_io(&args.path, args.wait) {
        Ok(_) => (),
        Err(e) => eprintln!("Error: {}", e),
    }
}
