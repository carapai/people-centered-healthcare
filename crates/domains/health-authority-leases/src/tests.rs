use super::*;

fn at(hours: i64) -> Timestamp {
    Timestamp::UNIX_EPOCH
        .checked_add(SignedDuration::from_hours(1_000 + hours))
        .expect("time")
}

fn stores(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|s| (*s).to_owned()).collect()
}

fn outreach(existing: &[Lease]) -> Result<Lease, LeaseRefused> {
    Lease::grant(
        Uuid::now_v7(),
        "desktop-1",
        stores(&["outreach-bag"]),
        "admin",
        at(0),
        SignedDuration::from_hours(8),
        existing,
    )
}

#[test]
fn a_granted_store_is_the_holders_alone_until_it_expires() {
    let lease = outreach(&[]).expect("granted");
    let leases = [lease.clone()];
    assert!(!edge_may_write(&leases, "outreach-bag"), "the edge refuses");
    assert!(
        edge_may_write(&leases, "main-pharmacy"),
        "outside the scope"
    );
    assert!(lease.holder_may_write("desktop-1", "outreach-bag", at(7)));
    assert!(!lease.holder_may_write("desktop-2", "outreach-bag", at(7)));
    assert!(!lease.holder_may_write("desktop-1", "main-pharmacy", at(7)));
    assert!(
        !lease.holder_may_write("desktop-1", "outreach-bag", at(8)),
        "expired"
    );
}

#[test]
fn a_store_is_under_one_lease_at_a_time() {
    let first = outreach(&[]).expect("granted");
    assert_eq!(
        outreach(std::slice::from_ref(&first)),
        Err(LeaseRefused::Overlaps("outreach-bag".into()))
    );
    let mut returned = first;
    returned.request_return("desktop-1", at(1)).expect("return");
    returned.complete_return(true, at(2)).expect("returned");
    assert!(outreach(&[returned]).is_ok(), "free again once returned");
}

#[test]
fn a_grant_names_a_store_and_a_time_box() {
    let grant = |stores: BTreeSet<String>, hours: i64| {
        Lease::grant(
            Uuid::now_v7(),
            "d",
            stores,
            "admin",
            at(0),
            SignedDuration::from_hours(hours),
            &[],
        )
    };
    assert_eq!(grant(BTreeSet::new(), 8), Err(LeaseRefused::Empty));
    assert_eq!(grant(stores(&["s"]), 0), Err(LeaseRefused::Lifetime));
    assert_eq!(grant(stores(&["s"]), 31 * 24), Err(LeaseRefused::Lifetime));
}

#[test]
fn it_returns_only_once_the_holders_outbox_has_drained() {
    let mut lease = outreach(&[]).expect("granted");
    assert_eq!(
        lease.complete_return(true, at(1)),
        Err(LeaseRefused::State("active"))
    );
    assert_eq!(
        lease.request_return("desktop-2", at(1)),
        Err(LeaseRefused::NotHolder)
    );
    lease.request_return("desktop-1", at(1)).expect("return");
    assert!(
        !lease.holder_may_write("desktop-1", "outreach-bag", at(1)),
        "no new writes while draining"
    );
    assert!(!edge_may_write(
        std::slice::from_ref(&lease),
        "outreach-bag"
    ));
    assert_eq!(
        lease.complete_return(false, at(2)),
        Err(LeaseRefused::NotDrained)
    );
    lease.complete_return(true, at(2)).expect("returned");
    assert_eq!(lease.state, LeaseState::Returned { at: at(2) });
    assert!(edge_may_write(&[lease], "outreach-bag"));
}

#[test]
fn an_unreturned_lease_freezes_the_scope_at_both_ends() {
    let mut lease = outreach(&[]).expect("granted");
    assert!(!lease.expire(at(7)), "not yet");
    assert!(lease.expire(at(9)), "froze: raise the alert");
    assert!(!lease.expire(at(10)), "once");
    assert_eq!(lease.state, LeaseState::Frozen { since: at(8) });
    assert!(!lease.holder_may_write("desktop-1", "outreach-bag", at(9)));
    assert!(!edge_may_write(
        std::slice::from_ref(&lease),
        "outreach-bag"
    ));

    // The holder reconnects: it drains and returns as usual.
    lease.request_return("desktop-1", at(10)).expect("return");
    lease.complete_return(true, at(11)).expect("returned");
}

#[test]
fn only_a_frozen_lease_is_recovered_by_an_admin_with_a_reason() {
    let mut lease = outreach(&[]).expect("granted");
    assert_eq!(
        lease.recover("admin", "laptop lost", at(1)),
        Err(LeaseRefused::State("active"))
    );
    lease.expire(at(9));
    assert_eq!(
        lease.recover("admin", " ", at(10)),
        Err(LeaseRefused::NoReason)
    );
    lease
        .recover("admin", "laptop lost", at(10))
        .expect("recovered");
    assert!(edge_may_write(std::slice::from_ref(&lease), "outreach-bag"));
    assert_eq!(
        lease.request_return("desktop-1", at(11)),
        Err(LeaseRefused::State("recovered"))
    );
}
