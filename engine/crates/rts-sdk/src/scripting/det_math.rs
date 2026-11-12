//! Pure integer / `I32F32` fixed-point deterministic math functions for `.rhai` and `.lua` scripts.
//!
//! Host `libm` functions (`f64::sin`, `f64::cos`, `f64::atan2`, `f64::sqrt`) can differ by 1+ ULP
//! across x86_64 AVX2, x86_64 SSE2, and ARM64 targets. Every function in this module converts
//! IEEE-754 `f64` bit patterns into signed 64-bit `I32F32` fixed-point integers (`i128`/`u128`
//! intermediates) and reconstructs IEEE-754 `f64` bit patterns using pure integer bit operations,
//! guaranteeing 100% bit-identical script execution across all architectures and build profiles,
//! as well as 100% bit-identical parity between Rhai and Lua scripts.

const FRAC_BITS: u32 = 32;
const SCALE: i64 = 1_i64 << FRAC_BITS;
const NEG_ONE: i64 = -SCALE;
const HALF_PI: i64 = 6_746_518_852;
const PI_FIXED: i64 = 13_493_037_705;
const TAU_FIXED: i64 = 26_986_075_409;

/// Deterministic fixed-point `PI` represented as exact IEEE-754 `f64`.
pub const DET_PI: f64 = fixed_to_f64(PI_FIXED);
/// Deterministic fixed-point `TAU` (`2 * PI`) represented as exact IEEE-754 `f64`.
pub const DET_TAU: f64 = fixed_to_f64(TAU_FIXED);

#[inline]
const fn is_non_finite_f64(v: f64) -> bool {
    ((v.to_bits() >> 52) & 0x7FF) == 0x7FF
}

/// Pure integer IEEE-754 `f64 -> I32F32` (`i64`) conversion with round-to-nearest-even.
#[inline]
pub const fn f64_to_fixed(v: f64) -> i64 {
    let bits = v.to_bits();
    let neg = (bits >> 63) != 0;
    let exp = ((bits >> 52) & 0x7FF) as i32;
    let mant = bits & 0x000F_FFFF_FFFF_FFFF;

    if exp == 0x7FF {
        if mant != 0 {
            return 0;
        }
        return if neg { i64::MIN } else { i64::MAX };
    }
    if exp == 0 && mant == 0 {
        return 0;
    }

    let (sig, unbiased_shift): (u128, i32) = if exp == 0 {
        (mant as u128, 1 - 1023 - 52 + (FRAC_BITS as i32))
    } else {
        (
            ((1_u64 << 52) | mant) as u128,
            exp - 1023 - 52 + (FRAC_BITS as i32),
        )
    };

    if unbiased_shift >= 12 {
        return if neg { i64::MIN } else { i64::MAX };
    }

    let mag_u128 = if unbiased_shift >= 0 {
        sig << (unbiased_shift as u32)
    } else {
        let rshift = (-unbiased_shift) as u32;
        if rshift >= 128 {
            0
        } else {
            let q = sig >> rshift;
            let rem = sig & ((1_u128 << rshift) - 1);
            let half = 1_u128 << (rshift - 1);
            if rem > half || (rem == half && (q & 1) == 1) {
                q + 1
            } else {
                q
            }
        }
    };

    if neg {
        let min_mag = (i64::MAX as u128) + 1;
        if mag_u128 >= min_mag {
            i64::MIN
        } else {
            -(mag_u128 as i64)
        }
    } else if mag_u128 > i64::MAX as u128 {
        i64::MAX
    } else {
        mag_u128 as i64
    }
}

/// Pure integer IEEE-754 `I32F32` (`i64`) -> `f64` conversion with round-to-nearest-even.
#[inline]
pub const fn fixed_to_f64(raw: i64) -> f64 {
    if raw == 0 {
        return f64::from_bits(0);
    }
    let neg = raw < 0;
    let mag = raw.unsigned_abs();
    let lz = mag.leading_zeros();
    let msb = 63 - lz;
    let mut biased_exp = (msb as i32) - (FRAC_BITS as i32) + 1023;

    let mant53 = if msb <= 52 {
        mag << (52 - msb)
    } else {
        let shift = msb - 52;
        let mut q = mag >> shift;
        let rem = mag & ((1_u64 << shift) - 1);
        let half = 1_u64 << (shift - 1);
        if rem > half || (rem == half && (q & 1) == 1) {
            q += 1;
            if q == (1_u64 << 53) {
                q >>= 1;
                biased_exp += 1;
            }
        }
        q
    };

    let sign_bit = if neg { 1_u64 << 63 } else { 0 };
    let bits = sign_bit | ((biased_exp as u64) << 52) | (mant53 & 0x000F_FFFF_FFFF_FFFF);
    f64::from_bits(bits)
}

#[inline]
const fn fp_mul(a: i64, b: i64) -> i64 {
    let prod = (a as i128) * (b as i128);
    let shifted = prod >> FRAC_BITS;
    if shifted > i64::MAX as i128 {
        i64::MAX
    } else if shifted < i64::MIN as i128 {
        i64::MIN
    } else {
        shifted as i64
    }
}

#[inline]
const fn fp_abs(x: i64) -> i64 {
    if x == i64::MIN {
        i64::MAX
    } else if x < 0 {
        -x
    } else {
        x
    }
}

#[inline]
const fn fp_clamp(x: i64, lo: i64, hi: i64) -> i64 {
    let actual_lo = if lo <= hi { lo } else { hi };
    let actual_hi = if lo <= hi { hi } else { lo };
    if x < actual_lo {
        actual_lo
    } else if x > actual_hi {
        actual_hi
    } else {
        x
    }
}

#[inline]
const fn isqrt_u128(n: u128) -> u64 {
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

#[inline]
const fn wrap_pi_fixed(x: i64) -> i64 {
    const TAU_Q60: i128 = 7_244_019_458_077_122_842;
    const PI_Q60: i128 = 3_622_009_729_038_561_421;

    let x_q60 = (x as i128) << 28;
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
    (rounded >> 28) as i64
}

#[inline]
const fn sin_half_pi_fixed(u: i64) -> i64 {
    const C3: i64 = -715_827_883; // -1 / 3!
    const C5: i64 = 35_791_394; // +1 / 5!
    const C7: i64 = -852_176; // -1 / 7!
    const C9: i64 = 11_836; // +1 / 9!
    const C11: i64 = -108; // -1 / 11!

    let u2 = fp_mul(u, u);
    let p9 = C9.saturating_add(fp_mul(u2, C11));
    let p7 = C7.saturating_add(fp_mul(u2, p9));
    let p5 = C5.saturating_add(fp_mul(u2, p7));
    let p3 = C3.saturating_add(fp_mul(u2, p5));
    let term = fp_mul(u, fp_mul(u2, p3));
    fp_clamp(u.saturating_add(term), NEG_ONE, SCALE)
}

#[inline]
const fn sin_cos_fixed(x: i64) -> (i64, i64) {
    let r = wrap_pi_fixed(x);
    let sin_arg = if r > HALF_PI {
        PI_FIXED.saturating_sub(r)
    } else if r < -HALF_PI {
        (-PI_FIXED).saturating_sub(r)
    } else {
        r
    };
    let cos_arg = HALF_PI.saturating_sub(fp_abs(r));
    (sin_half_pi_fixed(sin_arg), sin_half_pi_fixed(cos_arg))
}

#[inline]
const fn atan2_fixed(y: i64, x: i64) -> i64 {
    if x == 0 && y == 0 {
        return 0;
    }
    let ax = x.unsigned_abs() as u128;
    let ay = y.unsigned_abs() as u128;

    let min_v = if ay <= ax { ay } else { ax };
    let max_v = if ay <= ax { ax } else { ay };
    let r = isqrt_u128((ax * ax).saturating_add(ay * ay)) as u128;
    let denom = max_v + r;
    let w = match (min_v << 32).checked_div(denom) {
        Some(q) => q as i64,
        None => 0,
    };

    const K3: i64 = -1_431_655_765; // -1/3
    const K5: i64 = 858_993_459; // +1/5
    const K7: i64 = -613_566_757; // -1/7
    const K9: i64 = 477_218_588; // +1/9
    const K11: i64 = -390_451_572; // -1/11

    let w2 = fp_mul(w, w);
    let p9 = K9.saturating_add(fp_mul(w2, K11));
    let p7 = K7.saturating_add(fp_mul(w2, p9));
    let p5 = K5.saturating_add(fp_mul(w2, p7));
    let p3 = K3.saturating_add(fp_mul(w2, p5));
    let half_ang = w.saturating_add(fp_mul(w, fp_mul(w2, p3)));
    let mut ang = half_ang.saturating_add(half_ang);

    if ay > ax {
        ang = HALF_PI.saturating_sub(ang);
    }
    if x < 0 {
        ang = PI_FIXED.saturating_sub(ang);
    }
    if y < 0 {
        ang = -ang;
    }
    ang
}

/// Deterministic fixed-point `sin(x)` for Rhai and Lua scripts.
#[inline]
pub const fn det_sin(x: f64) -> f64 {
    if is_non_finite_f64(x) {
        return f64::from_bits(0);
    }
    fixed_to_f64(sin_cos_fixed(f64_to_fixed(x)).0)
}

/// Deterministic fixed-point `cos(x)` for Rhai and Lua scripts.
#[inline]
pub const fn det_cos(x: f64) -> f64 {
    if is_non_finite_f64(x) {
        return fixed_to_f64(SCALE);
    }
    fixed_to_f64(sin_cos_fixed(f64_to_fixed(x)).1)
}

/// Deterministic fixed-point `atan2(y, x)` in `[-PI, PI]` for Rhai and Lua scripts.
#[inline]
pub const fn det_atan2(y: f64, x: f64) -> f64 {
    if is_non_finite_f64(y) || is_non_finite_f64(x) {
        return f64::from_bits(0);
    }
    fixed_to_f64(atan2_fixed(f64_to_fixed(y), f64_to_fixed(x)))
}

/// Deterministic fixed-point `sqrt(max(x, 0))` for Rhai and Lua scripts.
#[inline]
pub const fn det_sqrt(x: f64) -> f64 {
    if is_non_finite_f64(x) {
        return f64::from_bits(0);
    }
    let fx = f64_to_fixed(x);
    if fx <= 0 {
        return f64::from_bits(0);
    }
    let radicand = (fx as u128) << FRAC_BITS;
    let root = isqrt_u128(radicand);
    let clamped = if root > i64::MAX as u64 {
        i64::MAX
    } else {
        root as i64
    };
    fixed_to_f64(clamped)
}

/// Deterministic fixed-point 2D hypotenuse `sqrt(dx^2 + dz^2)`.
#[inline]
pub const fn det_hypot(dx: f64, dz: f64) -> f64 {
    if is_non_finite_f64(dx) || is_non_finite_f64(dz) {
        return f64::from_bits(0);
    }
    let fdx = f64_to_fixed(dx) as i128;
    let fdz = f64_to_fixed(dz) as i128;
    let sum_sq = (fdx * fdx).saturating_add(fdz * fdz) as u128;
    let root = isqrt_u128(sum_sq);
    let clamped = if root > i64::MAX as u64 {
        i64::MAX
    } else {
        root as i64
    };
    fixed_to_f64(clamped)
}

/// Deterministic fixed-point 2D Euclidean distance `sqrt((x2 - x1)^2 + (z2 - z1)^2)`.
#[inline]
pub const fn det_dist(x1: f64, z1: f64, x2: f64, z2: f64) -> f64 {
    if is_non_finite_f64(x1)
        || is_non_finite_f64(z1)
        || is_non_finite_f64(x2)
        || is_non_finite_f64(z2)
    {
        return f64::from_bits(0);
    }
    let dx = (f64_to_fixed(x2) as i128) - (f64_to_fixed(x1) as i128);
    let dz = (f64_to_fixed(z2) as i128) - (f64_to_fixed(z1) as i128);
    let sum_sq = (dx.saturating_mul(dx)).saturating_add(dz.saturating_mul(dz)) as u128;
    let root = isqrt_u128(sum_sq);
    let clamped = if root > i64::MAX as u64 {
        i64::MAX
    } else {
        root as i64
    };
    fixed_to_f64(clamped)
}

/// Deterministic fixed-point clamp `val.clamp(lo, hi)`.
#[inline]
pub const fn det_clamp(val: f64, lo: f64, hi: f64) -> f64 {
    let f_val = f64_to_fixed(val);
    let f_lo = f64_to_fixed(lo);
    let f_hi = f64_to_fixed(hi);
    fixed_to_f64(fp_clamp(f_val, f_lo, f_hi))
}

/// Deterministic fixed-point absolute value `|x|`.
#[inline]
pub const fn det_abs(x: f64) -> f64 {
    if is_non_finite_f64(x) {
        return f64::from_bits(0);
    }
    fixed_to_f64(fp_abs(f64_to_fixed(x)))
}

/// Deterministic fixed-point `floor(x)` returning `i64`.
#[inline]
pub const fn det_floor(x: f64) -> i64 {
    if is_non_finite_f64(x) {
        return 0;
    }
    f64_to_fixed(x) >> FRAC_BITS
}

/// Deterministic fixed-point `ceil(x)` returning `i64`.
#[inline]
pub const fn det_ceil(x: f64) -> i64 {
    if is_non_finite_f64(x) {
        return 0;
    }
    let fx = f64_to_fixed(x);
    let base = fx >> FRAC_BITS;
    let frac = fx & ((1_i64 << FRAC_BITS) - 1);
    if frac != 0 {
        base.saturating_add(1)
    } else {
        base
    }
}

/// Deterministic fixed-point `round(x)` (half away from zero) returning `i64`.
#[inline]
pub const fn det_round(x: f64) -> i64 {
    if is_non_finite_f64(x) {
        return 0;
    }
    let fx = f64_to_fixed(x);
    let adjusted = if fx >= 0 {
        fx.saturating_add(1_i64 << 31)
    } else {
        fx.saturating_sub(1_i64 << 31)
    };
    adjusted >> FRAC_BITS
}
