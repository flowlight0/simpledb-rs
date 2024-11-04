use std::collections::HashMap;

use super::schema::{Schema, MAX_STRING_LENGTH_BYTES};

pub struct Layout {
    pub schema: Schema,
    field_name_to_offsets: HashMap<String, usize>,
    pub slot_size: usize,
}

impl Layout {
    pub fn new(schema: Schema) -> Self {
        let mut field_offsets = vec![];
        let mut field_name_to_offsets = HashMap::new();
        // 4 bytes is used for representing vacant or occupied slot.
        let mut offset = 4;
        for field_name in &schema.i32_fields {
            field_offsets.push(offset);
            field_name_to_offsets.insert(field_name.to_string(), offset);
            offset += 4;
        }

        for i in 0..schema.string_fields.len() {
            field_offsets.push(offset);
            field_name_to_offsets.insert(schema.string_fields[i].to_string(), offset);
            offset += MAX_STRING_LENGTH_BYTES + schema.string_max_lengths[i];
        }
        Layout {
            schema,
            field_name_to_offsets,
            slot_size: offset,
        }
    }

    pub fn get_type_code(&self, field_name: &str) -> i32 {
        // TODO: write cleaner code here.
        if self.schema.i32_field_to_index.contains_key(field_name) {
            0
        } else {
            1
        }
    }

    pub fn get_length(&self, field_name: &str) -> usize {
        // TODO: write cleaner code here.
        if self.schema.i32_field_to_index.contains_key(field_name) {
            4
        } else {
            *self
                .schema
                .string_max_lengths
                .get(*self.schema.string_field_to_index.get(field_name).unwrap())
                .unwrap()
        }
    }

    pub fn get_offset(&self, field_name: &str) -> usize {
        *self.field_name_to_offsets.get(field_name).unwrap()
    }
}
