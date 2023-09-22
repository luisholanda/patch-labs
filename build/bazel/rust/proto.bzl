load("//build/bazel/rust:library.bzl", "pl_rust_library")

def pl_rust_proto_library(
        name,
        protos,
        deps = [],
        **kwargs):
    """Compile a list of `proto_library` into a Rust library.

    Args:
        name: The name of the library target.
        protos: The list of `proto_library` to compile.
        deps: Rust dependencies to add to the Rust library.
        **kwargs: extra attributes to the rust library.
    """
    _gen_rust_proto(
        name = name + "_pb",
        protos = protos,
    )

    pl_rust_library(
        name = name,
        srcs = [name + "_pb"],
        deps = ["//third-party/crates:prost", "//third-party/crates:prost-types"] + deps,
        create_test_target = False,
        **kwargs
    )

def _gen_rust_proto_impl(ctx):
    protos = ctx.attr.protos

    output_file = ctx.actions.declare_file(ctx.label.name + ".rs")

    file_descriptor_sets = []
    direct_sets = []
    for proto in protos:
        file_descriptor_sets.append(proto[ProtoInfo].transitive_descriptor_sets)
        direct_sets.append(proto[ProtoInfo].direct_descriptor_set)

    args = ctx.actions.args()
    args.add(output_file)
    args.add_all(depset(transitive = file_descriptor_sets), before_each = "--transitive-file-descriptor-sets")
    args.add_all(direct_sets, before_each = "--direct-file-descriptor-sets")

    ctx.actions.run(
        outputs = [output_file],
        inputs = depset(direct = direct_sets, transitive = file_descriptor_sets),
        executable = ctx.executable._proto_gen,
        arguments = [args],
        mnemonic = "RustProtoGen",
        progress_message = "Compiling protos of %{label} into %{output}",
    )

    return [
        DefaultInfo(
            files = depset(direct = [output_file]),
        ),
    ]

_gen_rust_proto = rule(
    _gen_rust_proto_impl,
    attrs = {
        "protos": attr.label_list(
            providers = [ProtoInfo],
            doc = "The proto libraries to compile.",
        ),
        "_proto_gen": attr.label(
            default = "//build/bazel/rust:proto-gen",
            executable = True,
            cfg = "exec",
        ),
    },
)
