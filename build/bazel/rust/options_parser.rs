use prost_build::Config;
use prost_reflect::{DescriptorPool, FieldDescriptor, MessageDescriptor};

pub struct OptionsParser<'c> {
    config: &'c mut Config,
    pub descriptor_pool: DescriptorPool,
}

impl<'c> OptionsParser<'c> {
    pub fn new(config: &'c mut Config) -> Self {
        OptionsParser {
            config,
            descriptor_pool: DescriptorPool::new(),
        }
    }

    pub fn add_file_descriptor_set(&mut self, content: &[u8]) {
        self.descriptor_pool
            .decode_file_descriptor_set(content)
            .unwrap();
    }

    pub fn process_files(&mut self) {
        let messages = self.descriptor_pool.all_messages().collect::<Vec<_>>();
        let mut fields = vec![];

        for msg in messages {
            self.process_msg(&msg);

            fields.extend(msg.fields());

            for field in fields.drain(..) {
                self.process_field(&field);
            }
        }
    }

    fn process_msg(&mut self, msg: &MessageDescriptor) {
        for (ext, value) in msg.options().extensions() {
            match ext.full_name() {
                "google.api.resource" => {
                    value
                        .as_message()
                        .expect("invalid value for option (google.api.resource)");
                }
                "rust_message_options" => {
                    let opt_value = value
                        .as_message()
                        .expect("invalid value for option (pl.api.rust_message_options)");

                    if let Some(ext_ty) = opt_value.get_field_by_name("extern_type") {
                        if let Some(ext_ty) = ext_ty.as_str() {
                            self.config.extern_path(msg.name(), ext_ty);
                        }
                    }

                    if let Some(attribute) = opt_value.get_field_by_name("attribute") {
                        if let Some(attributes) = attribute.as_list() {
                            for attr in attributes.iter().filter_map(|a| a.as_str()) {
                                self.config.message_attribute(msg.name(), attr);
                            }
                        }
                    }
                }
                _ => (),
            }
        }
    }

    fn process_field(&mut self, field: &FieldDescriptor) {
        let field_name = field.full_name().to_string();

        for (ext, value) in field.options().extensions() {
            match ext.full_name() {
                // For now, do nothing with this option.
                "google.api.field_behavior" => {}
                // We can't change field type :/
                "google.api.resource_reference" => {
                    // We don't care about the type, parse to validate value.
                    value
                        .as_message()
                        .expect("invalid value for option (google.api.resource_reference)");
                }
                "rust_field_options" => {
                    let msg = value
                        .as_message()
                        .expect("invalid value for option (pl.api.rust_field_options)");

                    if let Some(attribute) = msg.get_field_by_name("attribute") {
                        if let Some(attributes) = attribute.as_list() {
                            for attr in attributes.iter().filter_map(|a| a.as_str()) {
                                self.config.field_attribute(&field_name, attr);
                            }
                        }
                    }

                    if let Some(boxed) = msg.get_field_by_name("boxed") {
                        if boxed.as_bool() == Some(true) {
                            self.config.boxed(&field_name);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
