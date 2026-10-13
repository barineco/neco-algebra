use core::mem::size_of;

use neco_expr::NecoFailure;

const MAXIMUM_BYTES: usize = 96;

#[test]
fn neco_failure_size_stays_within_bound() {
    let actual = size_of::<NecoFailure>();
    assert!(
        actual <= MAXIMUM_BYTES,
        "NecoFailure is {actual} bytes; the bound is {MAXIMUM_BYTES}"
    );
}
