This guide is meant for people wishing to contribute to this open-source project. For more information on contributing, see [CONTRIBUTING](CONTRIBUTING.md).

## Prerequisites

### Rust

You need at least **Rust 1.70.0** to build this project's code and run the tests. You can install Rust from the [official website](https://www.rust-lang.org/tools/install).
If you already have a version of Rust installed via `rustup` but it's too old, you can update by running

```bash
rustup update
```

### Rust nightly

In order to run `rustfmt`, you will need a Nightly Rust toolset. If you do not have one installed, you can install one via `rustup` by running

```bash
rustup toolchain install nightly
```

If you already have one installed but it was too old, it was probably updated earlier when you ran `rustup update` 😉

### Just

[just](https://github.com/casey/just) is a command-line tool to run scripts, a bit like `npm`'s scripts. It's written in Rust.

This project includes a [justfile](justfile) that makes it easier to run the various tools used for development. To install `just` via `cargo`, simply run

```bash
cargo install just
```

If you have [cargo-binstall](https://github.com/cargo-bins/cargo-binstall), it'll probably be faster to use it instead:

```bash
cargo binstall just
```

You can also install it via various [methods](https://github.com/casey/just#packages).

### Tarpaulin

If you want to run tests with coverage locally, you'll need to install [`cargo-tarpaulin`](https://github.com/xd009642/tarpaulin), a code coverage tool for Rust. You can install it via `cargo`:

```bash
cargo install cargo-tarpaulin
```
You can also install it via [cargo-binstall](https://github.com/cargo-bins/cargo-binstall):

```bash
cargo binstall cargo-tarpaulin
```

## Development

### Running the tests

In order to run all tests, you can use

```bash
just test
```

Any new feature or bug fix would need new tests to validate. Make sure all tests pass before submitting a PR.

### Linting

Before submitting a PR, make sure `rustfmt` and `clippy` are happy. To tidy up your code before committing, simply run

```bash
just tidy
```

Required checks will not pass if either of those report issues.

### Code coverage

This project's [code coverage settings](codecov.yml) are pretty stringent and require **100% coverage**. To validate this locally, you can run

```bash
just tarpaulin
```

Make sure coverage is at the required level before submitting a PR.

## Questions?

If any part of this documentation is unclear, please open a [new issue](https://github.com/clechasseur/exsb/issues/new/choose) so it can be fixed.
