allow(actor, action, resource) if
    allowRole(role, action, resource) and
    actorInRole(actor, role, resource);

allow(_: {sub: sub}, action, resource) if
    allow(new test_oso::Actor(name: sub), action, resource);

allow("guest", action, resource) if
    allow(new test_oso::Actor(name: "guest"), action, resource);

allow(_: {username: name}, action, resource) if
    allow(new test_oso::Actor(name: name), action, resource);

allow(_actor: test_oso::Actor, "read", _resource: test_oso::Widget);
allow(actor: test_oso::Actor, "create", resource: test_oso::Company) if
    resource.role(actor) = "admin";

allow(actor: test_oso::Actor, "frob", resource: test_oso::Company) if
    resource in actor.companies();

allow(actor: test_oso::Actor, "list", test_oso::Company) if
    actor.name = "auditor";

allow(foo: FooDecorated, "read", bar: BarDecorated) if
    foo.foo = bar.bar;

# Admins can update all fields
allow_field(actor: test_oso::Actor, "update", resource: test_oso::Widget, field) if
    resource.company().role(actor) = "admin" and
    field in ["name", "purpose", "private_field"];

# Anybody who can update a field can also read it
allow_field(actor, "read", resource: test_oso::Widget, field) if
    allow_field(actor, "update", resource, field);

# Anybody can read public fields
allow_field(_: test_oso::Actor, "read", _: test_oso::Widget, field) if
    field in ["name", "purpose"];
