use std::collections::HashMap;

use super::schema::{Schema, MAX_STRING_LENGTH_BYTES};

#[derive(Debug, Clone)]
pub struct Layout {
    pub schema: Schema,
    field_name_to_offsets: HashMap<String, usize>,
    field_name_to_bit_locations: HashMap<String, u8>,
    pub slot_size: usize,
}

const MAX_NUM_FIELDS: usize = 31;

impl Layout {
    pub fn new(schema: Schema) -> Self {
        assert!(
            schema.i32_fields.len() + schema.string_fields.len() <= MAX_NUM_FIELDS,
            "Layout supports at most {} fields",
            MAX_NUM_FIELDS
        );
        let mut field_name_to_offsets = HashMap::new();
        let mut field_name_to_bit_locations = HashMap::new();
        // 4 bytes is used for representing vacant or occupied slot and null bits.
        let mut offset = 4;
        let mut bit = 1u8;
        for field_name in &schema.i32_fields {
            field_name_to_offsets.insert(field_name.to_string(), offset);
            field_name_to_bit_locations.insert(field_name.to_string(), bit);
            offset += 4;
            bit += 1;
        }

        for i in 0..schema.string_fields.len() {
            let name = schema.string_fields[i].to_string();
            field_name_to_offsets.insert(name.clone(), offset);
            field_name_to_bit_locations.insert(name, bit);
            offset += MAX_STRING_LENGTH_BYTES + schema.string_max_lengths[i];
            bit += 1;
        }
        Layout {
            schema,
            field_name_to_offsets,
            field_name_to_bit_locations,
            slot_size: offset,
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

    pub fn null_bit_location(&self, field_name: &str) -> u8 {
        *self.field_name_to_bit_locations.get(field_name).unwrap()
    }

    pub fn has_field(&self, field_name: &str) -> bool {
        self.field_name_to_offsets.contains_key(field_name)
    }
}
