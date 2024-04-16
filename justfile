set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

toolchain := ""
tool := "cargo"

cargo := tool + (if toolchain != "" { " +" + toolchain } else { "" })

all_features := "true"
all_features_flag := if all_features == "true" { "--all-features" } else { "" }

all_targets := "true"
all_targets_flag := if all_targets == "true" { "--all-targets" } else { "" }

message_format := ""
message_format_flag := if message_format != "" { "--message-format " + message_format } else { "" }

target_tuple := ""
target_tuple_flag := if target_tuple != "" { "--target " + target_tuple } else { "" }

# Note: there seems to be an issue in `cargo-tarpaulin` when using Rust 1.75.0 or later - it reports some missing line coverage.
# I've entered an issue: https://github.com/xd009642/tarpaulin/issues/1438
# In the meantime, let's pin the Rust version used for code coverage to 1.74.1 until we know what's happening.
cargo_tarpaulin := tool + (if toolchain != "" { " +" + toolchain } else { " +1.74.1" }) + " tarpaulin"

[private]
default:
    @just --list

# Run clippy and rustfmt on workspace files
tidy: clippy fmt

# Run clippy on workspace files
clippy:
    {{cargo}} clippy --workspace {{all_targets_flag}} {{all_features_flag}} {{target_tuple_flag}} -- -D warnings

# Run rustfmt on workspace files
fmt:
    cargo +nightly fmt --all

# Run `cargo check` on workspace
check *extra_args:
    {{cargo}} check --workspace {{all_targets_flag}} {{all_features_flag}} {{message_format_flag}} {{target_tuple_flag}} {{extra_args}}

# Run `cargo build` on workspace
build *extra_args:
    {{cargo}} build --workspace {{all_targets_flag}} {{all_features_flag}} {{message_format_flag}} {{target_tuple_flag}} {{extra_args}}

# Run `cargo test` on workspace
test *extra_args:
    {{cargo}} test --workspace {{all_features_flag}} {{message_format_flag}} {{target_tuple_flag}} {{extra_args}}

# Run `cargo update` to update dependencies in Cargo.lock
update *extra_args:
    {{cargo}} update {{extra_args}}

# Run `cargo tarpaulin` to produce code coverage
@tarpaulin *extra_args:
    @{{cargo_tarpaulin}} --target-dir target-tarpaulin {{extra_args}}
    {{ if env('CI', '') == '' { `just _open-tarpaulin` } else { ` ` } }}

[unix]
@_open-tarpaulin:
    open tarpaulin-report.html

[windows]
@_open-tarpaulin:
    ./tarpaulin-report.html

# Generate documentation with rustdoc
doc: _doc

_doc $RUSTDOCFLAGS="-D warnings":
    {{cargo}} doc {{ if env('CI', '') != '' { '--no-deps' } else { '--open' } }} --workspace {{all_features_flag}} {{message_format_flag}}

# Check doc coverage with Nightly rustdoc
doc-coverage: _doc-coverage

_doc-coverage $RUSTDOCFLAGS="-Z unstable-options --show-coverage":
    cargo +nightly doc --no-deps --workspace {{all_features_flag}} {{message_format_flag}}

[private]
minimize:
    {{cargo}} hack --remove-dev-deps --workspace
    cargo +nightly update -Z minimal-versions

# Run `cargo minimal-versions check` on workspace
check-minimal: prep _check-minimal-only && (_rimraf "target-minimal") unprep

_check-minimal-only: (_rimraf "target-minimal")
    {{cargo}} minimal-versions check --target-dir target-minimal --workspace --lib --bins {{all_features_flag}} {{message_format_flag}}

# Run `cargo msrv` with `cargo minimal-versions check`
msrv-minimal: (prep "--manifest-backup-suffix .msrv-prep.outer.bak") && (_rimraf "target-minimal") (unprep "--manifest-backup-suffix .msrv-prep.outer.bak")
    {{cargo}} msrv -- just all_features="{{all_features}}" message_format="{{message_format}}" target_tuple="{{target_tuple}}" _check-minimal-only

# Run `cargo msrv` with `cargo check`
msrv *extra_args: && (_rimraf "target-msrv")
    {{cargo}} msrv -- just all_features="{{all_features}}" all_targets="{{all_targets}}" message_format="{{message_format}}" target_tuple="{{target_tuple}}" _msrv-check {{extra_args}}

_msrv-check *extra_args: (_rimraf "target-msrv")
    just all_features="{{all_features}}" all_targets="{{all_targets}}" message_format="{{message_format}}" target_tuple="{{target_tuple}}" check --target-dir target-msrv {{extra_args}}

# Perform `cargo publish` dry-run
test-package *extra_args:
    {{cargo}} publish --dry-run {{extra_args}}

# Run `cargo msrv-prep` on workspace
prep *extra_args:
    {{cargo}} msrv-prep --workspace {{extra_args}}

# Run `cargo msrv-unprep` on workspace
unprep *extra_args:
    {{cargo}} msrv-unprep --workspace {{extra_args}}

# ----- Utilities -----

@_rimraf target_dir:
    {{ if path_exists(target_dir) == "true" { "just _rimraf-it '" + target_dir + "'" } else { "" } }}

[unix]
@_rimraf-it target_dir:
    rm -rf '{{target_dir}}'

[windows]
@_rimraf-it target_dir:
    Remove-Item "{{target_dir}}" -Recurse
