use std::collections::HashMap;

const MAX_STRING_LENGTH: usize = 65535;
pub const MAX_STRING_LENGTH_BYTES: usize = 2;

pub struct Schema {
    pub i32_fields: Vec<String>,
    pub i32_field_to_index: HashMap<String, usize>,
    pub string_fields: Vec<String>,
    pub string_field_to_index: HashMap<String, usize>,
    pub string_max_lengths: Vec<usize>,
}

impl Schema {
    pub fn new() -> Self {
        Schema {
            i32_fields: vec![],
            i32_field_to_index: HashMap::new(),
            string_fields: vec![],
            string_max_lengths: vec![],
            string_field_to_index: HashMap::new(),
        }
    }

    pub fn add_i32_field(&mut self, field_name: &str) {
        assert!(!self.i32_field_to_index.contains_key(field_name));

        self.i32_field_to_index
            .insert(field_name.to_string(), self.i32_fields.len());
        self.i32_fields.push(field_name.to_string());
    }

    pub fn add_string_field(&mut self, field_name: &str, max_length: usize) {
        assert!(!self.string_field_to_index.contains_key(field_name));
        assert!(max_length <= MAX_STRING_LENGTH);

        self.string_field_to_index
            .insert(field_name.to_string(), self.string_fields.len());
        self.string_fields.push(field_name.to_string());
        self.string_max_lengths.push(max_length);
    }
}
