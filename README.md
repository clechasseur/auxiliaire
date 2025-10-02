# auxiliaire

[![CI](https://github.com/clechasseur/auxiliaire/actions/workflows/ci.yml/badge.svg?branch=main&event=push)](https://github.com/clechasseur/auxiliaire/actions/workflows/ci.yml) [![codecov](https://codecov.io/gh/clechasseur/auxiliaire/graph/badge.svg?token=roYWrCvQVx)](https://codecov.io/gh/clechasseur/auxiliaire) [![Security audit](https://github.com/clechasseur/auxiliaire/actions/workflows/audit-check.yml/badge.svg?branch=main)](https://github.com/clechasseur/auxiliaire/actions/workflows/audit-check.yml) [![crates.io](https://img.shields.io/crates/v/auxiliaire.svg)](https://crates.io/crates/auxiliaire) [![downloads](https://img.shields.io/crates/d/auxiliaire.svg)](https://crates.io/crates/auxiliaire) [![Contributor Covenant](https://img.shields.io/badge/Contributor%20Covenant-2.1-4baaaa.svg)](CODE_OF_CONDUCT.md)

`auxiliaire` is a command-line tool designed to provide utilities to users of the [Exercism.org](https://exercism.org) website, like solutions backup, etc.

## Exerci-what?

[Exercism](https://exercism.org) is a free, not-for-profit platform to learn new programming languages.
It supports a web editor for solving exercises, mentoring with real humans and a lot more.
For more information, see [its about page](https://exercism.org/about).

## Installing

Installing and using `auxiliaire` can be done simply by downloading the executable appropriate for your platform from the [project's Releases page](https://github.com/clechasseur/auxiliaire/releases) and saving it to a location in your PATH.

If you have Rust **1.85.1** or greater installed, you can also compile and install `auxiliaire` from source via `cargo`:

```sh
cargo install auxiliaire --locked
```

If you have [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall), you can use it to download and install `auxiliaire` from binaries:

```sh
cargo binstall auxiliaire
```

## Usage

To see all commands supported by `auxiliaire`, simply run it with `-h` (for short help) or `--help` (for long help).

### `backup` command

This command can be used to download all solutions you submitted to the Exercism platform for backup.

```sh
% auxiliaire backup -h
Download Exercism.org solutions for backup

Usage: auxiliaire backup [OPTIONS] <PATH>

Arguments:
  <PATH>  Path where to store the downloaded solutions

Options:
      --token <TOKEN>
          Exercism.org API token; if unspecified, CLI token will be used instead
  -v, --verbose...
          Increase logging verbosity
  -q, --quiet...
          Decrease logging verbosity
  -t, --track <TRACK>
          Only download solutions in the given track(s) (can be used multiple times)
  -e, --exercise <EXERCISE>
          Only download solutions for the given exercise(s) (can be used multiple times)
  -s, --status <STATUS>
          Only download solutions with the given status (or greater) [default: any] [possible values: any, submitted, completed, published]
  -o, --overwrite <OVERWRITE>
          How to handle solutions that already exist on disk [default: if-newer] [possible values: always, if-newer, never]
  -i, --iterations <ITERATIONS_SYNC_POLICY>
          Whether to also back up iterations and how [default: do-not-sync] [possible values: do-not-sync, new, full-sync, clean-up]
      --dry-run
          Determine what solutions to back up without downloading them
  -m, --max-downloads <MAX_DOWNLOADS>
          Maximum number of concurrent downloads [default: 4]
  -h, --help
          Print help (see more with '--help')
```

By default, using this command will download all submitted solutions, for all exercises, for all tracks.
It's possible to narrow the solutions to back up via the command-line arguments (see above).

When `auxiliaire` downloads a solution, it stores a backup state file in the solution folder in the `.auxiliaire` directory.
This file is used to determine whether a solution has been updated with (a) new iteration(s).
When this occurs, by default, `auxiliaire` will download the new version; this can be controlled via the `--overwrite` argument.

It is also possible to download _every_ iteration of each solution via the `--iterations` argument.
Iterations will be stored in a subdirectory called `_iterations`.
All iterations submitted will be downloaded, unless `--status published` is used, in which case only published iterations will be kept.

In order to communicate with the Exercism platform, `auxiliaire` needs an API token.
By default, if the [Exercism CLI tool](https://exercism.org/docs/using/solving-exercises/working-locally) is installed, `auxiliaire` will reuse the API token configured for it.
If the Exercism CLI is not installed, a valid API token will need to be passed to `auxiliaire` via the `--token` argument.
This token can be found in the [Exercism Settings](https://exercism.org/settings/api_cli).

## Questions? Comments?

`auxiliaire` is still in development, so issues may arise.
For instructions on filing bug reports or feature requests, see [CONTRIBUTING](./CONTRIBUTING.md).
