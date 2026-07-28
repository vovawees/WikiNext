use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type UserId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    View,
    Create,
    Edit,
    Delete,
    ManagePages,
    ManagePermissions,
    Admin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scope {
    Site,
    Namespace(String),
    Page { namespace: String, page_id: Uuid },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Principal {
    User(UserId),
    Group(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankRule {
    pub action: Action,
    pub min_rank: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AclEntry {
    pub scope: Scope,
    pub principal: Principal,
    pub action: Action,
    pub effect: Effect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicySubject {
    pub user_id: Option<UserId>,
    pub rank: u32,
    pub groups: Vec<String>,
}

impl PolicySubject {
    pub fn guest() -> Self {
        Self {
            user_id: None,
            rank: 0,
            groups: Vec::new(),
        }
    }

    pub fn user(user_id: UserId, rank: u32, groups: Vec<String>) -> Self {
        Self {
            user_id: Some(user_id),
            rank,
            groups,
        }
    }
}

pub fn is_allowed(
    subject: &PolicySubject,
    rank_rules: &[RankRule],
    acl: &[AclEntry],
    scope: &Scope,
    action: Action,
) -> bool {
    let mut allow = false;

    for entry in acl {
        if entry.action != action {
            continue;
        }

        if !scope_matches(scope, &entry.scope) {
            continue;
        }

        if !principal_matches(subject, &entry.principal) {
            continue;
        }

        match entry.effect {
            Effect::Deny => return false,
            Effect::Allow => allow = true,
        }
    }

    allow || rank_allowed(rank_rules, subject.rank, action)
}

fn rank_allowed(rank_rules: &[RankRule], rank: u32, action: Action) -> bool {
    rank_rules
        .iter()
        .any(|rule| rule.action == action && rank >= rule.min_rank)
}

fn scope_matches(requested: &Scope, entry: &Scope) -> bool {
    match (requested, entry) {
        (Scope::Site, Scope::Site) => true,
        (Scope::Namespace(requested_name), Scope::Namespace(entry_name)) => {
            requested_name == entry_name
        }
        (Scope::Namespace(_), Scope::Site) => true,
        (Scope::Page { namespace, .. }, Scope::Namespace(entry_name)) => namespace == entry_name,
        (Scope::Page { .. }, Scope::Site) => true,
        (
            Scope::Page {
                page_id: requested_page,
                ..
            },
            Scope::Page {
                page_id: entry_page,
                ..
            },
        ) => requested_page == entry_page,
        _ => false,
    }
}

fn principal_matches(subject: &PolicySubject, principal: &Principal) -> bool {
    match principal {
        Principal::User(user_id) => subject.user_id == Some(*user_id),
        Principal::Group(group) => subject.groups.iter().any(|name| name == group),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_rules() -> Vec<RankRule> {
        vec![
            RankRule {
                action: Action::View,
                min_rank: 0,
            },
            RankRule {
                action: Action::Create,
                min_rank: 1,
            },
            RankRule {
                action: Action::Edit,
                min_rank: 1,
            },
            RankRule {
                action: Action::Delete,
                min_rank: 5,
            },
            RankRule {
                action: Action::Admin,
                min_rank: 10,
            },
        ]
    }

    #[test]
    fn guest_can_view_by_rank() {
        let subject = PolicySubject::guest();
        let rules = default_rules();

        assert!(is_allowed(
            &subject,
            &rules,
            &[],
            &Scope::Site,
            Action::View
        ));
    }

    #[test]
    fn guest_cannot_create_by_rank() {
        let subject = PolicySubject::guest();
        let rules = default_rules();

        assert!(!is_allowed(
            &subject,
            &rules,
            &[],
            &Scope::Site,
            Action::Create
        ));
    }

    #[test]
    fn member_can_create_by_rank() {
        let subject = PolicySubject::user(Uuid::new_v4(), 1, Vec::new());
        let rules = default_rules();

        assert!(is_allowed(
            &subject,
            &rules,
            &[],
            &Scope::Site,
            Action::Create
        ));
    }

    #[test]
    fn deny_entry_overrides_rank() {
        let user_id = Uuid::new_v4();
        let subject = PolicySubject::user(user_id, 10, Vec::new());
        let rules = default_rules();
        let acl = vec![AclEntry {
            scope: Scope::Site,
            principal: Principal::User(user_id),
            action: Action::Delete,
            effect: Effect::Deny,
        }];

        assert!(!is_allowed(
            &subject,
            &rules,
            &acl,
            &Scope::Site,
            Action::Delete
        ));
    }

    #[test]
    fn group_allow_grants_permission() {
        let subject = PolicySubject::user(Uuid::new_v4(), 0, vec!["translator".to_owned()]);
        let rules = default_rules();
        let acl = vec![AclEntry {
            scope: Scope::Namespace("ru".to_owned()),
            principal: Principal::Group("translator".to_owned()),
            action: Action::Edit,
            effect: Effect::Allow,
        }];

        let scope = Scope::Page {
            namespace: "ru".to_owned(),
            page_id: Uuid::new_v4(),
        };

        assert!(is_allowed(&subject, &rules, &acl, &scope, Action::Edit));
    }

    #[test]
    fn namespace_deny_affects_pages() {
        let user_id = Uuid::new_v4();
        let subject = PolicySubject::user(user_id, 10, Vec::new());
        let rules = default_rules();
        let acl = vec![AclEntry {
            scope: Scope::Namespace("sandbox".to_owned()),
            principal: Principal::User(user_id),
            action: Action::Delete,
            effect: Effect::Deny,
        }];

        let scope = Scope::Page {
            namespace: "sandbox".to_owned(),
            page_id: Uuid::new_v4(),
        };

        assert!(!is_allowed(&subject, &rules, &acl, &scope, Action::Delete));
    }

    #[test]
    fn deny_wins_over_allow() {
        let user_id = Uuid::new_v4();
        let subject = PolicySubject::user(user_id, 0, vec!["staff".to_owned()]);
        let rules = default_rules();
        let acl = vec![
            AclEntry {
                scope: Scope::Site,
                principal: Principal::Group("staff".to_owned()),
                action: Action::Admin,
                effect: Effect::Allow,
            },
            AclEntry {
                scope: Scope::Site,
                principal: Principal::User(user_id),
                action: Action::Admin,
                effect: Effect::Deny,
            },
        ];

        assert!(!is_allowed(
            &subject,
            &rules,
            &acl,
            &Scope::Site,
            Action::Admin
        ));
    }
}
