//! Clean-room compatibility policy derived from observable RuFoundation behavior.
//!
//! This is intentionally not the final WikiNEXT global/namespace/page ACL model.
//! The compatibility result is kept explicit so M1 cannot silently invent a
//! different precedence rule.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::id::{GroupId, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Read,
    Edit,
    Create,
    Delete,
    Restore,
    Rename,
    ManageFiles,
    ManageAuthors,
    Tag,
    Rate,
    Comment,
    Moderate,
    ManageAcl,
    Lock,
    Admin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolePolicy {
    pub role_id: GroupId,
    pub allows: BTreeSet<Action>,
    pub restrictions: BTreeSet<Action>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleOverride {
    pub role_id: GroupId,
    pub allows: BTreeSet<Action>,
    pub restrictions: BTreeSet<Action>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyInput {
    pub roles: Vec<RolePolicy>,
    /// Overrides already resolved for the requested RuFoundation category.
    pub category_overrides: Vec<RoleOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicySubject {
    pub user_id: Option<UserId>,
    pub is_active: bool,
    pub is_superuser: bool,
    /// Must include the virtual `everyone` role and, for authenticated users,
    /// the virtual `registered` role.
    pub role_ids: BTreeSet<GroupId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PagePolicyState {
    pub locked: bool,
    pub subject_is_author: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    pub allowed: bool,
    pub reason: DecisionReason,
}

impl Decision {
    fn allow(reason: DecisionReason) -> Self {
        Self {
            allowed: true,
            reason,
        }
    }

    fn deny(reason: DecisionReason) -> Self {
        Self {
            allowed: false,
            reason,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionReason {
    InactiveAccount,
    Superuser,
    RoleGrant { role_id: GroupId },
    AuthorGrant,
    PageLocked,
    NoGrant,
    InvalidPolicy { issue: PolicyIssue },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyIssue {
    DuplicateRole { role_id: GroupId },
    DuplicateOverride { role_id: GroupId },
    UnknownSubjectRole { role_id: GroupId },
    UnknownOverrideRole { role_id: GroupId },
}

/// Resolves the permission using RuFoundation's role-local restriction model.
///
/// Invalid or incomplete input always produces a denial with an explicit
/// [`DecisionReason::InvalidPolicy`].
pub fn resolve(
    input: &PolicyInput,
    subject: &PolicySubject,
    page: PagePolicyState,
    action: Action,
) -> Decision {
    if !subject.is_active {
        return Decision::deny(DecisionReason::InactiveAccount);
    }

    if subject.is_superuser {
        return Decision::allow(DecisionReason::Superuser);
    }

    let roles = match unique_roles(&input.roles) {
        Ok(roles) => roles,
        Err(issue) => return invalid(issue),
    };
    let overrides = match unique_overrides(&input.category_overrides, &roles) {
        Ok(overrides) => overrides,
        Err(issue) => return invalid(issue),
    };

    for role_id in &subject.role_ids {
        if !roles.contains_key(role_id) {
            return invalid(PolicyIssue::UnknownSubjectRole { role_id: *role_id });
        }
    }

    let grants = effective_grants(subject, &roles, &overrides);
    if page.locked && is_removed_by_page_lock(action) && !grants.contains_key(&Action::Lock) {
        return Decision::deny(DecisionReason::PageLocked);
    }

    if action == Action::ManageAuthors && page.subject_is_author && !page.locked {
        return Decision::allow(DecisionReason::AuthorGrant);
    }

    match grants.get(&action) {
        Some(role_id) => Decision::allow(DecisionReason::RoleGrant { role_id: *role_id }),
        None => Decision::deny(DecisionReason::NoGrant),
    }
}

fn unique_roles(
    role_policies: &[RolePolicy],
) -> Result<BTreeMap<GroupId, &RolePolicy>, PolicyIssue> {
    let mut roles = BTreeMap::new();
    for role in role_policies {
        if roles.insert(role.role_id, role).is_some() {
            return Err(PolicyIssue::DuplicateRole {
                role_id: role.role_id,
            });
        }
    }
    Ok(roles)
}

fn unique_overrides<'a>(
    role_overrides: &'a [RoleOverride],
    roles: &BTreeMap<GroupId, &RolePolicy>,
) -> Result<BTreeMap<GroupId, &'a RoleOverride>, PolicyIssue> {
    let mut overrides = BTreeMap::new();
    for role_override in role_overrides {
        if !roles.contains_key(&role_override.role_id) {
            return Err(PolicyIssue::UnknownOverrideRole {
                role_id: role_override.role_id,
            });
        }
        if overrides
            .insert(role_override.role_id, role_override)
            .is_some()
        {
            return Err(PolicyIssue::DuplicateOverride {
                role_id: role_override.role_id,
            });
        }
    }
    Ok(overrides)
}

fn effective_grants(
    subject: &PolicySubject,
    roles: &BTreeMap<GroupId, &RolePolicy>,
    overrides: &BTreeMap<GroupId, &RoleOverride>,
) -> BTreeMap<Action, GroupId> {
    let mut grants = BTreeMap::new();

    for role_id in &subject.role_ids {
        let Some(role) = roles.get(role_id) else {
            continue;
        };
        let mut role_grants = role.allows.clone();
        role_grants.retain(|action| !role.restrictions.contains(action));

        if let Some(role_override) = overrides.get(role_id) {
            role_grants.extend(&role_override.allows);
            role_grants.retain(|action| !role_override.restrictions.contains(action));
        }

        for action in role_grants {
            grants.entry(action).or_insert(*role_id);
        }
    }

    grants
}

const fn is_removed_by_page_lock(action: Action) -> bool {
    matches!(
        action,
        Action::Edit
            | Action::Delete
            | Action::Rename
            | Action::ManageFiles
            | Action::ManageAuthors
            | Action::Tag
    )
}

fn invalid(issue: PolicyIssue) -> Decision {
    Decision::deny(DecisionReason::InvalidPolicy { issue })
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn group() -> GroupId {
        GroupId::new(Uuid::new_v4())
    }

    fn user() -> UserId {
        UserId::new(Uuid::new_v4())
    }

    fn set(actions: impl IntoIterator<Item = Action>) -> BTreeSet<Action> {
        actions.into_iter().collect()
    }

    fn role(
        role_id: GroupId,
        allows: impl IntoIterator<Item = Action>,
        restrictions: impl IntoIterator<Item = Action>,
    ) -> RolePolicy {
        RolePolicy {
            role_id,
            allows: set(allows),
            restrictions: set(restrictions),
        }
    }

    fn subject(role_ids: impl IntoIterator<Item = GroupId>) -> PolicySubject {
        PolicySubject {
            user_id: Some(user()),
            is_active: true,
            is_superuser: false,
            role_ids: role_ids.into_iter().collect(),
        }
    }

    fn page() -> PagePolicyState {
        PagePolicyState {
            locked: false,
            subject_is_author: false,
        }
    }

    #[test]
    fn restriction_removes_allow_inside_same_role() {
        let everyone = group();
        let input = PolicyInput {
            roles: vec![role(everyone, [Action::Read], [Action::Read])],
            category_overrides: Vec::new(),
        };

        let decision = resolve(&input, &subject([everyone]), page(), Action::Read);
        assert_eq!(decision, Decision::deny(DecisionReason::NoGrant));
    }

    #[test]
    fn grant_in_another_role_survives_a_restriction() {
        let restricted = group();
        let granting = group();
        let input = PolicyInput {
            roles: vec![
                role(restricted, [Action::Edit], [Action::Edit]),
                role(granting, [Action::Edit], []),
            ],
            category_overrides: Vec::new(),
        };

        let decision = resolve(
            &input,
            &subject([restricted, granting]),
            page(),
            Action::Edit,
        );
        assert!(decision.allowed);
        assert_eq!(
            decision.reason,
            DecisionReason::RoleGrant { role_id: granting }
        );
    }

    #[test]
    fn category_override_is_applied_per_role_before_union() {
        let restricted = group();
        let granting = group();
        let input = PolicyInput {
            roles: vec![role(restricted, [Action::Read], []), role(granting, [], [])],
            category_overrides: vec![
                RoleOverride {
                    role_id: restricted,
                    allows: BTreeSet::new(),
                    restrictions: set([Action::Read]),
                },
                RoleOverride {
                    role_id: granting,
                    allows: set([Action::Read]),
                    restrictions: BTreeSet::new(),
                },
            ],
        };

        assert!(
            resolve(
                &input,
                &subject([restricted, granting]),
                page(),
                Action::Read
            )
            .allowed
        );
    }

    #[test]
    fn lock_removes_mutation_unless_subject_can_manage_lock() {
        let editor = group();
        let locker = group();
        let input = PolicyInput {
            roles: vec![
                role(editor, [Action::Edit], []),
                role(locker, [Action::Lock], []),
            ],
            category_overrides: Vec::new(),
        };
        let locked = PagePolicyState {
            locked: true,
            subject_is_author: false,
        };

        let denied = resolve(&input, &subject([editor]), locked, Action::Edit);
        assert_eq!(denied.reason, DecisionReason::PageLocked);

        let allowed = resolve(&input, &subject([editor, locker]), locked, Action::Edit);
        assert!(allowed.allowed);
    }

    #[test]
    fn unlocked_author_can_manage_authors() {
        let everyone = group();
        let input = PolicyInput {
            roles: vec![role(everyone, [], [])],
            category_overrides: Vec::new(),
        };
        let author_page = PagePolicyState {
            locked: false,
            subject_is_author: true,
        };

        let decision = resolve(
            &input,
            &subject([everyone]),
            author_page,
            Action::ManageAuthors,
        );
        assert_eq!(decision.reason, DecisionReason::AuthorGrant);
    }

    #[test]
    fn inactive_account_is_denied_and_active_superuser_bypasses_roles() {
        let input = PolicyInput {
            roles: Vec::new(),
            category_overrides: Vec::new(),
        };
        let inactive = PolicySubject {
            user_id: Some(user()),
            is_active: false,
            is_superuser: true,
            role_ids: BTreeSet::new(),
        };
        assert_eq!(
            resolve(&input, &inactive, page(), Action::Admin).reason,
            DecisionReason::InactiveAccount
        );

        let active = PolicySubject {
            is_active: true,
            ..inactive
        };
        assert_eq!(
            resolve(&input, &active, page(), Action::Admin).reason,
            DecisionReason::Superuser
        );
    }

    #[test]
    fn damaged_policy_fails_closed_with_reason() {
        let missing = group();
        let input = PolicyInput {
            roles: Vec::new(),
            category_overrides: Vec::new(),
        };

        let decision = resolve(&input, &subject([missing]), page(), Action::Read);
        assert_eq!(
            decision,
            Decision::deny(DecisionReason::InvalidPolicy {
                issue: PolicyIssue::UnknownSubjectRole { role_id: missing }
            })
        );
    }

    #[test]
    fn role_input_order_does_not_change_decision() {
        let first = group();
        let second = group();
        let mut roles = vec![
            role(first, [Action::Read], []),
            role(second, [Action::Read], []),
        ];
        let policy_subject = subject([first, second]);
        let before = resolve(
            &PolicyInput {
                roles: roles.clone(),
                category_overrides: Vec::new(),
            },
            &policy_subject,
            page(),
            Action::Read,
        );
        roles.reverse();
        let after = resolve(
            &PolicyInput {
                roles,
                category_overrides: Vec::new(),
            },
            &policy_subject,
            page(),
            Action::Read,
        );

        assert_eq!(before, after);
    }
}
