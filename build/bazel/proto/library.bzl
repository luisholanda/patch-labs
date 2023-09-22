load("@rules_proto//proto:defs.bzl", "proto_library")
load("@rules_buf//buf:defs.bzl", "buf_lint_test")

def pl_proto_library(name, disable_lint = False, **kwargs):
    proto_library(
        name = name,
        strip_import_prefix = "/protos",
        **kwargs
    )

    if not disable_lint:
        buf_lint_test(
            name = name + "-lint",
            targets = [name],
            config = "//protos:buf.yaml",
        )
