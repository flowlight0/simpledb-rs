pub fn best_root(mut num_available_buffers: usize, table_size: usize) -> usize {
    if num_available_buffers < 3 {
        1
    } else {
        // Reserve a couple of buffers
        num_available_buffers -= 2;

        let mut k = usize::MAX;
        let mut i = 1;

        // k should be sqrt(table_size) or less
        while k > num_available_buffers {
            i += 1;
            k = (table_size as f64).powf(1.0 / i as f64) as usize
        }
        k
    }
}

pub fn best_factor(mut num_available_buffers: usize, table_size: usize) -> usize {
    if num_available_buffers < 3 {
        1
    } else {
        // Reserve a couple of buffers
        num_available_buffers -= 2;

        // We allow it to skip while loop if table_size is small
        let mut k = table_size;
        let mut i = 1;

        // k should be sqrt(table_size) or less
        while k > num_available_buffers {
            i += 1;
            k = (table_size as f64).powf(1.0 / i as f64) as usize
        }
        k
    }
}
