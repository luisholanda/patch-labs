load("@rules_rust//rust:defs.bzl", "rust_library")

def pl_rust_grpc_library(
        name,
        protos,
        deps = [],
        **kwargs):
    """Compile a list of `proto_library` into a Rust library.

    Args:
        name: The name of the library target.
        protos: The list of `proto_library` to compile.
        deps: Rust dependencies to add to the Rust library.
        **kwargs: extra attributes to the Rust library.
    """
    _gen_rust_grpc(
        name = name + "_grpc",
        protos = protos,
    )

    rust_library(
        name = name,
        srcs = [name + "_grpc"],
        deps = deps + [
            "//third-party/crates:prost",
            "//third-party/crates:prost-types",
            "//third-party/crates:tonic",
        ],
        **kwargs,
    )

def _gen_rust_grpc_impl(ctx):
    protos = ctx.attr.protos

    output_file = ctx.actions.declare_file(ctx.label.name + ".rs")

    file_descriptor_sets = []
    for proto in protos:
        file_descriptor_sets.append(proto[ProtoInfo].direct_descriptor_set)

    args = ctx.actions.args()
    args.add(output_file)
    args.add_all("--file-descriptor-sets", file_descriptor_sets)

    ctx.actions.run(
        outputs = [output_file],
        inputs = file_descriptor_sets,
        executable = ctx.executable._grpc_gen,
        arguments = [args],
        mnemonic = "RustProtoGen",
        progress_message = "Compiling protos of %{label} into %{output}",
    )

    return [
        DefaultInfo(
            files = depset(direct = [output_file]),
        ),
    ]

_gen_rust_grpc = rule(
    _gen_rust_grpc_impl,
    attrs = {
        "protos": attr.label_list(
            providers = [ProtoInfo],
            doc = "The proto libraries to compile.",
        ),
        "_grpc_gen": attr.label(
            default = "//build/bazel/rust:grpc-gen",
            executable = True,
            cfg = "exec",
        ),
    },
)
