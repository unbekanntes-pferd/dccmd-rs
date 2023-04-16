# DRACOON Commander RS

## What is this?
This is a port of [DRACOON Commander](https://github.com/unbekanntes-pferd/dccmd) - initially a Python3 project to use DRACOON via CLI.
The project serves to define a Rust DRACOON API wrapper.

### Built with
This project makes use of several awesome crates and uses async Rust throughout the project.
Crates used:
- [reqwest](https://crates.io/crates/reqwest)
- [clap](https://crates.io/crates/reqwest)
- [console](https://crates.io/crates/console)
- [dialoguer](https://crates.io/crates/console)
- [indicatif](https://crates.io/crates/console)

Full dependency list: [Cargo.toml](Cargo.toml)

For cryptography, the experimental crate `dco3-crypto` is used.

- [dco3-crypto](https://github.com/unbekanntes-pferd/dco3-crypto)

## Installation

Currently, only builds from source are supported (no official version, therefore no pre-compiled builds).

To get this running, clone the repository and either use `cargo run` or build your own executable with `cargo build`:

```bash
git clone https://github.com/unbekanntes-pferd/dccmd-rs.git
cd dccmd-rs
cargo build
```

## What works?

Currently, the following commands are working:

- `download` - downloads a file to a desired file on disk (encrypted, unencrypted)
- `ls` - lists all nodes for a given path
- `mkdir` - creates a folder in given path

## What is not working?

- Upload is **not** implemented (APIs not ready)
- Recursive download is not implemented

## Example usage

For the sake of clarity, the usage of the binary is called `dccmd`, regardless of the use via `cargo` or a compiled executable.

### Downloads

To download a file, use the download command:

```bash
dccmd download your.dracoon.domain/some/room/some-file.pdf ./your/path/your-name.pdf
```

### Listing nodes
To list nodes, use the `ls` command:

```
dccmd ls your.dracoon.domain/some/path

// for root node use a trailing slash
dccmd ls your.dracoon.domain/
```

Options:
 - `-l`, `--long` - prints all details (size, updated by, node id...)           
 - `-r`, `--human-readable` - prints size in human readable format
 -    `--managed` - shows room as room admin / room manager (rooms w/o permissions)       
 -    `--all` - fetches all items (default: first 500 items)


### Deleting nodes

To delete nodes, use the `rm` command:

```
dccmd rm your.dracoon.domain/some/path

```

### Creating folders

To create folders, use the `mkdir` command:

```
dccmd mkdir your.dracoon.domain/some/path/newfolder

```