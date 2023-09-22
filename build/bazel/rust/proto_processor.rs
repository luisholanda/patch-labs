use std::{
    io::{BufWriter, Write},
    path::PathBuf,
};

use pl_gen_options_parser::OptionsParser;
use prost_build::{Config, Module};
use prost_reflect::prost_types::FileDescriptorProto;

pub struct ProcessorOptions {
    pub direct_file_descriptor_sets: Vec<PathBuf>,
    pub transitive_file_descriptor_sets: Vec<PathBuf>,
    pub output_path: PathBuf,
}

pub struct ProtoProcessor {
    options: ProcessorOptions,
    config: Config,
    descriptor_sets: Vec<FileDescriptorProto>,
}

impl ProtoProcessor {
    pub fn new(config: Config, opts: ProcessorOptions) -> Self {
        Self {
            options: opts,
            config,
            descriptor_sets: vec![],
        }
    }

    pub fn process_descriptors(&mut self) {
        let mut opts_parser = OptionsParser::new(&mut self.config);

        self.options
            .transitive_file_descriptor_sets
            .retain(|f| !self.options.direct_file_descriptor_sets.contains(f));

        for file_descriptor_set_path in &self.options.transitive_file_descriptor_sets {
            let content = std::fs::read(file_descriptor_set_path).unwrap();
            opts_parser.add_file_descriptor_set(&content);
        }

        let direct_offset = opts_parser.descriptor_pool.files().len();

        for file_descriptor_set_path in &self.options.direct_file_descriptor_sets {
            let content = std::fs::read(file_descriptor_set_path).unwrap();
            opts_parser.add_file_descriptor_set(&content);
        }

        opts_parser.process_files();

        for file in opts_parser.descriptor_pool.files().skip(direct_offset) {
            let file_proto = file.file_descriptor_proto().clone();
            self.descriptor_sets.push(file_proto);
        }
    }

    pub fn generate_protos(mut self) {
        let mut prost_requests = Vec::with_capacity(self.descriptor_sets.len());

        for file_descriptor in self.descriptor_sets {
            let module = Module::from_protobuf_package_name(file_descriptor.name());
            prost_requests.push((module, file_descriptor));
        }

        let buffers = self.config.generate(prost_requests).unwrap();

        let output = std::fs::File::create(&self.options.output_path).unwrap();
        let mut output = BufWriter::new(output);

        for buf in buffers.into_values() {
            output.write_all(buf.as_bytes()).unwrap();
        }

        output.flush().unwrap();
    }
}
