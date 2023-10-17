# DRACOON Commander RS

## What is this?
This is a port of [DRACOON Commander](https://github.com/unbekanntes-pferd/dccmd) - initially a Python3 project to use DRACOON via CLI.
The project serves as a demo client implementation using `dco3` - an API wrapper in Rust for DRACOON. 

### Built with
This project makes use of several awesome crates and uses async Rust throughout the project.
Crates used:
- [reqwest](https://crates.io/crates/reqwest)
- [clap](https://crates.io/crates/reqwest)
- [console](https://crates.io/crates/console)
- [dialoguer](https://crates.io/crates/console)
- [indicatif](https://crates.io/crates/console)

Full dependency list: [Cargo.toml](Cargo.toml)

For all DRACOON operations `dco3` is used.

- [dco3](https://github.com/unbekanntes-pferd/dco3)

## Installation

You can install this using cargo like so:

```bash
cargo install dccmd-rs
```

You can also download precompiled binaries on the Github releases page: 
[Releases](https://github.com/unbekanntes-pferd/dccmd-rs/releases)

If you like it rough, feel free to compile from source:

Clone the repository and either use `cargo run` or build your own executable with `cargo build`:

```bash
git clone https://github.com/unbekanntes-pferd/dccmd-rs.git
cd dccmd-rs
cargo build
```

## What works?

Currently, the following commands are working:

- `download` - downloads a file or folder / room from DRACOON to a desired location on disk (encrypted, unencrypted)
- `upload` - uploads a file or folder to a parent in DRACOON (encrypted, unencrypted)
- `ls` - lists all nodes for a given path in DRACOON
- `mkdir` - creates a folder in given path in DRACOON
- `mkroom` - creates a room (inherits permissions) in given path in DRACOON
- `rm` - removes a node by given path in DRACOON

## Example usage

For the sake of clarity, the usage of the binary is called `dccmd-rs`, regardless of the use via `cargo` or a compiled executable.

### Downloads

To download a file, use the download command:

```bash
dccmd-rs download your.dracoon.domain/some/room/some-file.pdf ./your/path/your-name.pdf
```

To download a container (room or folder), use the download command with recursive flag:

```bash
dccmd-rs download -r your.dracoon.domain/some/room ./your/path
```
**Note**: This will create a directory with same name as your container. Sub rooms are **not** included.

To download a list search result, use the download command with a search string:

```bash
dccmd-rs download your.dracoon.domain/some/*.pdf ./your/path
```

### Uploads

To upload a file, use the upload command:

```bash
dccmd-rs upload ./your/path/your-name.pdf your.dracoon.domain/some/room
```

**Note:** Currently, providing a custom name is not implemented.

To upload a folder, use the `--recursive` flag:

```bash
dccmd-rs upload /your/path your.dracoon.domain/some/room
```
**Note:** Currently only absolute paths are supported for recursive uploads.

### Listing nodes
To list nodes, use the `ls` command:

```
dccmd-rs ls your.dracoon.domain/some/path

// for root node use a trailing slash
dccmd-rs ls your.dracoon.domain/

// for searches within the room
dccmd-rs ls your.dracoon.domain/*.pdf 
```

Options:
 - `-l`, `--long` - prints all details (size, updated by, node id...)           
 - `-r`, `--human-readable` - prints size in human readable format
 -    `--managed` - shows room as room admin / room manager (rooms w/o permissions)       
 -    `--all` - fetches all items (default: first 500 items)


### Deleting nodes

To delete nodes, use the `rm` command:

```
dccmd-rs rm your.dracoon.domain/some/path/some_file.pdf
dccmd-rs rm -r your.dracoon.domain/some/path/some/room
```
*Note*: If you intend to delete a container (room or folder), use the recursive flag.
*Note*: Room deletion always requires additional confirmation.

### Creating folders

To create folders, use the `mkdir` command:

```
dccmd-rs mkdir your.dracoon.domain/some/path/newfolder

```


To create rooms, use the `mkroom` command:

```
dccmd mkroom your.dracoon.domain/some/path/newfolder

```
*Note*: Rooms can currently only be created as inheriting permissions from parent.

### CLI mode

Currently dccmd-rs will fail to store credentials if you are running a headless Linux or are trying to run in Windows with WSL.
In such cases you can pass the username and password as arguments like so:

```
dccmd-rs --username your_username --password your_secure_password ls your.dracoon.domain/some/path

```

Use this at your own risk and be aware that the password is stored in plain in your shell history.
*Note*: This only works for the password flow - this means you **must** use a local user. 

This also works for the encryption password like so: 

```
dccmd-rs --username your_username --password your_secure_password --encryption-password your_secure_encryption_password ls your.dracoon.domain/some/path

```