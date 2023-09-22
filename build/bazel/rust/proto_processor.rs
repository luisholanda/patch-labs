use std::{
    collections::{HashMap, HashSet},
    io::{BufWriter, Write},
    path::PathBuf,
};

use heck::ToSnakeCase;
use pl_gen_options_parser::OptionsParser;
use prost_build::{Config, Module};
use prost_reflect::prost_types::FileDescriptorProto;

pub struct ProcessorOptions {
    pub direct_file_descriptor_sets: Vec<PathBuf>,
    pub transitive_file_descriptor_sets: Vec<PathBuf>,
    pub output_path: PathBuf,
    pub extern_crates: Vec<String>,
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

        let mut pkgs = HashSet::new();
        for file in opts_parser.descriptor_pool.files().take(direct_offset) {
            let mut pkg_name = file
                .file_descriptor_proto()
                .package()
                .split('.')
                .map(|s| s.to_snake_case())
                .collect::<Vec<_>>()
                .join(".");
            pkg_name.insert(0, '.');
            pkgs.insert(pkg_name);
        }

        for file in &self.descriptor_sets {
            let mut pkg_name = file
                .package()
                .split('.')
                .map(|s| s.to_snake_case())
                .collect::<Vec<_>>()
                .join(".");
            pkg_name.insert(0, '.');
            pkgs.remove(&pkg_name);
        }

        for pkg in pkgs {
            if pkg == ".google.protobuf" {
                continue;
            }
            self.config.extern_path(pkg, ".imports");
        }
    }

    pub fn generate_protos(mut self) {
        let output = std::fs::File::create(&self.options.output_path).unwrap();
        let mut output = BufWriter::new(output);

        writeln!(output, "pub(crate) mod imports {{").unwrap();
        for ext in &self.options.extern_crates {
            writeln!(output, "pub(crate) use ::{ext}::*;").unwrap();
        }
        writeln!(output, "}}").unwrap();
        writeln!(output, "pub(crate) use self::imports::*;").unwrap();

        let mut module_tree = ModuleTree::default();
        for file_descriptor in self.descriptor_sets {
            let module = Module::from_protobuf_package_name(file_descriptor.package());

            let buffers = self
                .config
                .generate(vec![(module, file_descriptor)])
                .unwrap();

            for (module, buf) in buffers {
                module_tree.insert_module_buf(&module, buf);
            }
        }

        module_tree.print(&mut output).unwrap();

        output.flush().unwrap();
    }
}

#[derive(Default)]
struct ModuleTree {
    roots: HashMap<String, ModuleTreeNode>,
}

impl ModuleTree {
    pub fn insert_module_buf(&mut self, module: &Module, buf: String) {
        let mut module = module.parts().peekable();
        let mut tree = self;

        while let Some(part) = module.next() {
            let node = tree.roots.entry(part.to_string()).or_default();

            if module.peek().is_none() {
                node.bufs.push(buf);
                break;
            } else {
                tree = &mut node.children;
            }
        }
    }

    pub fn print(&self, output: &mut impl Write) -> std::io::Result<()> {
        for (name, node) in &self.roots {
            //writeln!(output, "pub mod {name} {{")?;

            for buf in &node.bufs {
                output.write_all(buf.as_bytes())?;
            }

            node.children.print(output)?;

            //writeln!(output, "}}")?;
        }

        Ok(())
    }
}

#[derive(Default)]
struct ModuleTreeNode {
    bufs: Vec<String>,
    children: ModuleTree,
}
