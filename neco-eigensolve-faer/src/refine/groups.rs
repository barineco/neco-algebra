use crate::{try_filled, EigensolveFaerError};
use neco_complex::Complex;

pub(super) const GROUP_RADIUS: f64 = 1e-6;

fn find(parents: &mut [usize], start: usize) -> usize {
    let mut root = start;
    while parents[root] != root {
        root = parents[root];
    }
    let mut node = start;
    while parents[node] != root {
        let next = parents[node];
        parents[node] = root;
        node = next;
    }
    root
}

pub(super) fn form(
    upper_lambda: &[Complex<f64>],
    flagged: &[bool],
) -> Result<Vec<Vec<usize>>, EigensolveFaerError> {
    let count = upper_lambda.len();
    let mut parents: Vec<usize> = try_filled(count, 0)?;
    for (index, parent) in parents.iter_mut().enumerate() {
        *parent = index;
    }
    let mut by_imaginary: Vec<usize> = try_filled(count, 0)?;
    for (index, slot) in by_imaginary.iter_mut().enumerate() {
        *slot = index;
    }
    by_imaginary.sort_unstable_by(|&left, &right| {
        upper_lambda[left]
            .imaginary()
            .partial_cmp(upper_lambda[right].imaginary())
            .unwrap_or(core::cmp::Ordering::Equal)
            .then(left.cmp(&right))
    });
    for (offset, &first) in by_imaginary.iter().enumerate() {
        let radius = GROUP_RADIUS * upper_lambda[first].norm();
        for &second in &by_imaginary[offset + 1..] {
            let rise = upper_lambda[second].imaginary() - upper_lambda[first].imaginary();
            if rise > radius {
                break;
            }
            let run = upper_lambda[second].real() - upper_lambda[first].real();
            if run.hypot(rise) <= radius {
                let (a, b) = (find(&mut parents, first), find(&mut parents, second));
                if a != b {
                    parents[a] = b;
                }
            }
        }
    }
    let mut flagged_root: Vec<bool> = try_filled(count, false)?;
    for (index, &flag) in flagged.iter().enumerate() {
        if flag {
            let root = find(&mut parents, index);
            flagged_root[root] = true;
        }
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (root, &marked) in flagged_root.iter().enumerate() {
        if !marked || find(&mut parents, root) != root {
            continue;
        }
        let mut size = 0;
        for index in 0..count {
            if find(&mut parents, index) == root {
                size += 1;
            }
        }
        let mut members: Vec<usize> = try_filled(size, 0)?;
        let mut slot = 0;
        for index in 0..count {
            if find(&mut parents, index) == root {
                members[slot] = index;
                slot += 1;
            }
        }
        groups
            .try_reserve(1)
            .map_err(|_| crate::allocation_error(groups.len() + 1))?;
        groups.push(members);
    }
    Ok(groups)
}

pub(super) fn center(members: &[usize], upper_lambda: &[Complex<f64>]) -> Complex<f64> {
    let mut sum = Complex::<f64>::zero();
    for &member in members {
        sum += upper_lambda[member];
    }
    Complex::new(
        sum.real() / members.len() as f64,
        sum.imaginary() / members.len() as f64,
    )
}

pub(super) fn near_real_axis(center: Complex<f64>) -> bool {
    *center.imaginary() <= GROUP_RADIUS * center.norm()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_join_eigenvalues_within_the_relative_radius() {
        let lambda = [
            Complex::new(-1.0, 100.0),
            Complex::new(-1.0, 100.0 + 5e-5),
            Complex::new(-1.0, 100.1),
            Complex::new(-1.0, 100.0 + 9e-5),
        ];
        let groups = form(&lambda, &[true, false, false, false]).unwrap();
        assert_eq!(groups.len(), 1);
        let mut members = groups[0].clone();
        members.sort_unstable();
        assert_eq!(members, vec![0, 1, 3]);
    }

    #[test]
    fn groups_without_a_flagged_member_are_skipped() {
        let lambda = [Complex::new(-1.0, 100.0), Complex::new(-1.0, 200.0)];
        assert!(form(&lambda, &[false, false]).unwrap().is_empty());
    }

    #[test]
    fn near_real_axis_compares_the_imaginary_part_with_the_radius() {
        assert!(near_real_axis(Complex::new(1.0, 1e-8)));
        assert!(!near_real_axis(Complex::new(1.0, 1e-3)));
    }
}
