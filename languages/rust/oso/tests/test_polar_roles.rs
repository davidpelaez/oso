use oso::{PolarClass, PolarValue, Query, ResultSet, ToPolar};
mod common;
use common::OsoTest;
use oso::errors::polar::{ErrorKind, PolarError, RolesValidationError};
use oso::errors::OsoError;

#[derive(Clone, PolarClass, Eq)]
struct Org {
    #[polar(attribute)]
    pub name: String,
}

impl PartialEq for Org {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

#[derive(Clone, PolarClass, Eq)]
struct Repo {
    #[polar(attribute)]
    pub name: String,
    #[polar(attribute)]
    pub org: Org,
}

impl PartialEq for Repo {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.org == other.org
    }
}

#[derive(Clone, PolarClass)]
struct Issue {
    #[polar(attribute)]
    pub name: String,
    #[polar(attribute)]
    pub repo: Repo,
}

#[derive(Clone, PolarClass)]
struct Role {
    #[polar(attribute)]
    pub name: String,
    #[polar(attribute)]
    pub resource: PolarValue,
}

#[derive(Clone, PolarClass)]
struct User {
    #[polar(attribute)]
    pub name: String,
    #[polar(attribute)]
    pub roles: Vec<Role>,
}

fn roles_test_oso() -> OsoTest {
    let mut test = OsoTest::new();
    test.oso
        .register_class(Org::get_polar_class_builder().with_equality_check().build())
        .unwrap();
    test.oso
        .register_class(
            Repo::get_polar_class_builder()
                .with_equality_check()
                .build(),
        )
        .unwrap();
    test.oso.register_class(Issue::get_polar_class()).unwrap();
    test.oso.register_class(User::get_polar_class()).unwrap();

    test
}

#[test]
fn test_polar_roles() {
    common::setup();
    let mut test = roles_test_oso();
    let pol = r#"
        resource(_type: Org, "org", actions, roles) if
            actions = [
                "invite",
                "create_repo"
            ] and
            roles = {
                member: {
                    permissions: ["create_repo"],
                    implies: ["repo:reader"]
                },
                owner: {
                    permissions: ["invite"],
                    implies: ["member", "repo:writer"]
                }
            };

        resource(_type: Repo, "repo", actions, roles) if
            actions = [
                "push",
                "pull"
            ] and
            roles = {
                writer: {
                    permissions: ["push", "issue:edit"],
                    implies: ["reader"]
                },
                reader: {
                    permissions: ["pull"]
                }
            };

        resource(_type: Issue, "issue", actions, {}) if
            actions = [
                "edit"
            ];

        parent_child(parent_org: Org, repo: Repo) if
            repo.org = parent_org;

        parent_child(parent_repo: Repo, issue: Issue) if
            issue.repo = parent_repo;

        actor_has_role_for_resource(actor, role_name, role_resource) if
            role in actor.roles and
            role matches {name: role_name, resource: role_resource};

        allow(actor, action, resource) if
            role_allows(actor, action, resource);

    "#;

    test.load_str(pol);
    test.enable_roles();

    let osohq = Org {
        name: "oso".to_string(),
    };
    let apple = Org {
        name: "apple".to_string(),
    };
    let oso = Repo {
        name: "oso".to_string(),
        org: osohq.clone(),
    };
    let ios = Repo {
        name: "ios".to_string(),
        org: apple,
    };
    let bug = Issue {
        name: "bug".to_string(),
        repo: oso.clone(),
    };
    let laggy = Issue {
        name: "laggy".to_string(),
        repo: ios,
    };

    let osohq_owner = Role {
        name: "owner".to_string(),
        resource: osohq.clone().to_polar(),
    };
    let osohq_member = Role {
        name: "member".to_string(),
        resource: osohq.clone().to_polar(),
    };

    let gwen = User {
        name: "gwen".to_string(),
        roles: vec![osohq_member.clone()],
    };
    let dave = User {
        name: "dave".to_string(),
        roles: vec![osohq_owner.clone()],
    };

    fn empty(i: oso::Result<Query>) -> bool {
        i.unwrap()
            .collect::<oso::Result<Vec<ResultSet>>>()
            .unwrap()
            .is_empty()
    }

    assert!(!empty(
        test.oso
            .query_rule("allow", (dave.clone(), "invite", osohq.clone()))
    ));
    assert!(!empty(test.oso.query_rule(
        "allow",
        (dave.clone(), "create_repo", osohq.clone())
    )));
    assert!(!empty(
        test.oso
            .query_rule("allow", (dave.clone(), "push", oso.clone()))
    ));
    assert!(!empty(
        test.oso
            .query_rule("allow", (dave.clone(), "pull", oso.clone()))
    ));
    assert!(!empty(
        test.oso
            .query_rule("allow", (dave.clone(), "edit", bug.clone()))
    ));

    assert!(empty(
        test.oso
            .query_rule("allow", (gwen.clone(), "invite", osohq.clone()))
    ));
    assert!(!empty(
        test.oso
            .query_rule("allow", (gwen.clone(), "create_repo", osohq))
    ));
    assert!(empty(
        test.oso
            .query_rule("allow", (gwen.clone(), "push", oso.clone()))
    ));
    assert!(!empty(
        test.oso.query_rule("allow", (gwen.clone(), "pull", oso))
    ));
    assert!(empty(
        test.oso
            .query_rule("allow", (gwen.clone(), "edit", bug.clone()))
    ));

    assert!(empty(
        test.oso.query_rule("allow", (dave, "edit", laggy.clone()))
    ));
    assert!(empty(test.oso.query_rule("allow", (gwen, "edit", laggy))));

    let gabe = User {
        name: "gabe".to_string(),
        roles: vec![],
    };
    assert!(empty(
        test.oso.query_rule("allow", (gabe, "edit", bug.clone()))
    ));
    let gabe = User {
        name: "gabe".to_string(),
        roles: vec![osohq_member],
    };
    assert!(empty(
        test.oso.query_rule("allow", (gabe, "edit", bug.clone()))
    ));
    let gabe = User {
        name: "gabe".to_string(),
        roles: vec![osohq_owner],
    };
    assert!(!empty(test.oso.query_rule("allow", (gabe, "edit", bug))));
}

fn check_empty_roles_error(err: OsoError) {
    let msg = String::from("Must define actions or roles.");
    assert!(matches!(
        err,
        OsoError::Polar(PolarError {
            kind: ErrorKind::RolesValidation(RolesValidationError(x)),
            context: None
        }) if x == msg
    ));
}
static VALID_POL: &str = r#"
    resource(_: Repo, "repo", ["read"], {});
    actor_has_role_for_resource(_,_,_);"#;

#[test]
fn test_roles_revalidation_str() {
    common::setup();

    let mut test = roles_test_oso();
    test.load_str(VALID_POL);
    test.enable_roles();

    let invalid_pol = r#"
        resource(_: Org, "org", [], {});
        actor_has_role_for_resource(_,_,_);"#;
    check_empty_roles_error(test.oso.load_str(invalid_pol).unwrap_err());
}

#[test]
fn test_roles_revalidation_file() {
    common::setup();
    let mut test = roles_test_oso();
    test.load_str(VALID_POL);
    test.enable_roles();

    check_empty_roles_error(test.load_file(file!(), "invalid_roles.polar").unwrap_err());
}
