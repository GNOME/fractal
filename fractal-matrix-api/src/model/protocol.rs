#[derive(Debug, Clone)]
pub struct Protocol {
    pub id: String,
    pub desc: String,
}

impl Protocol {
    pub fn new(s: String) -> Self {
        Self {
            id: String::new(),
            desc: s.split('/').last().unwrap_or_default().to_string(),
        }
    }
}
