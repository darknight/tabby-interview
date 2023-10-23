# tabby-interview

The interview question is described [here](https://github.com/TabbyML/interview-questions/tree/main/301_sync_directory_over_websocket).

## Get Started

### development environment

`rustup show`:

```
active toolchain
----------------

stable-x86_64-pc-windows-msvc (default)
rustc 1.72.0 (5680fa18f 2023-08-23)
```

### build

After pulling this [repo](https://github.com/darknight/tabby-interview.git)

```bash
cd tabby-interview
cargo build
```

### run receiver

After build, the executable file `.\target\debug\sync-directory.exe` will be generated.

```bash
.\target\debug\sync-directory.exe --port 9000 --output-dir .\recv_dir
```

The `recv_dir` will be created under the current directory (project root, e.g. `.\tabby-interview`)
if it doesn't exist.

### run sender

After receiver is running up, open another shell, and run

```bash
.\target\debug\sync-directory.exe --to ws://localhost:9000 --from .\crates
```

This will sync all the contents in `.\crates`(actually the source codes in this project) to `.\recv_dir`.

### run unit tests

```bash
cargo test --workspace
```

## Assumptions

Based on the minimum requirements described in the question, I made two assumptions for each of them.

I'm not quite sure if the assumptions are reasonable or not, but these indeed help me to finalize the design and implementation.

We can discuss the assumptions during the interview if necessary.

1. Implement the "sync" semantic.

   It's missing one condition that a file exists in both sender's and receiver's directory. In this condition, I'll just **overwrite** the file in receiver's directory.

   This is definitely not ideal. If a file in the receiver's side is exactly the same as in the sender's one, then the copy is unnecessary. But I think it's acceptable for now.

2. Directory syncing should be performed recursively.

   By saying `recursively` my understanding is that we want to sync all the files in the directory and its subdirectories and so on.

   It's **NOT** related to implementation details. Based on what I know, Rust doesn't support tail recursion optimization officially ([only as experimental feature](https://github.com/rust-lang/rust/issues/112788)), which means there's a risk for recursive traversing to get stack overflow if the directory is too deep. So it's better not use recursion in the implementation.

Besides, I also made some other assumptions for the implementation.

- Skip syncing symbolic links, cause the `symlink` in one filesystem might not be valid in another one.
  So there's no point to sync it.

## Design Decision

### Only one sender connection

### Graceful shutdown

### Send large files

#### Message ordering

On sender side, for large files, we split file into chunks.

In order to be able to re-create the file on receiver side, we need to make sure the chunks are sent in order.

From `tokio::sync::mpsc::channel` documentation, it says:

```
All data sent on Sender will become available on Receiver in the same order as it was sent.
```

As for the network transmission, websocket relies on TCP, and TCP is a stream-oriented protocol.

So as long as there's no proxy or middleman to re-order the data, we can assume that the data will be received in order.

So the file **append** operation on receiver side is safe.

### PID file in receiver's directory

### Symbolic links

## Architecture

### Project Structure

### Workflow

## Program verification

- start receiver
- quit receiver, check PID file

- start receiver again
- start sender
- start another sender

## Limitations & Improvement

### Write completion notification

- Add command line argument to control
  - log level, log output file
  - if overwrite or skip same files
- Increase test coverage
- Compare file content to avoid unnecessary copy (for example, md5 or sha256 checksum)
- For large files, split file into chunks and send them separately
- Use more performant message serialization format (for example, protobuf)
- Sanitize file path (dir/./subdir/../ -> dir/)
