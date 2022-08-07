def crate_path(label, typ_path = None):
    label = label.replace("//", "").replace("/", "_y_").replace(":", "_x_")

    if typ_path:
        return "::{}::{}".format(label, typ_path)

    return "::" + label
