# A tool to monitor named pipe on Windows

## Build
```
cargo build
```

## Usage
```
Usage: winpipe.exe [OPTIONS] --path <PATH>

Options:
  -p, --path <PATH>   path of named pipe
  -w, --wait          whether to wait for the pipe be ready
  -r, --redir <PATH>  path of file to redirect
  -h, --help          Print help
  -V, --version       Print version
```
