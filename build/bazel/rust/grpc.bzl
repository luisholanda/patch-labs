load("@rules_rust//rust:defs.bzl", "rust_library")
load("//build/bazel/proto:library.bzl", "PlProtoInfo")

def pl_rust_grpc_library(
        name,
        protos,
        rust_deps = [],
        field_attributes = {},
        type_attributes = {}):
    """Compile a list of `proto_library` into a Rust library.

    Args:
        name: The name of the library target.
        protos: The list of `proto_library` to compile.
        rust_deps: Rust dependencies to add to the Rust library.
        field_attributes: Equivalent to `prost-build`'s `field_attributes`.
        type_attributes: Equivalent to `prost-build`'s `type_attributes`.
    """
    _gen_rust_grpc(
        name = name + "_grpc",
        protos = protos,
        field_attributes = field_attributes,
        type_attributes = type_attributes,
    )

    rust_library(
        name = name,
        srcs = [name + "_grpc"],
        deps = rust_deps + [
            "@crates//:prost",
            "@crates//:prost-types",
            "@crates//:tonic",
        ],
    )

def _dict_map_each(k):
    return "{}={}".format(k[0], k[1])

def _gen_rust_grpc_impl(ctx):
    protos = ctx.attr.protos
    field_attributes = ctx.attr.field_attributes
    type_attributes = ctx.attr.type_attributes

    output_file = ctx.actions.declare_file(ctx.label.name + ".rs")

    extern_paths = {}
    file_descriptor_sets = []
    for proto in protos:
        proto_info = proto[ProtoInfo]

        file_descriptor_sets.append(proto_info.direct_descriptor_set)

        if PlProtoInfo in proto:
            extern_paths.update(proto[PlProtoInfo].rust_exposed_types)

    args = ctx.actions.args()
    args.add(output_file)
    args.add_all("--file-descriptor-sets", file_descriptor_sets)

    for arg_name, d in {
        "--field-attributes": field_attributes,
        "--type-attributes": type_attributes,
        "--extern-paths": extern_paths,
    }.items():
        if len(d) != 0:
            args.add_all(arg_name, d.items(), map_each = _dict_map_each)

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
        "field_attributes": attr.string_dict(
            doc = "Attributes to add to specific fields.",
        ),
        "type_attributes": attr.string_dict(
            doc = "Attributes to add to specific types.",
        ),
        "_grpc_gen": attr.label(
            default = "//build/bazel/rust:grpc-gen",
            executable = True,
            cfg = "exec",
        ),
    },
)
