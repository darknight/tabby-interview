# tabby-interview

## Get Started

## Project Structure

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

## Improvement
- Add command line argument to control
  - log level, log output file
  - if overwrite or skip same files
- Increase test coverage
- Compare file content to avoid unnecessary copy (for example, md5 or sha256 checksum)
- For large files, split file into chunks and send them separately
- Use more performant message serialization format (for example, protobuf)
- Sanitize file path (dir/./subdir/../ -> dir/)
