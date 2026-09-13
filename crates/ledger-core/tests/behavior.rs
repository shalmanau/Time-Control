use chrono::DateTime;
use ledger_core::{format_local, Store, WEEK};
fn ts(s: &str) -> i64 {
    DateTime::parse_from_rfc3339(s).unwrap().timestamp_millis()
}
fn store() -> Store {
    Store::open(":memory:", "Test device".into(), "Europe/Minsk").unwrap()
}
fn group() -> (Store, Store, Store) {
    let mut a = store();
    let mut b = store();
    let mut c = store();
    a.admit(b.identity().member()).unwrap();
    b.join(a.replica()).unwrap();
    b.admit(c.identity().member()).unwrap();
    c.join(b.replica()).unwrap();
    a.merge(c.replica()).unwrap();
    b.merge(a.replica()).unwrap();
    (a, b, c)
}
fn cat(s: &mut Store) -> String {
    s.add_category("Work", 1000).unwrap().id
}
#[test]
fn past_entries_overlap_adjacency_and_future() {
    let mut s = store();
    let c = cat(&mut s);
    s.save_entry(None, c.clone(), 100, 200, 1000).unwrap();
    assert!(s.save_entry(None, c.clone(), 150, 250, 1000).is_err());
    assert!(s.save_entry(None, c.clone(), 200, 300, 1000).is_ok());
    assert!(s.save_entry(None, c.clone(), 400, 1001, 1000).is_err());
    assert!(s.save_entry(None, c.clone(), 400, 400, 1000).is_err());
    assert!(s.save_entry(None, c, 500, 400, 1000).is_err());
}
#[test]
fn seven_days_from_creation_not_activity_or_edit() {
    let mut s = store();
    let c = cat(&mut s);
    let e = s.save_entry(None, c.clone(), 100, 200, 1000).unwrap();
    s.save_entry(Some(e.id.clone()), c.clone(), 100, 210, 1000 + WEEK - 1)
        .unwrap();
    assert_eq!(s.snapshot().entries[0].created, 1000);
    assert!(s
        .save_entry(Some(e.id.clone()), c, 100, 220, 1000 + WEEK)
        .is_err());
    assert!(s.delete_entry(&e.id, 1000 + WEEK).is_err());
}
#[test]
fn deletion_before_deadline_and_no_resurrection() {
    let (mut a, mut b, mut c) = group();
    let category = cat(&mut a);
    let e = a.save_entry(None, category, 100, 200, 1000).unwrap();
    b.merge(a.replica()).unwrap();
    c.merge(a.replica()).unwrap();
    let stale = c.replica();
    b.delete_entry(&e.id, 1001).unwrap();
    a.merge(b.replica()).unwrap();
    a.merge(stale).unwrap();
    c.merge(a.replica()).unwrap();
    assert!(a.snapshot().entries.is_empty());
    assert!(c.snapshot().entries.is_empty());
}
#[test]
fn persistence_and_timer_restart() {
    let file = tempfile::NamedTempFile::new().unwrap();
    {
        let mut s = Store::open(file.path(), "A".into(), "UTC").unwrap();
        let c = cat(&mut s);
        s.start_timer(c, 1000).unwrap();
    }
    let mut s = Store::open(file.path(), "Ignored".into(), "UTC").unwrap();
    assert_eq!(s.snapshot().timer.unwrap().start, 1000);
    let e = s.stop_timer(1000 + WEEK + 10).unwrap();
    assert_eq!(e.created, 1000 + WEEK + 10);
    assert!(s.snapshot().timer.is_none());
}
#[test]
fn timer_handoff_and_no_duplicate_stop() {
    let (mut a, mut b, mut c) = group();
    let category = cat(&mut a);
    a.start_timer(category, 1000).unwrap();
    b.merge(a.replica()).unwrap();
    let e = b.stop_timer(4000).unwrap();
    a.merge(b.replica()).unwrap();
    c.merge(a.replica()).unwrap();
    assert_eq!(a.snapshot().entries, vec![e.clone()]);
    assert_eq!(c.snapshot().entries, vec![e]);
    assert!(a.stop_timer(5000).is_err());
}
#[test]
fn concurrent_timers_and_overlaps_use_join_priority() {
    let (mut a, mut b, mut c) = group();
    let category = cat(&mut a);
    b.merge(a.replica()).unwrap();
    c.merge(a.replica()).unwrap();
    a.start_timer(category.clone(), 1000).unwrap();
    b.start_timer(category.clone(), 1100).unwrap();
    c.start_timer(category, 1200).unwrap();
    let id = c.snapshot().timer.unwrap().id;
    a.merge(b.replica()).unwrap();
    a.merge(c.replica()).unwrap();
    c.merge(a.replica()).unwrap();
    assert_eq!(a.snapshot().timer.unwrap().id, id);
    assert_eq!(c.snapshot().timer.unwrap().id, id);
    assert_eq!(a.snapshot().excluded_count, 2);
}
#[test]
fn independent_changes_and_conflicting_edits_converge() {
    let (mut a, mut b, mut c) = group();
    let category = cat(&mut a);
    let e = a
        .save_entry(None, category.clone(), 100, 200, 1000)
        .unwrap();
    b.merge(a.replica()).unwrap();
    c.merge(a.replica()).unwrap();
    a.save_entry(Some(e.id.clone()), category.clone(), 100, 210, 1100)
        .unwrap();
    b.save_entry(Some(e.id.clone()), category.clone(), 100, 220, 1100)
        .unwrap();
    c.save_entry(Some(e.id.clone()), category.clone(), 100, 230, 1100)
        .unwrap();
    b.save_entry(None, category, 300, 400, 1200).unwrap();
    let ar = a.replica();
    let br = b.replica();
    let cr = c.replica();
    a.merge(cr.clone()).unwrap();
    a.merge(br.clone()).unwrap();
    b.merge(ar.clone()).unwrap();
    b.merge(cr).unwrap();
    c.merge(br).unwrap();
    c.merge(ar).unwrap();
    assert_eq!(a.snapshot().entries, b.snapshot().entries);
    assert_eq!(a.snapshot().entries, c.snapshot().entries);
    assert_eq!(a.snapshot().entries[0].end, 230);
    assert_eq!(a.snapshot().entries.len(), 2);
}
#[test]
fn causally_later_edit_from_old_device_wins() {
    let (mut a, mut b, mut c) = group();
    let category = cat(&mut c);
    let e = c
        .save_entry(None, category.clone(), 100, 200, 1000)
        .unwrap();
    a.merge(c.replica()).unwrap();
    a.save_entry(Some(e.id.clone()), category, 100, 250, 2000)
        .unwrap();
    c.merge(a.replica()).unwrap();
    b.merge(c.replica()).unwrap();
    assert_eq!(c.snapshot().entries[0].end, 250);
}
#[test]
fn overlap_keeps_whole_priority_entry() {
    let (mut a, mut b, mut c) = group();
    let category = cat(&mut a);
    b.merge(a.replica()).unwrap();
    c.merge(a.replica()).unwrap();
    a.save_entry(None, category.clone(), 100, 300, 1000)
        .unwrap();
    let e = c.save_entry(None, category, 200, 250, 1000).unwrap();
    a.merge(c.replica()).unwrap();
    assert_eq!(a.snapshot().entries, vec![e]);
    assert_eq!(a.snapshot().excluded_count, 1);
}
#[test]
fn sync_after_edit_window_accepts_predeadline_changes() {
    let (mut a, mut b, _) = group();
    let c = cat(&mut a);
    let e = a.save_entry(None, c.clone(), 100, 200, 1000).unwrap();
    b.merge(a.replica()).unwrap();
    b.save_entry(Some(e.id), c, 100, 250, 1000 + WEEK - 1)
        .unwrap();
    a.merge(b.replica()).unwrap();
    assert_eq!(a.snapshot().entries[0].end, 250);
}
#[test]
fn categories_deduplicate_and_are_reusable() {
    let (mut a, mut b, _) = group();
    let x = a.add_category(" Work ", 1000).unwrap();
    assert_eq!(a.add_category("work", 1001).unwrap().id, x.id);
    let y = b.add_category("WORK", 1000).unwrap();
    b.save_entry(None, y.id, 100, 200, 1000).unwrap();
    a.merge(b.replica()).unwrap();
    assert_eq!(a.snapshot().categories.len(), 1);
    assert_eq!(
        a.snapshot().entries[0].category,
        a.snapshot().categories[0].id
    );
}
#[test]
fn rejects_tampered_operations_and_foreign_groups() {
    let (mut a, mut b, _) = group();
    let c = cat(&mut a);
    a.save_entry(None, c, 100, 200, 1000).unwrap();
    let mut r = a.replica();
    r.operations[0].at = 999;
    assert!(b.merge(r).is_err());
    assert!(b.merge(store().replica()).is_err());
}
#[test]
fn overnight_and_month_boundaries() {
    let mut s = Store::open(":memory:", "A".into(), "UTC").unwrap();
    let c = cat(&mut s);
    let end = ts("2026-02-01T02:00:00Z");
    s.save_entry(None, c, ts("2026-01-31T22:00:00Z"), end, end)
        .unwrap();
    assert_eq!(
        s.report("day", "2026-01-31", end).unwrap().recorded,
        2 * 3600000
    );
    assert_eq!(
        s.report("month", "2026-02-01", end).unwrap().recorded,
        2 * 3600000
    );
    assert_eq!(s.report("month", "2026-02-01", end).unwrap().gaps, 0);
}
#[test]
fn dst_days_have_actual_elapsed_length() {
    let s = Store::open(":memory:", "A".into(), "Europe/Berlin").unwrap();
    let spring = s
        .report("day", "2026-03-29", ts("2026-12-01T00:00:00Z"))
        .unwrap();
    let fall = s
        .report("day", "2026-10-25", ts("2026-12-01T00:00:00Z"))
        .unwrap();
    assert_eq!(spring.gaps, 23 * 3600000);
    assert_eq!(fall.gaps, 25 * 3600000);
    assert!(s.parse_local("2026-03-29T02:30").is_err());
    assert!(s.parse_local("2026-10-25T02:30").is_err());
}
#[test]
fn week_starts_monday_and_running_time_is_provisional() {
    let mut s = store();
    let c = cat(&mut s);
    let start = ts("2026-09-13T10:00:00Z");
    s.start_timer(c, start).unwrap();
    let r = s.report("week", "2026-09-13", start + 3600000).unwrap();
    assert!(r.provisional);
    assert_eq!(r.recorded, 3600000);
    assert_eq!(
        format_local(r.start, "Europe/Minsk").unwrap(),
        "2026-09-07T00:00"
    );
    assert_eq!(r.end, start + 3600000);
    assert_eq!(r.categories[0].share, 1.);
}
#[test]
fn joining_through_any_member_and_concurrent_admissions() {
    let (mut a, mut b, mut c) = group();
    let mut d = store();
    let mut e = store();
    a.admit(d.identity().member()).unwrap();
    b.admit(e.identity().member()).unwrap();
    d.join(a.replica()).unwrap();
    e.join(b.replica()).unwrap();
    c.merge(d.replica()).unwrap();
    c.merge(e.replica()).unwrap();
    a.merge(c.replica()).unwrap();
    b.merge(c.replica()).unwrap();
    assert_eq!(a.replica().group.members.len(), 5);
    assert_eq!(
        serde_json::to_string(&a.replica().group).unwrap(),
        serde_json::to_string(&b.replica().group).unwrap()
    );
}

#[test]
fn editing_a_short_timer_preserves_unmodified_seconds() {
    let mut s = store();
    let c = cat(&mut s);
    let other = s.add_category("Rest", 1000).unwrap().id;
    s.start_timer(c, 1001).unwrap();
    let e = s.stop_timer(1555).unwrap();
    let start = format_local(e.start, "Europe/Minsk").unwrap();
    let end = format_local(e.end, "Europe/Minsk").unwrap();
    let edited = s
        .save_entry_local(Some(e.id), other, &start, &end, 2000)
        .unwrap();
    assert_eq!(edited.start, 1001);
    assert_eq!(edited.end, 1555);
    assert_eq!(edited.created, 1555);
}
#[test]
fn retrying_an_admission_is_idempotent() {
    let mut a = store();
    let b = store();
    let first = a.admit(b.identity().member()).unwrap();
    let again = a.admit(b.identity().member()).unwrap();
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&again).unwrap()
    );
}

#[test]
fn chosen_device_priority_controls_conflicts_and_survives_stale_sync() {
    let (mut a, mut b, mut c) = group();
    let category = cat(&mut a);
    b.merge(a.replica()).unwrap();
    c.merge(a.replica()).unwrap();
    let ea = a
        .save_entry(None, category.clone(), 100, 200, 1000)
        .unwrap();
    c.save_entry(None, category, 150, 250, 1000).unwrap();
    let stale = c.replica();
    let order = vec![a.identity().id, b.identity().id, c.identity().id];
    b.set_device_priority(order.clone(), 1100).unwrap();
    a.merge(b.replica()).unwrap();
    a.merge(c.replica()).unwrap();
    c.merge(a.replica()).unwrap();
    b.merge(c.replica()).unwrap();
    for s in [&a, &b, &c] {
        assert_eq!(s.snapshot().device_priority, order);
        assert_eq!(s.snapshot().entries, vec![ea.clone()]);
        s.replica().group.validate().unwrap();
    }
    a.merge(stale).unwrap();
    assert_eq!(a.snapshot().entries, vec![ea]);
    assert!(a
        .set_device_priority(vec![a.identity().id; 3], 1200)
        .is_err());
    assert!(a
        .set_device_priority(vec!["unknown".into(); 3], 1200)
        .is_err());
}

#[test]
fn concurrent_priority_changes_converge_then_causal_change_wins() {
    let (mut a, mut b, mut c) = group();
    let first = vec![a.identity().id, b.identity().id, c.identity().id];
    let second = vec![b.identity().id, c.identity().id, a.identity().id];
    a.set_device_priority(first.clone(), 1000).unwrap();
    c.set_device_priority(second.clone(), 1000).unwrap();
    b.merge(a.replica()).unwrap();
    b.merge(c.replica()).unwrap();
    c.merge(a.replica()).unwrap();
    a.merge(c.replica()).unwrap();
    assert_eq!(a.snapshot().device_priority, second);
    assert_eq!(b.snapshot().device_priority, second);
    a.set_device_priority(first.clone(), 1100).unwrap();
    c.merge(a.replica()).unwrap();
    b.merge(c.replica()).unwrap();
    assert_eq!(b.snapshot().device_priority, first);
    // New members still start at the highest priority until reordered.
    let d = store();
    a.admit(d.identity().member()).unwrap();
    assert_eq!(a.snapshot().device_priority[0], d.identity().id);
}

#[test]
fn colors_and_priority_persist_and_color_edits_converge() {
    let (mut a, mut b, mut c) = group();
    let category = cat(&mut a);
    let color = a.snapshot().category_colors[&category].clone();
    assert!(color.starts_with('#') && color.len() == 7);
    b.merge(a.replica()).unwrap();
    c.merge(a.replica()).unwrap();
    a.set_category_color(&category, "#ff0000", 1100).unwrap();
    c.set_category_color(&category, "#0055FF", 1100).unwrap();
    a.merge(c.replica()).unwrap();
    b.merge(a.replica()).unwrap();
    assert_eq!(b.snapshot().category_colors[&category], "#0055ff");
    a.set_category_color(&category, "#009900", 1200).unwrap();
    c.merge(a.replica()).unwrap();
    assert_eq!(c.snapshot().category_colors[&category], "#009900");
    assert!(a.set_category_color(&category, "url(bad)", 1200).is_err());
    assert!(a.set_category_color("missing", "#112233", 1200).is_err());
    let file = tempfile::NamedTempFile::new().unwrap();
    let mut local = Store::open(file.path(), "Persist".into(), "UTC").unwrap();
    let id = local.add_category("Rest", 1000).unwrap().id;
    local.set_category_color(&id, "#123456", 1100).unwrap();
    let order = vec![local.identity().id];
    local.set_device_priority(order.clone(), 1200).unwrap();
    drop(local);
    let local = Store::open(file.path(), "Ignored".into(), "UTC").unwrap();
    assert_eq!(local.snapshot().category_colors[&id], "#123456");
    assert_eq!(local.snapshot().device_priority, order);
}
