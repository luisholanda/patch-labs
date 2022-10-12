load("@rules_rust//rust:defs.bzl", "rust_library", "rust_test")

def pl_crate_name(name, pkg_name = None):
    pkg_name = pkg_name or native.package_name()

    if pkg_name.startswith("rust/") or pkg_name.startswith("//rust/"):
        pkg_name = pkg_name.split("rust/")[1]
    crate_name_prefix = pkg_name.replace("/", "_")

    if pkg_name.endswith(name):
        return crate_name_prefix
    else:
        return "{}_{}".format(crate_name_prefix, name)

def pl_rust_library(
        name,
        create_test_target = True,
        test_deps = [],
        test_proc_macro_deps = [],
        test_compile_data = [],
        test_data = [],
        **kwargs):
    rust_library(
        name = name,
        crate_name = pl_crate_name(name),
        **kwargs
    )

    if create_test_target:
        rust_test(
            name = name + "_test",
            crate = name,
            deps = test_deps,
            compile_data = test_compile_data,
            data = test_data,
            proc_macro_deps = test_proc_macro_deps,
        )
