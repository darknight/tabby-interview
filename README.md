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

After pulling this project [repo](https://github.com/darknight/tabby-interview.git)

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

This is a reasonable decision, since we expect the receiver's directory is the same as the sender's one.

If there are two senders sending files, this will cause data corruption.

Fortunately, `tokio` provide async version of `Semaphore` to help us.

In receiver implementation, before accepting a new connection, we acquire a permit from the semaphore, subsequent connection will be waiting for the permit to be released.

### Message ordering

On sender side, for large files, we split file into chunks.

In order to be able to re-create the file on receiver side, we need to make sure the chunks are sent in order.

From `tokio::sync::mpsc::channel` documentation, it says:

```
All data sent on Sender will become available on Receiver in the same order as it was sent.
```

As for the network transmission, websocket relies on TCP, and TCP is a stream-oriented protocol.

So as long as there's no proxy or middleman to re-order the data, we can assume that the data will be received in order.

So the file **append** operation on receiver side is safe.

### Send large files

For large files, if we load the whole file into memory, then send it. It will consume too much memory and probably cause OOM.

To make program more efficient and stable, we split the file into chunks, then send them separately.

Based on the explanation of message ordering, we can assume that the chunks will be received in order.

So on receiver side, what we need to do is open the file in append mode, then write the chunks.

### PID file in receiver's directory

To make sure there's only one receiver running for specified output directory, we create a PID file in that directory.

When receiver starts, it will check if the PID file exists, if it does, which means there might be another receiver using current directory, this program will exist.

### Graceful shutdown

To quit sender/receiver gracefully, we listen to `SIGINT` signal, `tokio` has built-in support for this.

When receiver quits, it will remove the PID file generated when it starts.

### Symbolic links

Symbolic links are skipped during syncing.

## Architecture

### Layered Design

Both sender and receiver are implemented in a layered design.

- The top layer is the entity exposed to the user, which is the `Sender` and `Receiver` struct.
- The middle layer is the handler which process application logic.
- The bottom layer is the transport layer, which is the websocket connection.

### Project Structure

The project contains four crates:

- `ws-cli`: implementation for `sync-directory` command
- `ws-common`: common code shared by other crates
- `ws-receiver`: implementation for receiver
- `ws-sender`: implementation for sender

### Workflow



## Program verification

- start receiver
- quit receiver, check PID file

- start receiver again
- start sender
- start another sender

## Limitations & Improvement

I understand that the current implementation is far from perfect, there're still a lot of things to improve.

To name a few I think are important:

### Write completion notification

Currently, when sender is done syncing, it will exist instead keep the connection alive.

My idea is to count the number of file entries sent, and break from the loop when all the entries are sent.

Due to time limit, I didn't implement this.

### Concurrent write on receiver side

Currently, the receiver has the pattern:

- `read message from ws -> process message -> send process result to ws`

The next read has be to waited until the previous `process-send` is done.

Actually, it's not necessary, especially when writing different files.

We can make the file write concurrently on receiver side, just like concurrent read on sender side.

This may require more effort to implement, but it's definitely worth it.

### Logging & Tracing & Metrics

I understand these are essential for production program.

Luckily, Rust ecosystem has good support for these.

For example:

- https://github.com/rust-lang/log
- https://github.com/tokio-rs/tracing
- https://github.com/tikv/rust-prometheus

### Integration test

Async, IO-intensive program is difficult to write unit test, the function are side effect, and the execution order is not deterministic. And we need to mock a lot of things, like network connection, filesystem etc.

So I think integration test is more suitable, and necessary.

### Send with deduplication

- Compare file content to avoid unnecessary copy (for example, md5 or sha256 checksum)

### Message serialization format & compression

- Use more performant message serialization format (for example, protobuf)

- Compress text message (for example, gzip)

### Code improvement

- Add command line argument to control `log level`, `log destination`, `max current read/write`, `overwrite option` etc.

- Increase test coverage

- Sanitize file path (dir/./subdir/../ -> dir/)
