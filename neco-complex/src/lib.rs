#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Complex<T> {
    re: T,
    im: T,
}

impl<T> Complex<T> {
    pub const fn new(re: T, im: T) -> Self {
        Self { re, im }
    }

    pub fn real(&self) -> &T {
        &self.re
    }

    pub fn imaginary(&self) -> &T {
        &self.im
    }

    pub fn into_parts(self) -> (T, T) {
        (self.re, self.im)
    }

    pub fn real_value(self) -> T
    where
        T: Copy,
    {
        self.re
    }

    pub fn imaginary_value(self) -> T
    where
        T: Copy,
    {
        self.im
    }

    pub fn set_real(&mut self, value: T) {
        self.re = value;
    }

    pub fn set_imaginary(&mut self, value: T) {
        self.im = value;
    }
}

impl Complex<f32> {
    pub const fn zero() -> Self {
        Self::new(0.0, 0.0)
    }

    pub const fn one() -> Self {
        Self::new(1.0, 0.0)
    }
}

impl Complex<f64> {
    pub const fn zero() -> Self {
        Self::new(0.0, 0.0)
    }

    pub const fn one() -> Self {
        Self::new(1.0, 0.0)
    }

    pub const fn from_real(value: f64) -> Self {
        Self::new(value, 0.0)
    }

    #[cfg(feature = "std")]
    pub fn argument(self) -> f64 {
        self.im.atan2(self.re)
    }
}

#[cfg(feature = "std")]
impl Complex<f32> {
    pub fn argument(self) -> f32 {
        self.im.atan2(self.re)
    }
}

impl<T> Complex<T>
where
    T: Copy + Neg<Output = T>,
{
    pub fn conjugate(self) -> Self {
        Self::new(self.re, -self.im)
    }
}

impl<T> Complex<T>
where
    T: Copy + Add<Output = T> + Mul<Output = T>,
{
    pub fn norm_squared(self) -> T {
        self.re * self.re + self.im * self.im
    }
}

#[cfg(feature = "std")]
impl Complex<f64> {
    pub fn norm(self) -> f64 {
        self.re.hypot(self.im)
    }
}

impl<T> Neg for Complex<T>
where
    T: Neg<Output = T>,
{
    type Output = Self;

    fn neg(self) -> Self {
        Self::new(-self.re, -self.im)
    }
}

impl<T> Add for Complex<T>
where
    T: Add<Output = T>,
{
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self::new(self.re + rhs.re, self.im + rhs.im)
    }
}

impl<T> Sub for Complex<T>
where
    T: Sub<Output = T>,
{
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self::new(self.re - rhs.re, self.im - rhs.im)
    }
}

impl<T> Mul for Complex<T>
where
    T: Copy + Add<Output = T> + Sub<Output = T> + Mul<Output = T>,
{
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        Self::new(
            self.re * rhs.re - self.im * rhs.im,
            self.re * rhs.im + self.im * rhs.re,
        )
    }
}

impl<T> Div for Complex<T>
where
    T: Copy + Add<Output = T> + Sub<Output = T> + Mul<Output = T> + Div<Output = T>,
{
    type Output = Self;

    fn div(self, rhs: Self) -> Self {
        let denominator = rhs.re * rhs.re + rhs.im * rhs.im;
        Self::new(
            (self.re * rhs.re + self.im * rhs.im) / denominator,
            (self.im * rhs.re - self.re * rhs.im) / denominator,
        )
    }
}

impl<T> Mul<T> for Complex<T>
where
    T: Copy + Mul<Output = T>,
{
    type Output = Self;

    fn mul(self, rhs: T) -> Self {
        Self::new(self.re * rhs, self.im * rhs)
    }
}

impl<T> Div<T> for Complex<T>
where
    T: Copy + Div<Output = T>,
{
    type Output = Self;

    fn div(self, rhs: T) -> Self {
        Self::new(self.re / rhs, self.im / rhs)
    }
}

impl<T> AddAssign for Complex<T>
where
    T: AddAssign,
{
    fn add_assign(&mut self, rhs: Self) {
        self.re += rhs.re;
        self.im += rhs.im;
    }
}

impl<T> SubAssign for Complex<T>
where
    T: SubAssign,
{
    fn sub_assign(&mut self, rhs: Self) {
        self.re -= rhs.re;
        self.im -= rhs.im;
    }
}

impl<T> MulAssign for Complex<T>
where
    T: Copy + Add<Output = T> + Sub<Output = T> + Mul<Output = T>,
{
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl<T> DivAssign for Complex<T>
where
    T: Copy + Add<Output = T> + Sub<Output = T> + Mul<Output = T> + Div<Output = T>,
{
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}

#[cfg(test)]
mod tests {
    use super::Complex;

    #[test]
    fn complex_arithmetic_preserves_components() {
        let left = Complex::new(1.0_f64, 2.0);
        let right = Complex::new(3.0_f64, -1.0);
        assert_eq!(left + right, Complex::new(4.0, 1.0));
        assert_eq!(left - right, Complex::new(-2.0, 3.0));
        assert_eq!(left * right, Complex::new(5.0, 5.0));
        assert_eq!(left.conjugate(), Complex::new(1.0, -2.0));
    }

    #[test]
    fn division_recovers_the_input_value() {
        let value = Complex::new(1.0_f64, 2.0);
        let factor = Complex::new(3.0_f64, -1.0);
        assert_eq!((value * factor) / factor, value);
    }

    #[test]
    fn norm_squared_is_available_in_each_runtime_configuration() {
        assert_eq!(Complex::new(3.0_f64, 4.0).norm_squared(), 25.0);
    }

    #[test]
    fn components_are_observed_and_replaced_through_methods() {
        let mut value = Complex::new(1.0_f64, -2.0);
        assert_eq!(value.real_value(), 1.0);
        assert_eq!(value.imaginary_value(), -2.0);
        value.set_real(3.0);
        value.set_imaginary(4.0);
        assert_eq!(value.into_parts(), (3.0, 4.0));
    }

    #[cfg(feature = "std")]
    #[test]
    fn norm_is_the_euclidean_magnitude() {
        assert_eq!(Complex::new(3.0_f64, 4.0).norm(), 5.0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn argument_uses_the_component_quadrants() {
        assert_eq!(
            Complex::new(0.0_f64, 1.0).argument(),
            core::f64::consts::FRAC_PI_2
        );
    }
}
