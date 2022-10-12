use std::io::{Write, BufWriter};

use prost::Message;
use prost_build::{Config, Module};
use prost_types::FileDescriptorSet;

#[derive(Debug, argh::FromArgs)]
#[argh(description = "compile a protobuf descriptor set into Rust code")]
struct Args {
    /// file name to where write the generated code.
    #[argh(positional)]
    output_file: String,
    /// descriptor sets to compile into Rust code.
    #[argh(option)]
    file_descriptor_sets: Vec<String>,
    /// attributes to add to specific fields.
    #[argh(option)]
    field_attributes: Vec<String>,
    /// attributes to add to specific types.
    #[argh(option)]
    type_attributes: Vec<String>,
    /// paths to replace with specific rust types.
    #[argh(option)]
    extern_paths: Vec<String>,
}

fn main() {
    let args: Args = argh::from_env();

    let generator = tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .service_generator();

    let mut config = Config::new();

    config
        .skip_protoc_run()
        .bytes(["."])
        .service_generator(generator)
        .include_file(&args.output_file);

    for f_attr in args.field_attributes {
        let (path, attr) = f_attr.split_once("=").unwrap();

        config.field_attribute(path, attr);
    }

    for t_attr in args.type_attributes {
        let (path, attr) = t_attr.split_once("=").unwrap();

        config.type_attribute(path, attr);
    }

    for ext_path in args.extern_paths {
        let (path, typ) = ext_path.split_once("=").unwrap();

        config.extern_path(path, typ);
    }

    let mut prost_requests = Vec::with_capacity(args.file_descriptor_sets.len());

    for file_descriptor_set_path in args.file_descriptor_sets {
        let content = std::fs::read(file_descriptor_set_path).unwrap();
        let descriptor_set = <FileDescriptorSet as Message>::decode(&*content).unwrap();

        for file in descriptor_set.file {
            let module = Module::from_protobuf_package_name(file.name.as_deref().unwrap());
            prost_requests.push((module, file));
        }
    }

    let buffers = config.generate(prost_requests).unwrap();

    let output = std::fs::File::create(&args.output_file).unwrap();
    let mut output = BufWriter::new(output);

    for (path, buf) in buffers {
        output.write_all(buf.as_bytes()).unwrap();
    }

    output.flush().unwrap();
}
