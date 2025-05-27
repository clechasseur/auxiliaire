set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

toolchain := ""
tool := "cargo"

cargo := tool + (if toolchain != "" { " +" + toolchain } else { "" })
cargo_nightly := cargo + " +nightly"
cargo_hack := cargo + " hack"
cargo_tarpaulin := cargo + " tarpaulin"
cargo_llvm_cov := cargo_nightly + " llvm-cov"

all_features := "true"
all_features_flag := if all_features == "true" { "--all-features" } else { "" }
feature_powerset_flag := if all_features == "true" { "--feature-powerset" } else { "" }

all_targets := "true"
all_targets_flag := if all_targets == "true" { "--all-targets" } else { "" }

message_format := ""
message_format_flag := if message_format != "" { "--message-format " + message_format } else { "" }

target_tuple := ""
target_tuple_flag := if target_tuple != "" { "--target " + target_tuple } else { "" }

release := "false"
release_flag := if release == "true" { "--release" } else { "" }

workspace := "true"
package := ""
workspace_flag := if workspace != "true" { "" } else if package != "" { "" } else { "--workspace" }
all_flag := if workspace_flag == "" { "" } else { "--all" }
package_flag := if package != "" { "--package " + package } else { workspace_flag }
package_all_flag := if package != "" { "--package " + package } else { all_flag }
package_only_flag := if package != "" { "--package " + package } else { "" }

warnings_as_errors := "true"
clippy_flags := if warnings_as_errors == "true" { "-- -D warnings" } else { "" }

force_prep := "false"
force_prep_flag := if force_prep == "true" { "--force" } else { "" }

just := "just all_features=" + all_features + " all_targets=" + all_targets + " message_format=" + message_format + " target_tuple=" + target_tuple + " release=" + release + " workspace=" + workspace + " package=" + package + " warnings_as_errors=" + warnings_as_errors + " force_prep=" + force_prep

rustdoc_extra_flags := if toolchain == "nightly" { "--cfg docsrs" } else { "" }

skip_test_reqs_value := "running"

[private]
default:
    @just --list

@_before-test:
@_after-test:
@_check-test-reqs:
    echo '{{ skip_test_reqs_value }}'

@_run-tests *cmd:
    {{ if env('CI', '') != '' { cmd } else if `just _check-test-reqs` == skip_test_reqs_value { cmd } else { just + " _run-with-test-reqs " + cmd } }}

_run-with-test-reqs *cmd: _before-test && _after-test
    {{cmd}}

# Run an executable
run bin_name="" *extra_args: (_run-it "--bin" bin_name extra_args)

# Run an example
teach example_name *extra_args: (_run-it "--example" example_name extra_args)

_run-it run_param run_param_value *extra_args:
    {{cargo}} run {{package_only_flag}} {{all_features_flag}} {{target_tuple_flag}} {{release_flag}} {{ if run_param_value != '' { run_param + ' ' + run_param_value } else { '' } }} {{ if extra_args != '' { '-- ' + extra_args } else { '' } }}

# Run `cargo hack clippy` for the feature powerset and rustfmt
tidy: clippy fmt

# Run `cargo hack clippy` for the feature powerset
clippy *extra_args:
    {{cargo_hack}} clippy --target-dir target/clippy-target {{package_flag}} {{all_targets_flag}} {{feature_powerset_flag}} {{message_format_flag}} {{target_tuple_flag}} {{extra_args}} {{clippy_flags}}

# Run rustfmt
fmt *extra_args:
    {{cargo_nightly}} fmt {{package_all_flag}} {{message_format_flag}} {{extra_args}}

# Run `cargo check`
check *extra_args:
    {{cargo}} check {{package_flag}} {{all_targets_flag}} {{all_features_flag}} {{message_format_flag}} {{target_tuple_flag}} {{release_flag}} {{extra_args}}

# Run `cargo hack check` for the feature powerset
check-powerset *extra_args:
    {{cargo_hack}} check --target-dir target/check-powerset-target {{package_flag}} --no-dev-deps --lib --bins {{feature_powerset_flag}} {{message_format_flag}} {{target_tuple_flag}} {{release_flag}} {{extra_args}}

# Run `cargo build`
build *extra_args:
    {{cargo}} build {{package_flag}} {{all_targets_flag}} {{all_features_flag}} {{message_format_flag}} {{target_tuple_flag}} {{release_flag}} {{extra_args}}

# Run `cargo test`
@test *extra_args: (_run-tests cargo "test" package_flag all_features_flag message_format_flag target_tuple_flag release_flag extra_args)

# Run `cargo update` to update dependencies in Cargo.lock
update *extra_args:
    {{cargo}} update {{extra_args}}

# Run `cargo tarpaulin` to produce code coverage
@tarpaulin *extra_args: (_run-tests cargo_tarpaulin "--target-dir target/tarpaulin-target" extra_args)
    {{ if env('CI', '') == '' { "just _open-tarpaulin" } else { "" } }}

[unix]
@_open-tarpaulin:
    open tarpaulin-report.html

[windows]
@_open-tarpaulin:
    ./tarpaulin-report.html

# Run `cargo llvm-cov` to produce code coverage
@llvm-cov *extra_args: (_run-tests cargo_llvm_cov "--codecov --output-path codecov.json" package_flag all_targets_flag all_features_flag target_tuple_flag extra_args)
    {{cargo_llvm_cov}} report --html {{ if env('CI', '') == '' { '--open' } else { '' } }}

# Generate documentation with rustdoc
doc: _doc

_doc $RUSTDOCFLAGS=("-D warnings " + rustdoc_extra_flags):
    {{cargo}} doc {{ if env('CI', '') != '' { '--no-deps' } else { '--open' } }} {{package_flag}} {{all_features_flag}} {{message_format_flag}}

# Check doc coverage with Nightly rustdoc
doc-coverage: _doc-coverage

_doc-coverage $RUSTDOCFLAGS="-Z unstable-options --show-coverage":
    {{cargo_nightly}} doc --no-deps {{package_flag}} {{all_features_flag}} {{message_format_flag}}

[private]
minimize:
    {{cargo_hack}} --remove-dev-deps {{package_flag}}
    {{cargo_nightly}} update -Z minimal-versions

# Run `cargo minimal-versions check`
check-minimal: prep _check-minimal-only && unprep

_check-minimal-only: (_rimraf "target/check-minimal-target")
    {{cargo}} minimal-versions check --target-dir target/check-minimal-target {{package_flag}} --lib --bins {{all_features_flag}} {{message_format_flag}}

# Run `cargo msrv` with `cargo minimal-versions check`
msrv: (prep "--manifest-backup-suffix .msrv-prep.outer.bak") && (unprep "--manifest-backup-suffix .msrv-prep.outer.bak")
    {{cargo}} msrv find -- {{just}} _check-minimal-only

# Run `cargo msrv` with `cargo check`
msrv-full *extra_args: (prep "--manifest-backup-suffix .msrv-prep.outer.bak --no-merge-pinned-dependencies") && (unprep "--manifest-backup-suffix .msrv-prep.outer.bak")
    {{cargo}} msrv find -- {{just}} _msrv-check {{extra_args}}

_msrv-check *extra_args: (_rimraf "target/msrv-target") (check "--target-dir target/msrv-target" extra_args)

# Perform `cargo publish` dry-run on a package
test-package *extra_args:
    {{cargo}} publish {{package_flag}} --dry-run {{extra_args}}

# Run `cargo msrv-prep`
prep *extra_args:
    {{cargo}} msrv-prep {{package_flag}} --backup-root-manifest {{force_prep_flag}} {{extra_args}}

# Run `cargo msrv-unprep`
unprep *extra_args:
    {{cargo}} msrv-unprep {{package_flag}} --backup-root-manifest {{extra_args}}

# ----- Utilities -----

# Removes a directory (like `rm -rf`)
@_rimraf target_dir:
    {{ if path_exists(target_dir) == "true" { "just _rimraf-it '" + target_dir + "'" } else { "" } }}

[unix]
@_rimraf-it target_dir:
    rm -rf '{{target_dir}}'

[windows]
@_rimraf-it target_dir:
    Remove-Item "{{target_dir}}" -Recurse

# Prints state of a docker container to stdout
@_check-container-state name:
    docker container ls --filter "name={{name}}" --format "{{{{.State}}"
