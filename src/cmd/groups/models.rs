use dco3::groups::Group;
use tabled::Tabled;

#[derive(Tabled)]
pub struct GroupInfo {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub cnt_users: u64,
    #[tabled(display_with = "crate::cmd::users::display_option")]
    pub updated_at: Option<String>,
}

impl From<Group> for GroupInfo {
    fn from(group: Group) -> Self {
        Self {
            id: group.id.to_string(),
            name: group.name,
            cnt_users: group.cnt_users.unwrap_or(0),
            created_at: group.created_at.to_string(),
            updated_at: group.updated_at.map(|dt| dt.to_rfc3339()),
        }
    }
}