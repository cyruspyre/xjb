#![allow(unused_imports, unsafe_op_in_unsafe_fn)]

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::{
    __m128i, _mm_add_epi8, _mm_add_epi64, _mm_cmpgt_epi8, _mm_cvtsi32_si128, _mm_cvtsi128_si64,
    _mm_movemask_epi8, _mm_mul_epu32, _mm_mulhi_epi16, _mm_mulhi_epu16, _mm_mullo_epi16,
    _mm_or_si128, _mm_set1_epi8, _mm_set1_epi16, _mm_set1_epi64x, _mm_setzero_si128,
    _mm_shuffle_epi32, _mm_slli_epi16, _mm_slli_epi32, _mm_srli_epi16, _mm_srli_epi64,
    _mm_storeu_si128, _mm_sub_epi16, _mm_unpacklo_epi64,
};

use core::hint::select_unpredictable;
use std::arch::x86_64::{_mm_mullo_epi32, _mm_set1_epi32, _mm_srli_epi32, _mm_sub_epi32};

#[cfg(target_endian = "big")]
const _: () = compile_error!("big-endian not supported");

const E10_DN: i32 = -6;
const E10_UP: i32 = 20;
const ZERO: u64 = 0x3030_3030_3030_3030;
const TEN: u64 = 0xA000_0000_0000_0000;

#[repr(C, align(64))]
struct DoubleTable {
    pow10_double: [u64; Self::NUM_POW10 * 2],
    exp_result_double: [u64; 633],
    e10_variable_data: [[u8; Self::MAX_DEC_SIG_LEN + 3]; 28],
    h7: [u8; 2048],
}

#[repr(C, align(64))]
struct FloatTable {
    pow10_float_reverse: [u64; Self::NUM_POW10],
    exp_result_float: [u32; 84],
    e10_variable_data: [[u8; Self::MAX_DEC_SIG_LEN + 3]; 28],
    h37: [u8; 256],
}

impl DoubleTable {
    const MAX_DEC_SIG_LEN: usize = 17;
    const NUM_POW10: usize = 617;
}

impl FloatTable {
    const MAX_DEC_SIG_LEN: usize = 9;
    const NUM_POW10: usize = 77;
}

static DOUBLE_TABLE: DoubleTable = {
    let mut tmp = DoubleTable {
        pow10_double: [0; DoubleTable::NUM_POW10 * 2],
        exp_result_double: [0; 633],
        e10_variable_data: [[0; DoubleTable::MAX_DEC_SIG_LEN + 3]; 28],
        h7: [0; 2048],
    };
    let (mut w0, mut w1, mut w2) = (
        0xB2E2_8CED_D086_D011_u64,
        0x1E53_ED49_A962_72C8_u64,
        0xCC5F_C196_FEFD_7D0C_u64,
    );

    let mut n = 0;
    while n < DoubleTable::NUM_POW10 {
        let e10 = n as i32 - 293;
        tmp.pow10_double[n * 2] = if e10 != 0 {
            w2 + (e10 >= 0 && e10 <= 27) as u64
        } else {
            1u64 << 63
        };
        tmp.pow10_double[n * 2 + 1] = w1.wrapping_add(1);

        let h0 = (w0 as u128 * TEN as u128 >> 64) as u64;
        let h1 = (w1 as u128 * TEN as u128 >> 64) as u64;
        let c0 = h0.wrapping_add(w1.wrapping_mul(TEN));
        let c1 = (c0 < h0) as u64 + h1.wrapping_add(w2.wrapping_mul(TEN));
        let c2 = (c1 < h1) as u64 + (w2 as u128 * TEN as u128 >> 64) as u64;

        if c2 >> 63 == 0 {
            (w0, w1, w2) = (c0 << 1, (c1 << 1) | (c0 >> 63), (c2 << 1) | (c1 >> 63));
        } else {
            (w0, w1, w2) = (c0, c1, c2);
        }
        n += 1;
    }

    let mut e10 = -324;
    while e10 <= 308 {
        let sign = if e10 < 0 { b'-' } else { b'+' };
        let e = b'e' as u64 + sign as u64 * 256;
        let e10_abs = if e10 < 0 { -e10 } else { e10 } as u64;
        let a = e10_abs / 100;
        let bc = e10_abs - a * 100;
        let b = bc / 10;
        let c = bc - b * 10;

        let exp_len = 3 + (e10_abs >= 100) as u64 + (e10_abs >= 10) as u64;
        let e10_abs_ascii = if e10_abs >= 100 {
            (a + b'0' as u64) + ((b + b'0' as u64) << 8) + ((c + b'0' as u64) << 16)
        } else if e10_abs >= 10 {
            (b + b'0' as u64) + ((c + b'0' as u64) << 8)
        } else {
            c + b'0' as u64
        };
        let exp_res = if e10 < E10_DN || e10 > E10_UP {
            e + (e10_abs_ascii << 16) + (exp_len << 56)
        } else {
            0
        };

        tmp.exp_result_double[(e10 + 324) as usize] = exp_res;
        e10 += 1;
    }

    let mut e10 = E10_DN;
    while e10 <= E10_UP + 1 {
        let row = (e10 - E10_DN) as usize;
        let first_sig_pos = if E10_DN <= e10 && e10 <= -1 {
            (1 - e10) as u64
        } else {
            0
        };
        let dot_pos = if 0 <= e10 && e10 <= E10_UP {
            (1 + e10) as u64
        } else {
            1
        };
        let move_pos = dot_pos + (0 <= e10 || e10 < E10_DN) as u64;

        tmp.e10_variable_data[row][DoubleTable::MAX_DEC_SIG_LEN] = first_sig_pos as u8;
        tmp.e10_variable_data[row][DoubleTable::MAX_DEC_SIG_LEN + 1] = dot_pos as u8;
        tmp.e10_variable_data[row][DoubleTable::MAX_DEC_SIG_LEN + 2] = move_pos as u8;

        let mut dec_sig_len = 1;
        while dec_sig_len <= DoubleTable::MAX_DEC_SIG_LEN {
            let exp_pos = if E10_DN <= e10 && e10 <= -1 {
                dec_sig_len as u64
            } else if 0 <= e10 && e10 <= E10_UP {
                let a = (e10 + 3) as u64;
                let b = dec_sig_len as u64 + 1;
                if a > b { a } else { b }
            } else {
                dec_sig_len as u64 + 1 - (dec_sig_len == 1) as u64
            };

            tmp.e10_variable_data[row][dec_sig_len - 1] = exp_pos as u8;
            dec_sig_len += 1;
        }

        e10 += 1;
    }

    let mut exp = 0;
    while exp < 2048 {
        let q = exp as i32 - 1075 + (exp == 0) as i32;
        let k = (q * 78913) >> 18;
        let h = q + (((-k - 1) * 217707) >> 16);

        tmp.h7[exp] = (h + 10) as u8;
        exp += 1;
    }

    tmp
};

static FLOAT_TABLE: FloatTable = {
    let mut tmp = FloatTable {
        pow10_float_reverse: [0; FloatTable::NUM_POW10],
        exp_result_float: [0; 84],
        e10_variable_data: [[0; FloatTable::MAX_DEC_SIG_LEN + 3]; 28],
        h37: [0; 256],
    };
    let (mut w0, mut w1) = (0x67DE_18ED_A581_4AF3_u64, 0xCFB1_1EAD_4539_94BA_u64);

    let mut n = 0;
    while n < FloatTable::NUM_POW10 {
        let e10 = n as i32 - 32;
        tmp.pow10_float_reverse[FloatTable::NUM_POW10 - n - 1] = if e10 == 0 {
            1u64 << 63
        } else {
            w1.wrapping_add(1)
        };

        let h0 = (w0 as u128 * TEN as u128 >> 64) as u64;
        let c0 = h0.wrapping_add(w1.wrapping_mul(TEN));
        let c1 = (c0 < h0) as u64 + (w1 as u128 * TEN as u128 >> 64) as u64;

        if c1 >> 63 == 0 {
            (w0, w1) = (c0 << 1, (c1 << 1) | (c0 >> 63));
        } else {
            (w0, w1) = (c0, c1);
        }
        n += 1;
    }

    let mut e10 = -45;
    while e10 <= 38 {
        let sign = if e10 < 0 { b'-' } else { b'+' };
        let e = b'e' as u64 + sign as u64 * 256;
        let e10_abs = if e10 < 0 { -e10 } else { e10 } as u64;
        let a = e10_abs / 10;
        let b = e10_abs - a * 10;
        let e10_abs_ascii = if a > 0 {
            (a + b'0' as u64) + ((b + b'0' as u64) << 8)
        } else {
            b + b'0' as u64
        };
        let exp_res = if e10 < E10_DN || e10 > E10_UP {
            e + (e10_abs_ascii << 16)
        } else {
            0
        };

        tmp.exp_result_float[(e10 + 45) as usize] = exp_res as u32;
        e10 += 1;
    }

    let mut e10 = E10_DN;
    while e10 <= E10_UP + 1 {
        let row = (e10 - E10_DN) as usize;
        let first_sig_pos = if E10_DN <= e10 && e10 <= -1 {
            (1 - e10) as u64
        } else {
            0
        };
        let dot_pos = if 0 <= e10 && e10 <= E10_UP {
            (1 + e10) as u64
        } else {
            1
        };
        let move_pos = dot_pos + (0 <= e10 || e10 < E10_DN) as u64;

        tmp.e10_variable_data[row][FloatTable::MAX_DEC_SIG_LEN] = first_sig_pos as u8;
        tmp.e10_variable_data[row][FloatTable::MAX_DEC_SIG_LEN + 1] = dot_pos as u8;
        tmp.e10_variable_data[row][FloatTable::MAX_DEC_SIG_LEN + 2] = move_pos as u8;

        let mut dec_sig_len = 1;
        while dec_sig_len <= FloatTable::MAX_DEC_SIG_LEN {
            let exp_pos = if E10_DN <= e10 && e10 <= -1 {
                dec_sig_len as u64
            } else if 0 <= e10 && e10 <= E10_UP {
                let a = (e10 + 3) as u64;
                let b = dec_sig_len as u64 + 1;
                if a > b { a } else { b }
            } else {
                dec_sig_len as u64 + 1 - (dec_sig_len == 1) as u64
            };

            tmp.e10_variable_data[row][dec_sig_len - 1] = exp_pos as u8;
            dec_sig_len += 1;
        }

        e10 += 1;
    }

    let mut exp = 0;
    while exp < 256 {
        let exp_bin = exp as i32 - 150 + (exp == 0) as i32;
        let k = (exp_bin * 1233) >> 12;
        let h37_precalc = (36 + 1) + exp_bin + ((k * -1701 - 1701) >> 9);

        tmp.h37[exp] = h37_precalc as u8;
        exp += 1;
    }

    tmp
};

struct ConstValueDouble {
    c3: u64,
    c4: u64,
    mul_const: u64,
    hundred_million: i64,
}

struct ConstValueFloat {
    c1: u64,
    e7: u32,
    e6: u32,
}

const CONSTANTS_DOUBLE: ConstValueDouble = ConstValueDouble {
    c3: 1_000_000_000_000_000 - 1,
    c4: (1u64 << 63) + 6,
    mul_const: 0xABCC_7711_8461_CEFD,
    hundred_million: -100_000_000,
};

const CONSTANTS_FLOAT: ConstValueFloat = ConstValueFloat {
    c1: ((b'0' as u64 + b'0' as u64 * 256) << (36 - 1)) + ((1u64 << (36 - 2)) - 7),
    e7: 10_000_000,
    e6: 1_000_000,
};

struct ShortestAscii16 {
    #[cfg(target_arch = "x86_64")]
    ascii16: __m128i,
    #[cfg(not(target_arch = "x86_64"))]
    ascii16: (u64, u64),
    dec_sig_len: u64,
}

struct ShortestAscii8 {
    ascii: u64,
    dec_sig_len: u64,
}

#[inline(always)]
unsafe fn to_ascii16(buf: *mut u8, m: u64, up_down: u64, d17: u64) -> ShortestAscii16 {
    let abcdefgh =
        ((m as u128 * CONSTANTS_DOUBLE.mul_const as u128 >> 64) as u64 >> (90 - 64)) as u32;
    let ijklmnop =
        m.wrapping_add((abcdefgh as i64 * CONSTANTS_DOUBLE.hundred_million) as u64) as u32;

    #[cfg(target_arch = "x86_64")]
    {
        let x = _mm_unpacklo_epi64(
            _mm_cvtsi32_si128(abcdefgh as _),
            _mm_cvtsi32_si128(ijklmnop as _),
        );
        let y = _mm_add_epi64(
            x,
            _mm_mul_epu32(
                _mm_set1_epi64x((1 << 32) - 10000),
                _mm_srli_epi64(_mm_mul_epu32(x, _mm_set1_epi64x(109951163)), 40),
            ),
        );

        let y_div_100 = _mm_srli_epi16(_mm_mulhi_epu16(y, _mm_set1_epi16(0x147B)), 3);
        let y_mod_100 = _mm_sub_epi16(y, _mm_mullo_epi16(y_div_100, _mm_set1_epi16(100)));
        let z = _mm_or_si128(y_div_100, _mm_slli_epi32(y_mod_100, 16));

        let z_div_10 = _mm_mulhi_epu16(z, _mm_set1_epi16(0x199A));
        let bcd_swapped = _mm_sub_epi16(
            _mm_slli_epi16(z, 8),
            _mm_mullo_epi16(_mm_set1_epi16(2559), z_div_10),
        );
        let le_bcd = _mm_shuffle_epi32(bcd_swapped, 0b1011_0001); // 0b1011_0001: _MM_SHUFFLE(2, 3, 0, 1)

        let mask = _mm_movemask_epi8(_mm_cmpgt_epi8(le_bcd, _mm_setzero_si128())) as u32 as u64;
        let tz = mask.leading_zeros() as u64;
        let ascii16 = _mm_add_epi8(le_bcd, _mm_set1_epi8('0' as _));

        _mm_storeu_si128(buf.cast(), _mm_set1_epi8('0' as _));
        _mm_storeu_si128(buf.add(16).cast(), _mm_set1_epi8('0' as _));

        return ShortestAscii16 {
            ascii16,
            dec_sig_len: select_unpredictable(up_down != 0, 63 - tz, 15 + d17),
        };
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        let abcd_efgh =
            abcdefgh as u64 + (0x100000000 - 10000) * ((abcdefgh as u64 * 0x68D_B8BB) >> 40);
        let ijkl_mnop =
            ijklmnop as u64 + (0x100000000 - 10000) * ((ijklmnop as u64 * 0x68D_B8BB) >> 40);
        let ab_cd_ef_gh =
            abcd_efgh + (0x10000 - 100) * (((abcd_efgh * 0x147B) >> 19) & 0x007F_0000_007F);
        let ij_kl_mn_op =
            ijkl_mnop + (0x10000 - 100) * (((ijkl_mnop * 0x147B) >> 19) & 0x007F_0000_007F);
        let a_b_c_d_e_f_g_h =
            ab_cd_ef_gh + (0x100 - 10) * (((ab_cd_ef_gh * 0x67) >> 10) & 0x000F_000F_000F_000F);
        let i_j_k_l_m_n_o_p =
            ij_kl_mn_op + (0x100 - 10) * (((ij_kl_mn_op * 0x67) >> 10) & 0x000F_000F_000F_000F);
        let abcdefgh_tz = a_b_c_d_e_f_g_h.trailing_zeros();
        let ijklmnop_tz = i_j_k_l_m_n_o_p.trailing_zeros();
        let abcdefgh_bcd = a_b_c_d_e_f_g_h.swap_bytes();
        let ijklmnop_bcd = i_j_k_l_m_n_o_p.swap_bytes();
        let tz = if ijklmnop == 0 {
            64 + abcdefgh_tz
        } else {
            ijklmnop_tz
        } / 8;

        buf.cast::<u64>().write_unaligned(ZERO);
        buf.add(8).cast::<u64>().write_unaligned(ZERO);
        buf.add(16).cast::<u64>().write_unaligned(ZERO);
        buf.add(24).cast::<u64>().write_unaligned(ZERO);

        ShortestAscii16 {
            ascii16: (abcdefgh_bcd | ZERO, ijklmnop_bcd | ZERO),
            dec_sig_len: select_unpredictable(up_down != 0, 15 - tz as u64, 15 + d17),
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe fn xjb64(v: f64, mut buf: *mut u8) -> usize {
    let bits = v.to_bits();
    let is_neg = (bits >> 63) as usize;

    *buf = b'-';
    buf = buf.add(is_neg);

    let sig = bits & ((1u64 << 52) - 1);
    let exp = (bits << 1) >> 53;

    let mut q = exp as i64 - 1075;
    let mut c = (1u64 << 52) | sig;

    if exp == 0 {
        if sig <= 1 {
            if sig == 0 {
                buf.cast::<u64>()
                    .write_unaligned(u64::from_le_bytes(*b"0.0\0\0\0\0\0"));
                return is_neg + 3;
            } else {
                buf.cast::<u64>()
                    .write_unaligned(u64::from_le_bytes(*b"5e-324\0\0"));
                return is_neg + 6;
            }
        }

        c = sig;
        q = 1 - 1075;
    }

    if exp == 2047 {
        let src = if sig == 0 { b"inf\0" } else { b"NaN\0" };
        buf.cast::<u32>().write_unaligned(u32::from_le_bytes(*src));
        return is_neg + 3;
    }

    let offset = 9;
    let h7_precalc = DOUBLE_TABLE.h7[exp as usize];
    let irregular = sig == 0;
    let mut k = ((exp as i64 - 1075) * 78913) >> 18;

    let get_e10 = -1 - k;
    let pow10_ptr = DOUBLE_TABLE.pow10_double.as_ptr().add(293 * 2);
    let p10 = pow10_ptr.offset((get_e10 * 2) as _);

    let cb = c << h7_precalc;
    let pow10_hi = *p10;
    let pow10_lo = *p10.add(1);
    let (hi64, lo64) = {
        let hi128: u128 = cb as u128 * pow10_hi as u128 + ((cb as u128 * pow10_lo as u128) >> 64);
        ((hi128 >> 64) as u64, hi128 as u64)
    };
    let dot_one = (hi64 << (64 - offset)) | (lo64 >> offset);
    let half_ulp = (pow10_hi >> ((1 + offset) - h7_precalc)) + ((c + 1) & 1);
    let up = half_ulp > u64::MAX - dot_one;
    let down = half_ulp > dot_one;
    let mut m_up = (hi64 >> offset) as u64 + up as u64;
    let mut up_down = up as u64 + down as u64;
    let mut one = (dot_one as u128 * 10 as u128 + CONSTANTS_DOUBLE.c4 as u128 >> 64) as u64;

    if irregular {
        k = (q * 315653 - 131072) as i64 >> 20;
        let h = q + ((k * -217707 - 217707) >> 16);
        let phi = *DOUBLE_TABLE
            .pow10_double
            .as_ptr()
            .offset((293 * 2 - 2 + k * -2) as _);
        let hulp = phi >> -h;
        let dot = phi << (53 + h);
        let irr_up = hulp > u64::MAX - dot;
        let irr_down = (hulp >> 1) > dot;

        m_up = (phi >> (11 - h)) + irr_up as u64;
        up_down = irr_up as u64 + irr_down as u64;
        one = ((dot >> (53 + h)) * 5 + (1 << (9 - h))) >> (10 - h);

        if (((dot >> 54) * 5) & ((1 << 9) - 1)) > ((hulp >> 55) * 5) {
            one = (((dot >> 54) * 5) >> 9) + 1
        }
    }

    if dot_one == 1u64 << 62 {
        one = 2
    }

    let d17 = m_up > CONSTANTS_DOUBLE.c3;
    let mr = if d17 { m_up } else { m_up * 10 };
    let s = to_ascii16(buf, mr, up_down, d17 as u64);
    let mut e10 = k + 15 + d17 as i64;

    let e10_dn = E10_DN as i64;
    let e10_up = E10_UP as i64;
    let interval = (e10_up - e10_dn + 1) as u64;

    if e10_up >= DoubleTable::MAX_DEC_SIG_LEN as i64 - 1 {
        one = select_unpredictable(up_down != 0, ZERO, one | ZERO)
    }

    let e10_3 = (e10 + -e10_dn) as u64;
    let e10_data_ofs = e10_3.min(interval);
    let (first_sig_pos, dot_pos, move_pos, mut exp_pos) = {
        let tmp = DOUBLE_TABLE.e10_variable_data[e10_data_ofs as usize];
        (
            tmp[17],
            tmp[18],
            tmp[19],
            *tmp.as_ptr().add(s.dec_sig_len as usize),
        )
    };
    let buf_origin = buf;

    buf = buf.add(first_sig_pos as _);
    #[cfg(target_arch = "x86_64")]
    buf.copy_from_nonoverlapping(&s.ascii16 as *const __m128i as *const u8, 16);
    #[cfg(not(target_arch = "x86_64"))]
    {
        buf.cast::<u64>().write_unaligned(s.ascii16.0);
        buf.add(8).cast::<u64>().write_unaligned(s.ascii16.1);
    }
    buf.add(15 + d17 as usize)
        .cast::<u64>()
        .write_unaligned(one | 0x3030_3030);
    buf.add(move_pos as _).copy_from(buf.add(dot_pos as _), 16);
    buf_origin.add(dot_pos as _).write(b'.');

    if m_up < 1e14 as u64 {
        let mut lz = 0;
        while *buf.add(2 + lz) == b'0' {
            lz += 1;
        }

        lz += 2;
        e10 -= lz as i64 - 1;
        buf.write(*buf.add(lz));
        buf.add(2).copy_from(buf.add(lz + 1), 16);
        exp_pos = exp_pos - lz as u8 + (exp_pos - lz as u8 != 1) as u8;
    }

    let exp_result = *DOUBLE_TABLE
        .exp_result_double
        .as_ptr()
        .offset(e10 as isize + 324);
    let exp_len = exp_result >> 56;

    buf = buf.add(exp_pos as _);
    buf.cast::<u64>().write_unaligned(exp_result);
    is_neg + exp_len as usize + buf.offset_from_unsigned(buf_origin)
}

pub unsafe fn xjb32(v: f32, mut buf: *mut u8) -> usize {
    let bits = v.to_bits();
    let is_neg = (bits >> 31) as usize;

    *buf = b'-';
    buf = buf.add(is_neg);

    let sig = bits & ((1 << 23) - 1);
    let exp = ((bits << 1) >> 24) as u64;
    let mut sig_bin = sig as u64 | (1 << 23);
    let mut exp_bin = exp as i64 - 150;

    if exp == 0 {
        if sig == 0 {
            buf.cast::<u32>()
                .write_unaligned(u32::from_le_bytes(*b"0.0\0"));
            return is_neg + 3;
        }

        exp_bin = 1 - 150;
        sig_bin = sig as u64;
    }

    if exp == 255 {
        let src = if sig == 0 { b"inf\0" } else { b"NaN\0" };
        buf.cast::<u32>().write_unaligned(u32::from_le_bytes(*src));
        return is_neg + 3;
    }

    let mut h37_precalc = FLOAT_TABLE.h37[exp as usize] as u32;
    let irregular = sig == 0;
    const BIT: i32 = 36;
    let mut k = (exp_bin * 1233) >> 12;

    if irregular {
        k = (exp_bin * 1233 - 512) >> 12;
        h37_precalc = ((BIT + 1) as i64 + exp_bin + ((k * -1701 - 1701) >> 9)) as u32;
    }

    let pow10_hi = *FLOAT_TABLE
        .pow10_float_reverse
        .as_ptr()
        .offset((45 + k) as _);
    let cb = sig_bin << h37_precalc;
    let hi64 = (cb as u128 * pow10_hi as u128 >> 64) as u64;
    let half_ulp = (pow10_hi >> (65 - h37_precalc)) + ((sig + 1) & 1) as u64;
    let dot_one_36bit = hi64 & ((1 << BIT) - 1);

    let m_up = ((hi64 + half_ulp) >> BIT) as u32;
    let mut up_down = (m_up > ((hi64 - half_ulp) >> BIT) as u32) as u32;
    let mut one = ((dot_one_36bit * 5 + CONSTANTS_FLOAT.c1 + (dot_one_36bit >> (BIT - 4)))
        >> (BIT - 1)) as u32;

    if irregular {
        if (exp_bin == 31 - 150) | (exp_bin == 214 - 150) | (exp_bin == 217 - 150) {
            one += 1;
        }
        up_down = (m_up > ((hi64 - (half_ulp >> 1)) >> BIT) as u32) as u32;
    }

    if E10_UP >= FloatTable::MAX_DEC_SIG_LEN as i32 - 2 {
        one = select_unpredictable(up_down != 0, b'0' as _, one);
    }

    let lz = (m_up < CONSTANTS_FLOAT.e7) as u32 + (m_up < CONSTANTS_FLOAT.e6) as u32;
    buf.write_bytes(b'0', 24);
    let s = to_ascii8(m_up as _, up_down, lz);
    let mut e10 = k + (8 - lz as i64);
    let interval = (E10_UP - E10_DN + 1) as u64;
    let e10_3 = (e10 + (-E10_DN as i64)) as u64;
    let e10_data_ofs = e10_3.min(interval);
    let exp_len = if e10_3 >= interval {
        if (e10 as u64) < (-9i64) as u64 { 4 } else { 3 }
    } else {
        0
    };
    let (first_sig_pos, dot_pos, move_pos, mut exp_pos) = {
        let tmp = FLOAT_TABLE.e10_variable_data[e10_data_ofs as usize];
        (
            tmp[9],
            tmp[10],
            tmp[11],
            *tmp.as_ptr().add(s.dec_sig_len as usize),
        )
    };
    let buf_origin = buf;

    buf = buf.add(first_sig_pos as _);
    buf.cast::<u64>().write_unaligned(s.ascii);
    buf.add(8 - lz as usize).write(one as u8);
    buf.add(move_pos as _).copy_from(buf.add(dot_pos as _), 8);
    buf_origin.add(dot_pos as _).write(b'.');

    if m_up < 100000 {
        let tmp = buf.add(2).cast::<u64>().read_unaligned();
        let lz = (tmp & 0x0F0F_0F0F_0F0F_0F0F).trailing_zeros() / 8 + 2;

        e10 -= lz as i64 - 1;
        buf.write(*buf.add(lz as _));
        buf.add(2).copy_from(buf.add(lz as usize + 1), 8);
        exp_pos = exp_pos - lz as u8 + (exp_pos - lz as u8 != 1) as u8;
    }

    let exp_result = *FLOAT_TABLE
        .exp_result_float
        .as_ptr()
        .offset(e10 as isize + 45) as u64;

    buf = buf.add(exp_pos as _);
    buf.cast::<u64>().write_unaligned(exp_result);
    is_neg + exp_len as usize + buf.offset_from(buf_origin) as usize
}

#[inline(always)]
unsafe fn to_ascii8(m: u64, up_down: u32, lz: u32) -> ShortestAscii8 {
    // prefer scaler in `x86-64-v1`
    #[cfg(all(target_arch = "x86_64", target_feature = "sse3"))]
    let abcdefgh_bcd = {
        let aabb_ccdd_merge = (m << 32).wrapping_add(
            1u64.wrapping_sub(10000 << 32)
                .wrapping_mul(((m as u128 * 1844674407370956u128) >> 64) as u64),
        );
        let y = _mm_set1_epi64x(aabb_ccdd_merge as _);
        let z = {
            let y_div_100 = _mm_srli_epi16(_mm_mulhi_epi16(y, _mm_set1_epi16(0x147b)), 3);
            let y_mod_100 = _mm_sub_epi16(y, _mm_mullo_epi16(y_div_100, _mm_set1_epi16(100)));
            _mm_or_si128(y_div_100, _mm_slli_epi32(y_mod_100, 16))
        };
        let z_div_10 = _mm_mulhi_epi16(z, _mm_set1_epi16(0x199a));
        let tmp = _mm_sub_epi16(
            _mm_slli_epi16(z, 8),
            _mm_mullo_epi16(_mm_set1_epi16(2559), z_div_10),
        );

        _mm_cvtsi128_si64(tmp) as u64
    };
    #[cfg(not(all(target_arch = "x86_64", target_feature = "sse3")))]
    let abcdefgh_bcd = {
        let aabb_ccdd_merge = (m << 32).wrapping_add(
            1u64.wrapping_sub(10000 << 32)
                .wrapping_mul((m * 109_951_163) >> 40),
        );
        let aa_bb_cc_dd_merge = (aabb_ccdd_merge << 16).wrapping_add(
            1u64.wrapping_sub(100 << 16)
                .wrapping_mul((aabb_ccdd_merge * 10_486 >> 20) & ((0x7F << 32) | 0x7F)),
        );

        (aa_bb_cc_dd_merge << 8).wrapping_add(1u64.wrapping_sub(10 << 8).wrapping_mul(
            (aa_bb_cc_dd_merge * 103 >> 10) & ((0xF << 48) | (0xF << 32) | (0xF << 16) | 0xF),
        ))
    };
    let tz = abcdefgh_bcd.leading_zeros() >> 3;

    ShortestAscii8 {
        ascii: (abcdefgh_bcd >> (lz << 3)) | ZERO,
        dec_sig_len: select_unpredictable(up_down != 0, ((7 ^ lz) - tz) as u64, 8 - lz as u64),
    }
}
