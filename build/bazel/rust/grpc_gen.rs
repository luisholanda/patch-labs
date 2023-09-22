use std::path::PathBuf;

use pl_gen_proto_processor::*;
use prost_build::Config;

#[derive(Debug, argh::FromArgs)]
#[argh(description = "compile a protobuf descriptor set into Rust code")]
struct Args {
    /// file name to where write the generated code.
    #[argh(positional)]
    output_file: PathBuf,
    /// descriptor sets to compile into Rust code.
    #[argh(option)]
    direct_file_descriptor_sets: Vec<PathBuf>,
    /// descriptor sets of dependencies.
    #[argh(option)]
    transitive_file_descriptor_sets: Vec<PathBuf>,
    /// extern crate dependencies.
    #[argh(option)]
    extern_crates: Vec<String>,
}

fn main() {
    let args: Args = argh::from_env();

    let generator = tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .generate_default_stubs(true)
        .service_generator();

    let mut config = Config::new();

    config
        .skip_protoc_run()
        .bytes(["."])
        .service_generator(generator)
        .include_file(&args.output_file);

    let mut processor = ProtoProcessor::new(
        config,
        ProcessorOptions {
            direct_file_descriptor_sets: args.direct_file_descriptor_sets,
            transitive_file_descriptor_sets: args.transitive_file_descriptor_sets,
            output_path: args.output_file,
            extern_crates: args.extern_crates,
        },
    );

    processor.process_descriptors();
    processor.generate_protos();
}
