load("@rules_rust//crate_universe:defs.bzl", "crate", "crates_repository")

RUST_DEPS = {
    "argh": "0.1.8",
    "async-trait": "0.1.51",
    "mockall": "0.10.2",
    "prost": "0.11.0",
    "prost-build": "0.11.1",
    "prost-types": "0.11.1",
    "svix-ksuid": "0.6.0",
    "tokio": "1.12.0",
}

def external_crates_dependencies(name):
    crates_repository(
        name = name,
        cargo_lockfile = "//:Cargo.lock",
        lockfile = "//:Cargo.Bazel.json",
        packages = _build_external_packages(),
    )

def _build_external_packages():
    translated = {}

    for _crate, spec in RUST_DEPS.items():
        if type(spec) == "string":
            translated[_crate] = crate.spec(version = spec)
        else:
            translated[_crate] = crate.spec(**spec)

    return translated
