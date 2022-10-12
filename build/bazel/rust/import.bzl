load("//build/bazel/rust:library.bzl", _crate_name = "pl_crate_name")

def crate_path(label, typ_path = None):
    [pkg_name, label] = label.split(":")
    label = _crate_name(label, pkg_name)

    if typ_path:
        return "::{}::{}".format(label, typ_path)

    return "::" + label
