load("@rules_proto//proto:defs.bzl", "proto_library")

PlProtoInfo = provider(
    doc = "Extra protobuf informations.",
    fields = {
        "rust_exposed_types": "Types to be exposed to Rust in a different way.",
    },
)

def _pl_proto_library_proxy_impl(ctx):
    base_lib = ctx.attr.base_lib

    rust_exposed_types = dict(ctx.attr.rust_exposed_types)
    for proto in ctx.attr.deps:
        if PlProtoInfo in proto:
            rust_exposed_types.update(proto[PlProtoInfo].rust_exposed_types)

    return [
        base_lib[DefaultInfo],
        base_lib[ProtoInfo],
        PlProtoInfo(
            rust_exposed_types = rust_exposed_types,
        ),
    ]

_pl_proto_library_proxy = rule(
    _pl_proto_library_proxy_impl,
    attrs = {
        "base_lib": attr.label(
            providers = [ProtoInfo],
            doc = "Base library of this proxy.",
        ),
        "deps": attr.label_list(
            providers = [ProtoInfo],
            doc = "Dependencies of the base library.",
        ),
        "rust_exposed_types": attr.string_dict(
            doc = "Types to be exposed to Rust in a different way.",
        ),
    },
)

def pl_proto_library(name, rust_exposed_types = {}, **kwargs):
    visibility = kwargs.pop("visibility", [])
    proto_library(
        name = name + "-internal",
        **kwargs
    )

    _pl_proto_library_proxy(
        name = name,
        base_lib = name + "-internal",
        deps = kwargs.get("deps", []),
        rust_exposed_types = rust_exposed_types,
        tags = kwargs.get("tags", []),
        visibility = visibility,
    )
