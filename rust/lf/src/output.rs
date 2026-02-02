pub fn use_color() -> bool {
    std::env::var("NO_COLOR").is_err() && atty::is(atty::Stream::Stdout)
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct Colors {
    pub cyan: &'static str,
    pub bold: &'static str,
    pub dim: &'static str,
    pub yellow: &'static str,
    pub green: &'static str,
    pub reset: &'static str,
}

impl Colors {
    pub fn new() -> Self {
        if use_color() {
            Self {
                cyan: "\x1b[36m",
                bold: "\x1b[1m",
                dim: "\x1b[90m",
                yellow: "\x1b[33m",
                green: "\x1b[32m",
                reset: "\x1b[0m",
            }
        } else {
            Self {
                cyan: "",
                bold: "",
                dim: "",
                yellow: "",
                green: "",
                reset: "",
            }
        }
    }
}

impl Default for Colors {
    fn default() -> Self {
        Self::new()
    }
}
