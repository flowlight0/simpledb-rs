use std::collections::HashMap;

use super::field::{Spec, Type};

const MAX_STRING_LENGTH: usize = 65535;
pub const MAX_STRING_LENGTH_BYTES: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
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

    pub fn add_field(&mut self, field_name: &str, field_spec: &Spec) {
        match field_spec {
            Spec::I32 => self.add_i32_field(field_name),
            Spec::VarChar(max_length) => self.add_string_field(field_name, *max_length),
        }
    }

    pub fn add_all(&mut self, schema: &Schema) {
        for field in &schema.get_fields() {
            let spec = schema.get_field_spec(field);
            self.add_field(field, &spec);
        }
    }

    pub fn has_field(&self, field_name: &str) -> bool {
        self.i32_field_to_index.contains_key(field_name)
            || self.string_field_to_index.contains_key(field_name)
    }

    pub fn get_field_type(&self, field_name: &str) -> Type {
        if self.i32_field_to_index.contains_key(field_name) {
            Type::I32
        } else {
            Type::String
        }
    }

    pub fn get_field_spec(&self, field_name: &str) -> Spec {
        if self.i32_field_to_index.contains_key(field_name) {
            Spec::I32
        } else {
            let index = self.string_field_to_index.get(field_name).unwrap();
            let length = self.string_max_lengths[*index];
            Spec::VarChar(length)
        }
    }

    pub fn get_fields(&self) -> Vec<String> {
        let mut fields = self.i32_fields.clone();
        fields.extend(self.string_fields.clone());
        fields
    }
}
