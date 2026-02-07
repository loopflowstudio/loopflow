pub trait Progress {
    fn status(&self, msg: &str);
    fn error(&self, msg: &str);
    fn confirm(&self, msg: &str) -> bool;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NullProgress;

impl Progress for NullProgress {
    fn status(&self, _msg: &str) {}

    fn error(&self, _msg: &str) {}

    fn confirm(&self, _msg: &str) -> bool {
        true
    }
}
