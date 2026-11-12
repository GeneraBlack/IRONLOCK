//! Pure-Rust deterministic fixed-point (`SimScalar` / `I32F32`) and trigonometric math
//! for bit-identical lockstep simulation across `debug`/`release` and CPU architectures.
//!
//! Platform `libm` implementations (`f32::sin`, `f32::cos`, `f32::atan2`, `f32::sqrt`) and
//! compiler FMA fusion can differ by 1+ ULP between x86_64 MSVC, x86_64 GNU, and aarch64, or
//! between unoptimized `debug` and optimized `--release` builds. `SimScalar` performs exact
//! 64-bit signed fixed-point (`I32F32`) integer arithmetic with `i128` intermediates and
//! integer bit-level IEEE-754 boundary conversion so every build profile and platform produces
//! 100% bit-identical results.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// 64-bit signed fixed-point scalar (`I32F32`: 32 integer bits, 32 fractional bits).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SimScalar(pub i64);

impl fmt::Debug for SimScalar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SimScalar({:.6}, raw={})", self.to_f32(), self.0)
    }
}

impl fmt::Display for SimScalar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.6}", self.to_f32())
    }
}

impl SimScalar {
    /// Number of fractional bits (`32`).
    pub const FRAC_BITS: u32 = 32;
    /// Fixed-point unit scale (`1 << 32`).
    pub const SCALE: i64 = 1_i64 << Self::FRAC_BITS;

    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(Self::SCALE);
    pub const NEG_ONE: Self = Self(-Self::SCALE);
    pub const HALF: Self = Self(1_i64 << 31);
    pub const TWO: Self = Self(2_i64 << Self::FRAC_BITS);
    pub const PI: Self = Self(13_493_037_705);
    pub const TAU: Self = Self(26_986_075_409);
    pub const FRAC_PI_2: Self = Self(6_746_518_852);
    pub const FRAC_PI_4: Self = Self(3_373_259_426);
    pub const FRAC_1_SQRT_2: Self = Self(3_037_000_500);
    pub const MIN: Self = Self(i64::MIN);
    pub const MAX: Self = Self(i64::MAX);

    #[inline]
    pub const fn from_raw(raw: i64) -> Self {
        Self(raw)
    }

    #[inline]
    pub const fn raw(self) -> i64 {
        self.0
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Self {
        Self((v as i64) << Self::FRAC_BITS)
    }

    #[inline]
    pub const fn from_num(v: i32) -> Self {
        Self::from_i32(v)
    }

    #[inline]
    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    #[inline]
    pub const fn from_u32(v: u32) -> Self {
        if v >= i32::MAX as u32 {
            Self(i64::MAX)
        } else {
            Self((v as i64) << Self::FRAC_BITS)
        }
    }

    /// Pure integer IEEE-754 `f32 -> SimScalar` (`I32F32`) conversion with zero float ops.
    /// Non-finite `NaN` maps to `0`; `+/-Infinity` and out-of-range magnitudes saturate.
    #[inline]
    pub const fn from_f32(v: f32) -> Self {
        let bits = v.to_bits();
        let neg = (bits >> 31) != 0;
        let exp = ((bits >> 23) & 0xFF) as i32;
        let mant = bits & 0x007F_FFFF;

        if exp == 0xFF {
            // NaN -> ZERO; +/-Inf -> saturate
            if mant != 0 {
                return Self::ZERO;
            }
            return if neg { Self::MIN } else { Self::MAX };
        }
        if exp == 0 && mant == 0 {
            return Self::ZERO;
        }

        // Normal: value = (1 << 23 | mant) * 2^(exp - 127 - 23)
        // Subnormal: value = mant * 2^(1 - 127 - 23)
        let (sig, unbiased_shift) = if exp == 0 {
            (mant as u64, 1 - 127 - 23 + (Self::FRAC_BITS as i32))
        } else {
            (
                ((1_u32 << 23) | mant) as u64,
                exp - 127 - 23 + (Self::FRAC_BITS as i32),
            )
        };

        if unbiased_shift >= 40 {
            // (1 << 23) << 40 == 1 << 63 -> overflows signed i64
            return if neg { Self::MIN } else { Self::MAX };
        }

        let mag_u64 = if unbiased_shift >= 0 {
            sig << (unbiased_shift as u32)
        } else {
            let rshift = (-unbiased_shift) as u32;
            if rshift >= 64 {
                0
            } else {
                let q = sig >> rshift;
                let rem = sig & ((1_u64 << rshift) - 1);
                let half = 1_u64 << (rshift - 1);
                if rem > half || (rem == half && (q & 1) == 1) {
                    q + 1
                } else {
                    q
                }
            }
        };

        if neg {
            if mag_u64 > (i64::MAX as u64) + 1 {
                Self::MIN
            } else if mag_u64 == (i64::MAX as u64) + 1 {
                Self(i64::MIN)
            } else {
                Self(-(mag_u64 as i64))
            }
        } else if mag_u64 > i64::MAX as u64 {
            Self::MAX
        } else {
            Self(mag_u64 as i64)
        }
    }

    /// Pure integer IEEE-754 round-to-nearest-even `SimScalar -> f32` conversion.
    #[inline]
    pub const fn to_f32(self) -> f32 {
        if self.0 == 0 {
            return f32::from_bits(0);
        }
        let neg = self.0 < 0;
        let mag = self.0.unsigned_abs();
        let lz = mag.leading_zeros();
        let msb = 63 - lz; // 0..=63
        let mut biased_exp = (msb as i32) - (Self::FRAC_BITS as i32) + 127;

        let mant24 = if msb <= 23 {
            mag << (23 - msb)
        } else {
            let shift = msb - 23;
            let mut q = mag >> shift;
            let rem = mag & ((1_u64 << shift) - 1);
            let half = 1_u64 << (shift - 1);
            if rem > half || (rem == half && (q & 1) == 1) {
                q += 1;
                if q == (1_u64 << 24) {
                    q >>= 1;
                    biased_exp += 1;
                }
            }
            q
        };

        let sign_bit = if neg { 1_u32 << 31 } else { 0 };
        let bits = sign_bit | ((biased_exp as u32) << 23) | ((mant24 as u32) & 0x007F_FFFF);
        f32::from_bits(bits)
    }

    #[inline]
    pub const fn abs(self) -> Self {
        if self.0 == i64::MIN {
            Self::MAX
        } else if self.0 < 0 {
            Self(-self.0)
        } else {
            self
        }
    }

    #[inline]
    pub const fn min(self, other: Self) -> Self {
        if self.0 <= other.0 {
            self
        } else {
            other
        }
    }

    #[inline]
    pub const fn max(self, other: Self) -> Self {
        if self.0 >= other.0 {
            self
        } else {
            other
        }
    }

    #[inline]
    pub const fn clamp(self, lo: Self, hi: Self) -> Self {
        self.max(lo).min(hi)
    }

    #[inline]
    pub const fn floor_to_i32(self) -> i32 {
        (self.0 >> Self::FRAC_BITS) as i32
    }

    #[inline]
    pub const fn round_to_i32(self) -> i32 {
        let adjusted = if self.0 >= 0 {
            self.0.saturating_add(1_i64 << 31)
        } else {
            self.0.saturating_sub(1_i64 << 31)
        };
        (adjusted >> Self::FRAC_BITS) as i32
    }

    #[inline]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    #[inline]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    #[inline]
    pub const fn saturating_mul(self, rhs: Self) -> Self {
        let prod = (self.0 as i128) * (rhs.0 as i128);
        let shifted = prod >> Self::FRAC_BITS;
        if shifted > i64::MAX as i128 {
            Self::MAX
        } else if shifted < i64::MIN as i128 {
            Self::MIN
        } else {
            Self(shifted as i64)
        }
    }

    #[inline]
    pub const fn checked_div(self, rhs: Self) -> Option<Self> {
        if rhs.0 == 0 {
            return None;
        }
        let num = (self.0 as i128) << Self::FRAC_BITS;
        let q = num / (rhs.0 as i128);
        if q > i64::MAX as i128 {
            Some(Self::MAX)
        } else if q < i64::MIN as i128 {
            Some(Self::MIN)
        } else {
            Some(Self(q as i64))
        }
    }

    /// Deterministic integer square root (`sqrt(max(self, 0))`).
    #[inline]
    pub const fn det_sqrt(self) -> Self {
        if self.0 <= 0 {
            return Self::ZERO;
        }
        let radicand = (self.0 as u128) << Self::FRAC_BITS;
        let root = isqrt_u128(radicand);
        if root > i64::MAX as u64 {
            Self::MAX
        } else {
            Self(root as i64)
        }
    }
}

impl Neg for SimScalar {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self::Output {
        Self(self.0.saturating_neg())
    }
}

impl Add for SimScalar {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        self.saturating_add(rhs)
    }
}

impl AddAssign for SimScalar {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for SimScalar {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        self.saturating_sub(rhs)
    }
}

impl SubAssign for SimScalar {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Mul for SimScalar {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        self.saturating_mul(rhs)
    }
}

impl MulAssign for SimScalar {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl Div for SimScalar {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        if rhs.0 == 0 {
            if self.0 == 0 {
                Self::ZERO
            } else if self.0 > 0 {
                Self::MAX
            } else {
                Self::MIN
            }
        } else {
            self.checked_div(rhs).unwrap_or(Self::ZERO)
        }
    }
}

impl DivAssign for SimScalar {
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}

/// Exact integer floor square root for `u128` (`floor(sqrt(n))`).
#[inline]
pub const fn isqrt_u128(n: u128) -> u64 {
    if n == 0 {
        return 0;
    }
    let shift = (127 - n.leading_zeros()) & !1;
    let mut bit = 1_u128 << shift;
    let mut rem = n;
    let mut res = 0_u128;
    while bit != 0 {
        let cand = res + bit;
        res >>= 1;
        if rem >= cand {
            rem -= cand;
            res += bit;
        }
        bit >>= 2;
    }
    res as u64
}

/// Quantizes an `f32` boundary value into deterministic `SimScalar` (`I32F32`).
#[inline]
pub const fn quantize_f32_to_sim(v: f32) -> SimScalar {
    SimScalar::from_f32(v)
}

/// Canonicalizes an `f32` value through `SimScalar` (`I32F32`) roundtrip (`NaN/-0.0` cleaned, exact `I32F32` grid).
#[inline]
pub const fn canonicalize_f32_sim(v: f32) -> f32 {
    SimScalar::from_f32(v).to_f32()
}

/// Canonicalizes a 3D position through `SimScalar` (`I32F32`) roundtrip.
#[inline]
pub const fn canonicalize_pos3_sim(p: [f32; 3]) -> [f32; 3] {
    [
        canonicalize_f32_sim(p[0]),
        canonicalize_f32_sim(p[1]),
        canonicalize_f32_sim(p[2]),
    ]
}

/// Canonicalizes an `f32` value to deterministic `I32F16` (`1 / 65536` grid, ~0.015 mm precision)
/// for state-hash and cross-build canonicalization so minor float rounding never diverges.
#[inline]
pub const fn quantize_f32_canonical(v: f32) -> i64 {
    let s = SimScalar::from_f32(v);
    let rounded = if s.0 >= 0 {
        s.0.saturating_add(1_i64 << 15)
    } else {
        s.0.saturating_sub(1_i64 << 15)
    };
    rounded >> 16
}

/// Deterministic square root on `SimScalar`.
#[inline]
pub const fn det_sqrt(v: SimScalar) -> SimScalar {
    v.det_sqrt()
}

/// Deterministic `f32` square root backed by `SimScalar` integer `isqrt`.
#[inline]
pub const fn det_sqrt_f32(v: f32) -> f32 {
    SimScalar::from_f32(v).det_sqrt().to_f32()
}

/// Deterministic 2D hypotenuse `sqrt(dx^2 + dz^2)` using full 128-bit intermediate precision.
#[inline]
pub const fn det_hypot_scalar(dx: SimScalar, dz: SimScalar) -> SimScalar {
    let dx128 = dx.0 as i128;
    let dz128 = dz.0 as i128;
    let sum_sq = (dx128 * dx128).saturating_add(dz128 * dz128) as u128;
    let root = isqrt_u128(sum_sq);
    if root > i64::MAX as u64 {
        SimScalar::MAX
    } else {
        SimScalar(root as i64)
    }
}

/// Deterministic XZ-plane distance between four `SimScalar` coordinates `(ax, az)` and `(bx, bz)`.
#[inline]
pub const fn det_distance_xz_scalars(
    ax: SimScalar,
    az: SimScalar,
    bx: SimScalar,
    bz: SimScalar,
) -> SimScalar {
    det_hypot_scalar(ax.saturating_sub(bx), az.saturating_sub(bz))
}

/// Deterministic XZ-plane distance between two 3D world positions in `SimScalar`.
#[inline]
pub const fn det_distance_xz(a: [f32; 3], b: [f32; 3]) -> SimScalar {
    let dx = SimScalar::from_f32(a[0]).saturating_sub(SimScalar::from_f32(b[0]));
    let dz = SimScalar::from_f32(a[2]).saturating_sub(SimScalar::from_f32(b[2]));
    det_hypot_scalar(dx, dz)
}

/// Deterministic XZ-plane distance between two 3D world positions as `f32`.
#[inline]
pub const fn det_distance_xz_f32(a: [f32; 3], b: [f32; 3]) -> f32 {
    det_distance_xz(a, b).to_f32()
}

/// Deterministic 3D Euclidean distance between two world positions in `SimScalar`.
#[inline]
pub const fn det_distance_3d(a: [f32; 3], b: [f32; 3]) -> SimScalar {
    let dx = SimScalar::from_f32(a[0])
        .saturating_sub(SimScalar::from_f32(b[0]))
        .0 as i128;
    let dy = SimScalar::from_f32(a[1])
        .saturating_sub(SimScalar::from_f32(b[1]))
        .0 as i128;
    let dz = SimScalar::from_f32(a[2])
        .saturating_sub(SimScalar::from_f32(b[2]))
        .0 as i128;
    let sum_sq = (dx * dx).saturating_add(dy * dy).saturating_add(dz * dz) as u128;
    let root = isqrt_u128(sum_sq);
    if root > i64::MAX as u64 {
        SimScalar::MAX
    } else {
        SimScalar(root as i64)
    }
}

/// Wraps a `SimScalar` radian angle into `[-PI, PI]` using `Q60` integer remainder.
#[inline]
pub const fn det_wrap_pi_scalar(x: SimScalar) -> SimScalar {
    // 2 * pi * 2^60 = 7_244_019_458_077_122_842
    const TAU_Q60: i128 = 7_244_019_458_077_122_842;
    const PI_Q60: i128 = 3_622_009_729_038_561_421;

    let x_q60 = (x.0 as i128) << 28;
    let mut r = x_q60 % TAU_Q60;
    if r > PI_Q60 {
        r -= TAU_Q60;
    } else if r < -PI_Q60 {
        r += TAU_Q60;
    }
    let rounded = if r >= 0 {
        r + (1_i128 << 27)
    } else {
        r - (1_i128 << 27)
    };
    SimScalar((rounded >> 28) as i64)
}

/// Evaluates `sin(u)` for `u` in `[-PI/2, PI/2]` via 11th-degree fixed-point Horner polynomial.
#[inline]
const fn sin_half_pi_scalar(u: SimScalar) -> SimScalar {
    const C3: SimScalar = SimScalar(-715_827_883); // -1 / 3!
    const C5: SimScalar = SimScalar(35_791_394); // +1 / 5!
    const C7: SimScalar = SimScalar(-852_176); // -1 / 7!
    const C9: SimScalar = SimScalar(11_836); // +1 / 9!
    const C11: SimScalar = SimScalar(-108); // -1 / 11!

    let u2 = u.saturating_mul(u);
    let p = C3.saturating_add(u2.saturating_mul(C5.saturating_add(u2.saturating_mul(
        C7.saturating_add(u2.saturating_mul(C9.saturating_add(u2.saturating_mul(C11)))),
    ))));
    let term = u.saturating_mul(u2.saturating_mul(p));
    u.saturating_add(term)
        .clamp(SimScalar::NEG_ONE, SimScalar::ONE)
}

/// Deterministic `(sin(x), cos(x))` pair in `SimScalar` (`I32F32`).
#[inline]
pub const fn det_sin_cos_scalar(x: SimScalar) -> (SimScalar, SimScalar) {
    let r = det_wrap_pi_scalar(x);
    let sin_arg = if r.0 > SimScalar::FRAC_PI_2.0 {
        SimScalar::PI.saturating_sub(r)
    } else if r.0 < -SimScalar::FRAC_PI_2.0 {
        SimScalar(-SimScalar::PI.0).saturating_sub(r)
    } else {
        r
    };
    let cos_arg = SimScalar::FRAC_PI_2.saturating_sub(r.abs());
    (sin_half_pi_scalar(sin_arg), sin_half_pi_scalar(cos_arg))
}

/// Deterministic `atan2(y, x)` in `SimScalar` (`[-PI, PI]`), returning `0` on `(0, 0)`.
#[inline]
pub const fn det_atan2_scalar(y: SimScalar, x: SimScalar) -> SimScalar {
    if x.0 == 0 && y.0 == 0 {
        return SimScalar::ZERO;
    }
    let ax = x.0.unsigned_abs() as u128;
    let ay = y.0.unsigned_abs() as u128;

    let min_v = if ay <= ax { ay } else { ax };
    let max_v = if ay <= ax { ax } else { ay };
    let r = isqrt_u128((ax * ax).saturating_add(ay * ay)) as u128;
    let denom = max_v + r;
    let w = match (min_v << 32).checked_div(denom) {
        Some(q) => SimScalar(q as i64),
        None => SimScalar::ZERO,
    };

    // Half-angle Taylor series on w in [0, sqrt(2) - 1]:
    // 2 * w * (1 - w^2/3 + w^4/5 - w^6/7 + w^8/9 - w^10/11)
    const K3: SimScalar = SimScalar(-1_431_655_765); // -1/3
    const K5: SimScalar = SimScalar(858_993_459); // +1/5
    const K7: SimScalar = SimScalar(-613_566_757); // -1/7
    const K9: SimScalar = SimScalar(477_218_588); // +1/9
    const K11: SimScalar = SimScalar(-390_451_572); // -1/11

    let w2 = w.saturating_mul(w);
    let poly = K3.saturating_add(w2.saturating_mul(K5.saturating_add(w2.saturating_mul(
        K7.saturating_add(w2.saturating_mul(K9.saturating_add(w2.saturating_mul(K11)))),
    ))));
    let half_ang = w.saturating_add(w.saturating_mul(w2.saturating_mul(poly)));
    let mut ang = half_ang.saturating_add(half_ang);

    if ay > ax {
        ang = SimScalar::FRAC_PI_2.saturating_sub(ang);
    }
    if x.0 < 0 {
        ang = SimScalar::PI.saturating_sub(ang);
    }
    if y.0 < 0 {
        ang = SimScalar(-ang.0);
    }
    ang
}

/// Reduces an arbitrary radian angle `x` into `[-PI, PI]` deterministically via `SimScalar`.
#[inline]
pub fn det_wrap_pi(x: f32) -> f32 {
    if !x.is_finite() || x.abs() > 1.0e9 {
        return 0.0;
    }
    det_wrap_pi_scalar(SimScalar::from_f32(x)).to_f32()
}

/// Deterministic `sin(x)` for lockstep simulation (`SimScalar` fixed-point backed).
#[inline]
pub fn det_sin(x: f32) -> f32 {
    if !x.is_finite() || x.abs() > 1.0e9 {
        return 0.0;
    }
    det_sin_cos_scalar(SimScalar::from_f32(x)).0.to_f32()
}

/// Deterministic `cos(x)` for lockstep simulation (`SimScalar` fixed-point backed).
#[inline]
pub fn det_cos(x: f32) -> f32 {
    if !x.is_finite() || x.abs() > 1.0e9 {
        return 1.0;
    }
    det_sin_cos_scalar(SimScalar::from_f32(x)).1.to_f32()
}

/// Deterministic `(sin(x), cos(x))` pair sharing a single fixed-point range reduction.
#[inline]
pub fn det_sin_cos(x: f32) -> (f32, f32) {
    if !x.is_finite() || x.abs() > 1.0e9 {
        return (0.0, 1.0);
    }
    let (s, c) = det_sin_cos_scalar(SimScalar::from_f32(x));
    (s.to_f32(), c.to_f32())
}

/// Deterministic `atan2(y, x)` in `[-PI, PI]` (`SimScalar` fixed-point backed).
#[inline]
pub fn det_atan2(y: f32, x: f32) -> f32 {
    if !y.is_finite() || !x.is_finite() {
        return 0.0;
    }
    det_atan2_scalar(SimScalar::from_f32(y), SimScalar::from_f32(x)).to_f32()
}

/// Deterministic 8-direction unit vectors `(cos(k * PI/4), sin(k * PI/4))` for `k in 0..8`.
pub const OCTANT_UNIT_DIRS: [[f32; 2]; 8] = [
    [1.0, 0.0],
    [
        std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
    ],
    [0.0, 1.0],
    [
        -std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
    ],
    [-1.0, 0.0],
    [
        -std::f32::consts::FRAC_1_SQRT_2,
        -std::f32::consts::FRAC_1_SQRT_2,
    ],
    [0.0, -1.0],
    [
        std::f32::consts::FRAC_1_SQRT_2,
        -std::f32::consts::FRAC_1_SQRT_2,
    ],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sim_scalar_roundtrip_and_arithmetic_exactness() {
        for &v in &[
            0.0_f32,
            1.0,
            -1.0,
            0.05,
            0.5,
            -123.75,
            2048.125,
            0.003_906_25,
        ] {
            let s = SimScalar::from_f32(v);
            assert_eq!(
                s.to_f32().to_bits(),
                v.to_bits(),
                "exact IEEE-754 roundtrip failed for {v}"
            );
        }
        // Sub-millimeter values (< 2^-9) quantize to I32F32 (`2^-32 = 2.33e-10`) precision
        let tiny = SimScalar::from_f32(1.0e-5).to_f32();
        assert!((tiny - 1.0e-5).abs() < 3.0e-10);
        let a = SimScalar::from_i32(3);
        let b = SimScalar::from_i32(4);
        assert_eq!(det_hypot_scalar(a, b), SimScalar::from_i32(5));
        assert_eq!((a + b) * SimScalar::HALF, SimScalar::from_f32(3.5));
        assert_eq!(SimScalar::from_i32(9).det_sqrt(), SimScalar::from_i32(3));
        assert_eq!(SimScalar::from_f32(f32::NAN), SimScalar::ZERO);
    }

    #[test]
    fn det_trig_matches_reference_across_domain() {
        for i in -2000..=2000 {
            let x = (i as f32) * 0.037;
            let (s, c) = det_sin_cos(x);
            assert!(
                (s - x.sin()).abs() < 2.0e-5,
                "sin mismatch at {x}: {s} vs {}",
                x.sin()
            );
            assert!(
                (c - x.cos()).abs() < 2.0e-5,
                "cos mismatch at {x}: {c} vs {}",
                x.cos()
            );
            assert_eq!(s.to_bits(), det_sin(x).to_bits());
            assert_eq!(c.to_bits(), det_cos(x).to_bits());
        }
        assert_eq!(det_sin(f32::NAN), 0.0);
        assert_eq!(det_cos(f32::INFINITY), 1.0);
    }

    #[test]
    fn det_atan2_matches_all_quadrants_and_axes() {
        for iy in -25..=25 {
            for ix in -25..=25 {
                if ix == 0 && iy == 0 {
                    assert_eq!(det_atan2(0.0, 0.0), 0.0);
                    continue;
                }
                let y = iy as f32 * 0.7;
                let x = ix as f32 * 0.7;
                let got = det_atan2(y, x);
                let exp = y.atan2(x);
                let diff = det_wrap_pi(got - exp).abs();
                assert!(
                    diff < 1.0e-5,
                    "atan2({y}, {x}): got {got}, exp {exp}, diff {diff}"
                );
            }
        }
        assert_eq!(det_atan2(f32::NAN, 1.0), 0.0);
        assert_eq!(det_atan2(1.0, f32::NEG_INFINITY), 0.0);
    }
}
