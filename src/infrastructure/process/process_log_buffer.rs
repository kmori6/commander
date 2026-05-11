#[derive(Debug, Clone)]
pub struct ProcessLogBuffer {
    max_bytes: usize,
    stdout: String,
    stderr: String,
    truncated: bool,
}

impl ProcessLogBuffer {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            stdout: String::new(),
            stderr: String::new(),
            truncated: false,
        }
    }

    pub fn append_stdout(&mut self, text: &str) {
        append_with_limit(&mut self.stdout, text, self.max_bytes, &mut self.truncated);
    }

    pub fn append_stderr(&mut self, text: &str) {
        append_with_limit(&mut self.stderr, text, self.max_bytes, &mut self.truncated);
    }

    pub fn stdout(&self) -> &str {
        &self.stdout
    }

    pub fn stderr(&self) -> &str {
        &self.stderr
    }

    pub fn truncated(&self) -> bool {
        self.truncated
    }
}

fn append_with_limit(buffer: &mut String, text: &str, max_bytes: usize, truncated: &mut bool) {
    buffer.push_str(text);

    if buffer.len() <= max_bytes {
        return;
    }

    *truncated = true;

    let overflow = buffer.len() - max_bytes;
    let mut start = overflow;

    while !buffer.is_char_boundary(start) {
        start += 1;
    }

    buffer.drain(..start);
}
