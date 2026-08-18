use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicBool, Ordering};
use std::alloc::System;

use neco_bigint::{BigInt, BigUint, BigintError, Dyadic, RawRational};

struct FailNextAllocator;

static FAIL_NEXT: AtomicBool = AtomicBool::new(false);

// SAFETY: Requests selected for success are delegated unchanged to System.
unsafe impl GlobalAlloc for FailNextAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if FAIL_NEXT.swap(false, Ordering::SeqCst) {
            core::ptr::null_mut()
        } else {
            // SAFETY: The caller supplied a layout satisfying GlobalAlloc requirements.
            unsafe { System.alloc(layout) }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: Successful allocations above come from System with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: FailNextAllocator = FailNextAllocator;

fn limbs(values: &[u32]) -> BigUint {
    BigUint::from_limbs_le(values.to_vec()).expect("valid input limbs")
}

fn arm_next_allocation_failure() {
    assert!(!FAIL_NEXT.swap(true, Ordering::SeqCst));
}

fn assert_failure_consumed() {
    assert!(!FAIL_NEXT.load(Ordering::SeqCst));
}

#[test]
fn representative_public_paths_report_exact_allocation_payloads() {
    let clone_input = limbs(&[1, 2, 3]);
    arm_next_allocation_failure();
    assert_eq!(
        clone_input.try_clone(),
        Err(BigintError::AllocationFailure { requested_limbs: 3 })
    );
    assert_failure_consumed();

    let add_left = limbs(&[1, 1]);
    let add_right = limbs(&[1]);
    arm_next_allocation_failure();
    assert_eq!(
        add_left.add(&add_right),
        Err(BigintError::AllocationFailure { requested_limbs: 2 })
    );
    assert_failure_consumed();

    let carrying_add_left = limbs(&[u32::MAX, u32::MAX]);
    let carrying_add_right = limbs(&[1]);
    arm_next_allocation_failure();
    assert_eq!(
        carrying_add_left.add(&carrying_add_right),
        Err(BigintError::AllocationFailure { requested_limbs: 3 })
    );
    assert_failure_consumed();

    let sub_left = limbs(&[0, 0, 1]);
    let sub_right = limbs(&[1]);
    arm_next_allocation_failure();
    assert_eq!(
        sub_left.checked_sub(&sub_right),
        Err(BigintError::AllocationFailure { requested_limbs: 2 })
    );
    assert_failure_consumed();

    let mul_left = limbs(&[u32::MAX, 1]);
    let mul_right = limbs(&[2, 3]);
    arm_next_allocation_failure();
    assert_eq!(
        mul_left.mul(&mul_right),
        Err(BigintError::AllocationFailure { requested_limbs: 3 })
    );
    assert_failure_consumed();

    let shift_without_carry = limbs(&[1, 1]);
    arm_next_allocation_failure();
    assert_eq!(
        shift_without_carry.shl_bits(33),
        Err(BigintError::AllocationFailure { requested_limbs: 3 })
    );
    assert_failure_consumed();

    let shift_input = limbs(&[0x1234_5678, 0x9abc_def0]);
    arm_next_allocation_failure();
    assert_eq!(
        shift_input.shl_bits(36),
        Err(BigintError::AllocationFailure { requested_limbs: 4 })
    );
    assert_failure_consumed();

    let dividend = limbs(&[0, 0, 1]);
    let divisor = limbs(&[1]);
    arm_next_allocation_failure();
    assert_eq!(
        dividend.div_rem(&divisor),
        Err(BigintError::AllocationFailure { requested_limbs: 3 })
    );
    assert_failure_consumed();

    let power_base = limbs(&[3, 1]);
    arm_next_allocation_failure();
    assert_eq!(
        power_base.pow_u32(5),
        Err(BigintError::AllocationFailure { requested_limbs: 1 })
    );
    assert_failure_consumed();

    let gcd_left = limbs(&[6, 2]);
    let gcd_right = limbs(&[4, 1]);
    arm_next_allocation_failure();
    assert_eq!(
        gcd_left.gcd(&gcd_right),
        Err(BigintError::AllocationFailure { requested_limbs: 2 })
    );
    assert_failure_consumed();

    let lcm_left = limbs(&[6, 2]);
    let lcm_right = limbs(&[4, 1]);
    arm_next_allocation_failure();
    assert_eq!(
        lcm_left.lcm(&lcm_right),
        Err(BigintError::AllocationFailure { requested_limbs: 2 })
    );
    assert_failure_consumed();

    let extended_left = BigInt::from_sign_magnitude(neco_bigint::Sign::Positive, limbs(&[6, 2]));
    let extended_right = BigInt::from_sign_magnitude(neco_bigint::Sign::Negative, limbs(&[4, 1]));
    arm_next_allocation_failure();
    assert_eq!(
        extended_left.extended_gcd(&extended_right),
        Err(BigintError::AllocationFailure { requested_limbs: 2 })
    );
    assert_failure_consumed();

    let rational_left = RawRational::new(BigInt::try_from(1).unwrap(), limbs(&[3]))
        .reduce()
        .unwrap()
        .into_reduced();
    let rational_right = RawRational::new(BigInt::try_from(2).unwrap(), limbs(&[5]))
        .reduce()
        .unwrap()
        .into_reduced();
    arm_next_allocation_failure();
    assert_eq!(
        rational_left.add(&rational_right),
        Err(BigintError::AllocationFailure { requested_limbs: 1 })
    );
    assert_failure_consumed();

    let dyadic_left = Dyadic::new(BigInt::try_from(3).unwrap(), 4);
    let dyadic_right = Dyadic::new(BigInt::try_from(5).unwrap(), 4);
    arm_next_allocation_failure();
    assert_eq!(
        dyadic_left.add(&dyadic_right),
        Err(BigintError::AllocationFailure { requested_limbs: 1 })
    );
    assert_failure_consumed();
}
