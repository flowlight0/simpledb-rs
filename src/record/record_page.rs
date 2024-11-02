use super::layout::Layout;

struct RecordPage {}

impl RecordPage {
    pub fn new(layout: Layout) -> Self {
        RecordPage {}
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_record_page() {
        let mut schema = Schema::new();
        schema.add_i32_field("A");

        let mut layout = Layout::new(schema);
    }
}
