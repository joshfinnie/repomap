use std::collections::HashMap;

pub enum Mode {
    Fast,
    Slow,
}

pub trait Render {
    fn render(&self) -> String;
}

impl Render for Mode {
    fn render(&self) -> String {
        String::new()
    }
}

pub const MAX: u32 = 10;

fn hidden(cache: &HashMap<u32, String>) -> usize {
    cache.len()
}
