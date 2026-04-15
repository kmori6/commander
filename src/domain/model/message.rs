use crate::domain::model::role::Role;

#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
}
