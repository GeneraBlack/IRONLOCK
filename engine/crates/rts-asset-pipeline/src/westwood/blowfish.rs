use anyhow::{bail, Result};
use std::sync::OnceLock;

/// Westwood's global RSA public modulus (`e = 65537`) embedded in RA1/TS/RA2/YR executables
/// (DER ASN.1 INTEGER Base64: `"AihRvNoIbTn85FZRYNZRcT+i6KpU+maCsEqr3Q5q+LDB5tH7Tz2qQ38V"`).
pub const WESTWOOD_MIX_PUBLIC_MODULUS_B64: &str =
    "AihRvNoIbTn85FZRYNZRcT+i6KpU+maCsEqr3Q5q+LDB5tH7Tz2qQ38V";

/// Westwood's corresponding RSA private exponent from Westwood's legacy `keys.ini`
/// (used for deterministic roundtrip encryption tests of `key_source` blocks).
pub const WESTWOOD_MIX_PRIVATE_EXPONENT_B64: &str =
    "AigKVje8mROcR8QixnxUEF5b29Curkq01DNDWCdOG99XBqH79OaCiTCB";

struct BlowfishPiTables {
    p: [u32; 18],
    s: [[u32; 256]; 4],
}

static PI_TABLES: OnceLock<BlowfishPiTables> = OnceLock::new();

fn div_big_fixed_small(buf: &mut [u32], start: usize, divisor: u64) -> bool {
    let mut rem: u64 = 0;
    let mut any_nonzero = false;
    for slot in &mut buf[start..] {
        let cur = (rem << 32) | (*slot as u64);
        let q = (cur / divisor) as u32;
        rem = cur % divisor;
        *slot = q;
        if q != 0 {
            any_nonzero = true;
        }
    }
    any_nonzero
}

fn add_big_fixed(acc: &mut [u32], term: &[u32]) {
    let mut carry: u64 = 0;
    for i in (0..acc.len()).rev() {
        let s = (acc[i] as u64) + (term[i] as u64) + carry;
        acc[i] = s as u32;
        carry = s >> 32;
    }
}

fn sub_big_fixed(acc: &mut [u32], term: &[u32]) {
    let mut borrow: i64 = 0;
    for i in (0..acc.len()).rev() {
        let d = (acc[i] as i64) - (term[i] as i64) - borrow;
        if d < 0 {
            acc[i] = (d + (1i64 << 32)) as u32;
            borrow = 1;
        } else {
            acc[i] = d as u32;
            borrow = 0;
        }
    }
}

/// Computes `scale * arctan(1 / inv_x)` into `acc` (either adding or subtracting) using fixed-point words.
fn accumulate_atan_recip(acc: &mut [u32], scale: u32, inv_x: u64, subtract_overall: bool) {
    let len = acc.len();
    let mut power = vec![0u32; len];
    let mut term = vec![0u32; len];
    power[0] = scale;
    div_big_fixed_small(&mut power, 0, inv_x);

    let step_div = inv_x * inv_x;
    let mut first_nz = 0usize;
    let mut k: u64 = 0;

    loop {
        while first_nz < len && power[first_nz] == 0 {
            first_nz += 1;
        }
        if first_nz >= len {
            break;
        }

        term[..first_nz].fill(0);
        term[first_nz..].copy_from_slice(&power[first_nz..]);
        let denom = 2 * k + 1;
        if !div_big_fixed_small(&mut term, first_nz, denom) {
            break;
        }

        let is_negative_term = (k & 1) == 1;
        if is_negative_term ^ subtract_overall {
            sub_big_fixed(acc, &term);
        } else {
            add_big_fixed(acc, &term);
        }

        div_big_fixed_small(&mut power, first_nz, step_div);
        k += 1;
    }
}

fn init_pi_tables() -> BlowfishPiTables {
    // 1 integer word + 1042 fractional words (18 P + 1024 S) + 6 guard words (192 guard bits)
    const FRAC_WORDS: usize = 18 + 4 * 256;
    const GUARD_WORDS: usize = 6;
    const TOTAL_WORDS: usize = 1 + FRAC_WORDS + GUARD_WORDS;

    let mut pi = vec![0u32; TOTAL_WORDS];
    // Machin's identity: pi = 16 * arctan(1/5) - 4 * arctan(1/239)
    accumulate_atan_recip(&mut pi, 16, 5, false);
    accumulate_atan_recip(&mut pi, 4, 239, true);
    debug_assert_eq!(pi[0], 3);

    let mut p = [0u32; 18];
    p.copy_from_slice(&pi[1..19]);

    let mut s = [[0u32; 256]; 4];
    for (box_idx, s_box) in s.iter_mut().enumerate() {
        let base = 19 + box_idx * 256;
        s_box.copy_from_slice(&pi[base..base + 256]);
    }

    BlowfishPiTables { p, s }
}

/// Pure-Rust implementation of Bruce Schneier's Blowfish block cipher (`4..=448`-bit keys),
/// supporting both big-endian standard ECB blocks and Westwood x86 little-endian `.mix` ECB blocks.
#[derive(Clone)]
pub struct BlowfishCipher {
    p: [u32; 18],
    s: [[u32; 256]; 4],
}

impl BlowfishCipher {
    /// Initializes a Blowfish key schedule from a `1..=56`-byte key.
    pub fn new(key: &[u8]) -> Result<Self> {
        if key.is_empty() || key.len() > 56 {
            bail!(
                "Blowfish key length must be in 1..=56 bytes (got {})",
                key.len()
            );
        }
        let init = PI_TABLES.get_or_init(init_pi_tables);
        let mut cipher = Self {
            p: init.p,
            s: init.s,
        };

        let mut key_pos = 0usize;
        for p_word in &mut cipher.p {
            let mut k_word = 0u32;
            for _ in 0..4 {
                k_word = (k_word << 8) | (key[key_pos] as u32);
                key_pos = (key_pos + 1) % key.len();
            }
            *p_word ^= k_word;
        }

        let mut xl = 0u32;
        let mut xr = 0u32;
        for i in (0..18).step_by(2) {
            (xl, xr) = cipher.encipher_words(xl, xr);
            cipher.p[i] = xl;
            cipher.p[i + 1] = xr;
        }
        for box_idx in 0..4 {
            for i in (0..256).step_by(2) {
                (xl, xr) = cipher.encipher_words(xl, xr);
                cipher.s[box_idx][i] = xl;
                cipher.s[box_idx][i + 1] = xr;
            }
        }

        Ok(cipher)
    }

    #[inline]
    fn f(&self, x: u32) -> u32 {
        let a = ((x >> 24) & 0xFF) as usize;
        let b = ((x >> 16) & 0xFF) as usize;
        let c = ((x >> 8) & 0xFF) as usize;
        let d = (x & 0xFF) as usize;
        let y = self.s[0][a].wrapping_add(self.s[1][b]);
        (y ^ self.s[2][c]).wrapping_add(self.s[3][d])
    }

    #[inline]
    pub fn encipher_words(&self, mut xl: u32, mut xr: u32) -> (u32, u32) {
        for i in 0..16 {
            xl ^= self.p[i];
            xr ^= self.f(xl);
            std::mem::swap(&mut xl, &mut xr);
        }
        std::mem::swap(&mut xl, &mut xr);
        xr ^= self.p[16];
        xl ^= self.p[17];
        (xl, xr)
    }

    #[inline]
    pub fn decipher_words(&self, mut xl: u32, mut xr: u32) -> (u32, u32) {
        for i in (2..18).rev() {
            xl ^= self.p[i];
            xr ^= self.f(xl);
            std::mem::swap(&mut xl, &mut xr);
        }
        std::mem::swap(&mut xl, &mut xr);
        xr ^= self.p[1];
        xl ^= self.p[0];
        (xl, xr)
    }

    /// Standard big-endian 8-byte block encryption (Schneier specification).
    pub fn encrypt_block_be(&self, block: &mut [u8; 8]) {
        let xl = u32::from_be_bytes([block[0], block[1], block[2], block[3]]);
        let xr = u32::from_be_bytes([block[4], block[5], block[6], block[7]]);
        let (ol, or) = self.encipher_words(xl, xr);
        block[0..4].copy_from_slice(&ol.to_be_bytes());
        block[4..8].copy_from_slice(&or.to_be_bytes());
    }

    /// Standard big-endian 8-byte block decryption (Schneier specification).
    pub fn decrypt_block_be(&self, block: &mut [u8; 8]) {
        let xl = u32::from_be_bytes([block[0], block[1], block[2], block[3]]);
        let xr = u32::from_be_bytes([block[4], block[5], block[6], block[7]]);
        let (ol, or) = self.decipher_words(xl, xr);
        block[0..4].copy_from_slice(&ol.to_be_bytes());
        block[4..8].copy_from_slice(&or.to_be_bytes());
    }

    /// Westwood `.mix` (`CBlowfish`) x86 little-endian 8-byte block encryption.
    pub fn encrypt_block_le(&self, block: &mut [u8; 8]) {
        let xl = u32::from_le_bytes([block[0], block[1], block[2], block[3]]);
        let xr = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
        let (ol, or) = self.encipher_words(xl, xr);
        block[0..4].copy_from_slice(&ol.to_le_bytes());
        block[4..8].copy_from_slice(&or.to_le_bytes());
    }

    /// Westwood `.mix` (`CBlowfish`) x86 little-endian 8-byte block decryption.
    pub fn decrypt_block_le(&self, block: &mut [u8; 8]) {
        let xl = u32::from_le_bytes([block[0], block[1], block[2], block[3]]);
        let xr = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
        let (ol, or) = self.decipher_words(xl, xr);
        block[0..4].copy_from_slice(&ol.to_le_bytes());
        block[4..8].copy_from_slice(&or.to_le_bytes());
    }

    /// Encrypts an 8-byte-aligned buffer in-place using Westwood `.mix` ECB mode (`u32` LE words).
    pub fn encrypt_westwood_ecb(&self, buf: &mut [u8]) -> Result<()> {
        if !buf.len().is_multiple_of(8) {
            bail!("Blowfish ECB buffer length must be a multiple of 8 bytes");
        }
        for chunk in buf.chunks_exact_mut(8) {
            let mut blk: [u8; 8] = chunk.try_into().unwrap();
            self.encrypt_block_le(&mut blk);
            chunk.copy_from_slice(&blk);
        }
        Ok(())
    }

    /// Decrypts an 8-byte-aligned buffer in-place using Westwood `.mix` ECB mode (`u32` LE words).
    pub fn decrypt_westwood_ecb(&self, buf: &mut [u8]) -> Result<()> {
        if !buf.len().is_multiple_of(8) {
            bail!("Blowfish ECB buffer length must be a multiple of 8 bytes");
        }
        for chunk in buf.chunks_exact_mut(8) {
            let mut blk: [u8; 8] = chunk.try_into().unwrap();
            self.decrypt_block_le(&mut blk);
            chunk.copy_from_slice(&blk);
        }
        Ok(())
    }
}

fn decode_base64_simple(b64: &str) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(b64.len() * 3 / 4);
    let mut val = 0u32;
    let mut bits = 0u32;
    for ch in b64.bytes() {
        let sextet = match ch {
            b'A'..=b'Z' => (ch - b'A') as u32,
            b'a'..=b'z' => (ch - b'a' + 26) as u32,
            b'0'..=b'9' => (ch - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b' ' | b'\r' | b'\n' | b'\t' => continue,
            other => bail!("Invalid Base64 byte 0x{other:02X}"),
        };
        val = (val << 6) | sextet;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((val >> bits) & 0xFF) as u8);
        }
    }
    Ok(out)
}

/// 11-limb (`352`-bit) little-endian unsigned integer for Westwood 320-bit RSA `key_source` math.
const LIMBS: usize = 11;
const PROD_LIMBS: usize = LIMBS * 2;

fn limbs_from_le_bytes(bytes: &[u8]) -> [u32; LIMBS] {
    let mut out = [0u32; LIMBS];
    for (i, &b) in bytes.iter().take(LIMBS * 4).enumerate() {
        out[i / 4] |= (b as u32) << ((i % 4) * 8);
    }
    out
}

fn limbs_from_der_asn1_b64(b64: &str) -> Result<[u32; LIMBS]> {
    let raw = decode_base64_simple(b64)?;
    if raw.len() < 3 || raw[0] != 0x02 {
        bail!("Invalid Westwood ASN.1 INTEGER key blob");
    }
    let int_len = raw[1] as usize;
    if raw.len() < 2 + int_len {
        bail!("Truncated Westwood ASN.1 INTEGER key blob");
    }
    // ASN.1 INTEGER is big-endian; reverse into little-endian bytes
    let mut le_bytes: Vec<u8> = raw[2..2 + int_len].iter().copied().rev().collect();
    while le_bytes.last() == Some(&0) && le_bytes.len() > 1 {
        le_bytes.pop();
    }
    Ok(limbs_from_le_bytes(&le_bytes))
}

fn limbs_to_le_bytes(val: &[u32; LIMBS], out_len: usize) -> Vec<u8> {
    let mut out = vec![0u8; out_len];
    for (i, slot) in out.iter_mut().enumerate() {
        if i / 4 < LIMBS {
            *slot = ((val[i / 4] >> ((i % 4) * 8)) & 0xFF) as u8;
        }
    }
    out
}

fn cmp_slice_be(a: &[u32], b: &[u32]) -> std::cmp::Ordering {
    for i in (0..a.len()).rev() {
        match a[i].cmp(&b[i]) {
            std::cmp::Ordering::Equal => continue,
            non_eq => return non_eq,
        }
    }
    std::cmp::Ordering::Equal
}

fn bit_len_prod(a: &[u32; PROD_LIMBS]) -> usize {
    for i in (0..PROD_LIMBS).rev() {
        if a[i] != 0 {
            return i * 32 + (32 - a[i].leading_zeros() as usize);
        }
    }
    0
}

fn shl_prod(a: &[u32; PROD_LIMBS], shift_bits: usize) -> [u32; PROD_LIMBS] {
    let mut out = [0u32; PROD_LIMBS];
    let word_shift = shift_bits / 32;
    let bit_shift = shift_bits % 32;
    for i in 0..PROD_LIMBS {
        if i + word_shift < PROD_LIMBS {
            out[i + word_shift] |= a[i] << bit_shift;
        }
        if bit_shift > 0 && i + word_shift + 1 < PROD_LIMBS {
            out[i + word_shift + 1] |= a[i] >> (32 - bit_shift);
        }
    }
    out
}

fn sub_assign_prod(a: &mut [u32; PROD_LIMBS], b: &[u32; PROD_LIMBS]) {
    let mut borrow: i64 = 0;
    for i in 0..PROD_LIMBS {
        let d = (a[i] as i64) - (b[i] as i64) - borrow;
        if d < 0 {
            a[i] = (d + (1i64 << 32)) as u32;
            borrow = 1;
        } else {
            a[i] = d as u32;
            borrow = 0;
        }
    }
}

fn mul_mod_limbs(a: &[u32; LIMBS], b: &[u32; LIMBS], modulus: &[u32; LIMBS]) -> [u32; LIMBS] {
    let mut prod = [0u32; PROD_LIMBS];
    for i in 0..LIMBS {
        let mut carry: u64 = 0;
        for j in 0..LIMBS {
            let acc = (prod[i + j] as u64) + (a[i] as u64) * (b[j] as u64) + carry;
            prod[i + j] = acc as u32;
            carry = acc >> 32;
        }
        prod[i + LIMBS] = prod[i + LIMBS].wrapping_add(carry as u32);
    }

    let mut mod_ext = [0u32; PROD_LIMBS];
    mod_ext[..LIMBS].copy_from_slice(modulus);
    let mod_bits = bit_len_prod(&mod_ext);
    if mod_bits == 0 {
        return [0u32; LIMBS];
    }

    let mut cur_bits = bit_len_prod(&prod);
    while cur_bits >= mod_bits {
        let shift = cur_bits - mod_bits;
        let mut shifted = shl_prod(&mod_ext, shift);
        if cmp_slice_be(&prod, &shifted) == std::cmp::Ordering::Less {
            if shift == 0 {
                break;
            }
            shifted = shl_prod(&mod_ext, shift - 1);
        }
        sub_assign_prod(&mut prod, &shifted);
        cur_bits = bit_len_prod(&prod);
    }

    let mut rem = [0u32; LIMBS];
    rem.copy_from_slice(&prod[..LIMBS]);
    rem
}

fn mod_pow_limbs(
    mut base: [u32; LIMBS],
    exp: &[u32; LIMBS],
    modulus: &[u32; LIMBS],
) -> [u32; LIMBS] {
    let mut result = [0u32; LIMBS];
    result[0] = 1;

    let mut top_bit = 0usize;
    for i in (0..LIMBS).rev() {
        if exp[i] != 0 {
            top_bit = i * 32 + (32 - exp[i].leading_zeros() as usize);
            break;
        }
    }

    for bit_idx in 0..top_bit {
        if ((exp[bit_idx / 32] >> (bit_idx % 32)) & 1) != 0 {
            result = mul_mod_limbs(&result, &base, modulus);
        }
        if bit_idx + 1 < top_bit {
            base = mul_mod_limbs(&base, &base, modulus);
        }
    }
    result
}

/// Decodes Westwood's 80-byte RSA `key_source` header block (offset `4..84` of an encrypted `.mix`)
/// into the 56-byte Blowfish session key using Westwood's public modulus and exponent `65537`.
pub fn decode_westwood_mix_key_block(key_source: &[u8; 80]) -> Result<[u8; 56]> {
    let modulus = limbs_from_der_asn1_b64(WESTWOOD_MIX_PUBLIC_MODULUS_B64)?;
    let mut pub_exp = [0u32; LIMBS];
    pub_exp[0] = 0x0001_0001; // 65537

    let c0 = limbs_from_le_bytes(&key_source[0..40]);
    let c1 = limbs_from_le_bytes(&key_source[40..80]);

    let m0 = mod_pow_limbs(c0, &pub_exp, &modulus);
    let m1 = mod_pow_limbs(c1, &pub_exp, &modulus);

    let mut plain = Vec::with_capacity(78);
    plain.extend_from_slice(&limbs_to_le_bytes(&m0, 39));
    plain.extend_from_slice(&limbs_to_le_bytes(&m1, 39));

    let mut key = [0u8; 56];
    key.copy_from_slice(&plain[0..56]);
    Ok(key)
}

/// Encodes a 56-byte Blowfish session key into Westwood's 80-byte RSA `key_source` block
/// using Westwood's legacy private exponent (useful for creating valid encrypted `.mix` test archives).
pub fn encode_westwood_mix_key_block(blowfish_key: &[u8; 56]) -> Result<[u8; 80]> {
    let modulus = limbs_from_der_asn1_b64(WESTWOOD_MIX_PUBLIC_MODULUS_B64)?;
    let priv_exp = limbs_from_der_asn1_b64(WESTWOOD_MIX_PRIVATE_EXPONENT_B64)?;

    let mut plain = [0u8; 78];
    plain[0..56].copy_from_slice(blowfish_key);

    let m0 = limbs_from_le_bytes(&plain[0..39]);
    let m1 = limbs_from_le_bytes(&plain[39..78]);

    let c0 = mod_pow_limbs(m0, &priv_exp, &modulus);
    let c1 = mod_pow_limbs(m1, &priv_exp, &modulus);

    let mut key_source = [0u8; 80];
    key_source[0..40].copy_from_slice(&limbs_to_le_bytes(&c0, 40));
    key_source[40..80].copy_from_slice(&limbs_to_le_bytes(&c1, 40));
    Ok(key_source)
}
