#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(u64);

pub trait Object {
    fn get_identifier(&self) -> ObjectId;
    fn destroy(&self) {}
}

impl ObjectId {
    pub fn random() -> Self {
        Self(rand::random())
    }
}

impl std::fmt::Display for ObjectId {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "#{:x}", self.0)
    }
}
