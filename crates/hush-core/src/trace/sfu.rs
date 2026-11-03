/*
 * Copyright (C) 2015 Marcelina Kościelnicka <mwk@0x04.net>
 * All Rights Reserved.
 *
 * Permission is hereby granted, free of charge, to any person obtaining a
 * copy of this software and associated documentation files (the "Software"),
 * to deal in the Software without restriction, including without limitation
 * the rights to use, copy, modify, merge, publish, distribute, sublicense,
 * and/or sell copies of the Software, and to permit persons to whom the
 * Software is furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice (including the next
 * paragraph) shall be included in all copies or substantial portions of the
 * Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.  IN NO EVENT SHALL
 * THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR
 * OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
 * ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
 * OTHER DEALINGS IN THE SOFTWARE.
 */
//! Diagnostic SFU model, adapted from the pinned MIT-licensed envytools reference.
//! Its vendored notice is in the development history archive. Never used by normal inference.

const EXP2: [[u64; 3]; 64] = [
    [0x2000000, 0x58b9, 0x1ee],
    [0x2059347, 0x59b0, 0x1f5],
    [0x20b361b, 0x5aaa, 0x1fa],
    [0x210e8a2, 0x5ba7, 0x1ff],
    [0x216ab0f, 0x5ca6, 0x206],
    [0x21c7d86, 0x5da9, 0x209],
    [0x2226039, 0x5eae, 0x20f],
    [0x2285356, 0x5fb6, 0x214],
    [0x22e5709, 0x60c0, 0x21d],
    [0x2346b7e, 0x61ce, 0x222],
    [0x23a90e5, 0x62df, 0x227],
    [0x240c772, 0x63f2, 0x22f],
    [0x2470f4e, 0x6509, 0x234],
    [0x24d68ad, 0x6623, 0x238],
    [0x253d3c0, 0x673f, 0x241],
    [0x25a50b6, 0x685f, 0x247],
    [0x260dfc2, 0x6982, 0x24d],
    [0x2678117, 0x6aa8, 0x254],
    [0x26e34e6, 0x6bd2, 0x258],
    [0x274fb67, 0x6cfe, 0x261],
    [0x27bd4c9, 0x6e2e, 0x267],
    [0x282c145, 0x6f61, 0x26e],
    [0x289c10b, 0x7098, 0x273],
    [0x290d456, 0x71d2, 0x279],
    [0x297fb5a, 0x730f, 0x281],
    [0x29f364e, 0x7450, 0x287],
    [0x2a6856a, 0x7594, 0x28f],
    [0x2ade8e7, 0x76dc, 0x295],
    [0x2b560fb, 0x7827, 0x29e],
    [0x2bcede2, 0x7976, 0x2a5],
    [0x2c48fd7, 0x7ac8, 0x2ae],
    [0x2cc4710, 0x7c1f, 0x2b3],
    [0x2d413cc, 0x7d79, 0x2bb],
    [0x2dbf648, 0x7ed7, 0x2c1],
    [0x2e3eebe, 0x8038, 0x2cb],
    [0x2ebfd6a, 0x819e, 0x2d1],
    [0x2f4228e, 0x8307, 0x2da],
    [0x2fc5e67, 0x8474, 0x2e3],
    [0x304b134, 0x85e5, 0x2ec],
    [0x30d1b33, 0x875b, 0x2f1],
    [0x3159ca8, 0x88d4, 0x2fb],
    [0x31e35d3, 0x8a51, 0x305],
    [0x326e6f7, 0x8bd3, 0x30b],
    [0x32fb054, 0x8d59, 0x313],
    [0x3389230, 0x8ee3, 0x31c],
    [0x3418ccf, 0x9071, 0x326],
    [0x34aa076, 0x9204, 0x32d],
    [0x353cd6a, 0x939b, 0x336],
    [0x35d13f4, 0x9536, 0x341],
    [0x3667457, 0x96d6, 0x34a],
    [0x36feede, 0x987b, 0x351],
    [0x37983d3, 0x9a24, 0x35a],
    [0x383337d, 0x9bd1, 0x366],
    [0x38cfe27, 0x9d84, 0x36d],
    [0x396e41c, 0x9f3b, 0x377],
    [0x3a0e5aa, 0xa0f7, 0x380],
    [0x3ab031d, 0xa2b7, 0x38d],
    [0x3b53cc1, 0xa47d, 0x395],
    [0x3bf92e7, 0xa648, 0x39d],
    [0x3ca05dd, 0xa817, 0x3aa],
    [0x3d495f5, 0xa9ec, 0x3b2],
    [0x3df437f, 0xabc5, 0x3bf],
    [0x3ea0ecc, 0xada4, 0x3c9],
    [0x3f4f831, 0xaf88, 0x3d4],
];

/// Older NVIDIA SFU exp2, including input/output flush-to-zero and special values.
/// Integer arithmetic and coefficients follow envytools without fitted corrections.
pub fn exp2(x: f32) -> f32 {
    let bits = x.to_bits();
    let magnitude = bits & 0x7fff_ffff;
    let negative = bits >> 31 != 0;
    if magnitude > 0x7f80_0000 {
        return f32::from_bits(0x7fff_ffff);
    }
    if magnitude >= 0x4300_0000 {
        return if negative { 0. } else { f32::INFINITY };
    }
    let exponent = (magnitude >> 23) as i32 - 127;
    let significand = (magnitude & 0x7f_ffff) | 0x80_0000;
    let fixed = if exponent >= 0 {
        significand << exponent
    } else {
        significand.checked_shr((-exponent) as u32).unwrap_or(0)
    };
    let mut exponent = (fixed >> 23) as i32;
    let mut fraction = fixed & 0x7f_ffff;
    if negative {
        exponent = -exponent;
        if fraction != 0 {
            exponent -= 1;
            fraction = !fraction & 0x7f_ffff;
        }
    }
    let [a, b, c] = EXP2[(fraction >> 17) as usize];
    let remainder = u64::from(fraction & 0x1_ffff);
    let mut square = 0;
    for i in 0..17 {
        if remainder & (1 << i) != 0 {
            square += remainder >> (18 - i);
        }
    }
    square >>= 1;
    let mut mantissa = ((a << 13) + b * remainder + c * square + 0x77e2) >> 15;
    exponent += 127;
    if mantissa == 0x100_0000 {
        mantissa >>= 1;
        exponent += 1;
    }
    if exponent <= 0 {
        0.
    } else if exponent >= 255 {
        f32::INFINITY
    } else {
        f32::from_bits(((exponent as u32) << 23) | (mantissa as u32 - 0x80_0000))
    }
}

const RCP: [[i64; 3]; 128] = [
    [67108862, -65534, 1013],
    [66588640, -64522, 989],
    [66076420, -63533, 966],
    [65572019, -62567, 945],
    [65075260, -61622, 923],
    [64585974, -60699, 902],
    [64103990, -59797, 883],
    [63629144, -58914, 863],
    [63161282, -58051, 845],
    [62700252, -57207, 827],
    [62245903, -56381, 809],
    [61798089, -55572, 791],
    [61356675, -54781, 774],
    [60921522, -54007, 758],
    [60492497, -53249, 742],
    [60069472, -52507, 727],
    [59652323, -51780, 711],
    [59240928, -51068, 696],
    [58835168, -50371, 682],
    [58434928, -49688, 669],
    [58040097, -49019, 656],
    [57650567, -48363, 642],
    [57266229, -47721, 631],
    [56886984, -47091, 618],
    [56512727, -46473, 605],
    [56143364, -45868, 594],
    [55778796, -45274, 582],
    [55418932, -44692, 572],
    [55063684, -44121, 561],
    [54712959, -43560, 549],
    [54366675, -43011, 540],
    [54024745, -42471, 529],
    [53687091, -41942, 519],
    [53353631, -41423, 510],
    [53024286, -40913, 501],
    [52698985, -40412, 490],
    [52377649, -39921, 482],
    [52060210, -39439, 474],
    [51746594, -38965, 465],
    [51436735, -38500, 457],
    [51130563, -38043, 449],
    [50828015, -37594, 441],
    [50529027, -37153, 433],
    [50233536, -36720, 426],
    [49941480, -36294, 418],
    [49652801, -35876, 412],
    [49367440, -35464, 403],
    [49085340, -35060, 397],
    [48806447, -34663, 390],
    [48530703, -34272, 383],
    [48258058, -33888, 377],
    [47988462, -33511, 371],
    [47721859, -33140, 366],
    [47458202, -32774, 358],
    [47197442, -32415, 353],
    [46939532, -32062, 348],
    [46684427, -31714, 341],
    [46432080, -31373, 337],
    [46182444, -31036, 331],
    [45935478, -30705, 326],
    [45691141, -30379, 320],
    [45449390, -30059, 316],
    [45210183, -29743, 310],
    [44973479, -29432, 305],
    [44739242, -29127, 302],
    [44507434, -28826, 297],
    [44278014, -28529, 291],
    [44050946, -28237, 287],
    [43826197, -27950, 283],
    [43603728, -27667, 279],
    [43383508, -27388, 274],
    [43165501, -27114, 271],
    [42949673, -26843, 266],
    [42735993, -26577, 263],
    [42524428, -26314, 258],
    [42314948, -26056, 256],
    [42107523, -25801, 251],
    [41902120, -25550, 248],
    [41698712, -25302, 243],
    [41497268, -25058, 240],
    [41297762, -24818, 237],
    [41100166, -24581, 233],
    [40904451, -24348, 231],
    [40710591, -24117, 226],
    [40518559, -23890, 223],
    [40328331, -23667, 222],
    [40139881, -23446, 218],
    [39953184, -23228, 214],
    [39768216, -23014, 212],
    [39584953, -22802, 208],
    [39403369, -22593, 205],
    [39223446, -22388, 204],
    [39045157, -22184, 199],
    [38868482, -21984, 197],
    [38693399, -21787, 196],
    [38519886, -21592, 193],
    [38347922, -21399, 189],
    [38177487, -21209, 186],
    [38008561, -21022, 184],
    [37841121, -20837, 182],
    [37675152, -20655, 180],
    [37510631, -20475, 178],
    [37347541, -20297, 175],
    [37185864, -20122, 173],
    [37025580, -19949, 171],
    [36866671, -19778, 169],
    [36709122, -19609, 166],
    [36552913, -19443, 165],
    [36398028, -19278, 161],
    [36244450, -19116, 160],
    [36092162, -18956, 159],
    [35941149, -18797, 155],
    [35791394, -18641, 154],
    [35642882, -18487, 153],
    [35495597, -18334, 150],
    [35349526, -18184, 149],
    [35204650, -18035, 147],
    [35060957, -17888, 145],
    [34918434, -17743, 143],
    [34777062, -17600, 143],
    [34636833, -17458, 140],
    [34497730, -17318, 138],
    [34359739, -17180, 137],
    [34222847, -17043, 135],
    [34087042, -16908, 133],
    [33952311, -16775, 132],
    [33818639, -16643, 131],
    [33686017, -16513, 130],
];

/// Reciprocal from the same pinned envytools SFU implementation as `exp2`.
/// Retains input/output flush-to-zero, signed zeros and the canonical NaN.
pub fn reciprocal(x: f32) -> f32 {
    let bits = x.to_bits();
    let sign = bits & 0x8000_0000;
    let magnitude = bits & 0x7fff_ffff;
    if magnitude > 0x7f80_0000 {
        return f32::from_bits(0x7fff_ffff);
    }
    if magnitude == 0x7f80_0000 {
        return f32::from_bits(sign);
    }
    let input_exponent = (magnitude >> 23) as i32;
    if input_exponent == 0 {
        return f32::from_bits(sign | 0x7f80_0000);
    }
    let mut exponent = 253 - input_exponent;
    let [a, b, c] = RCP[((magnitude >> 16) & 127) as usize];
    let remainder = i64::from(magnitude & 0xffff);
    let shifted = remainder << 1;
    let mut square = 0;
    for i in 0..17 {
        if shifted & (1 << i) != 0 {
            square += shifted >> (18 - i);
        }
    }
    square >>= 1;
    let mut mantissa = ((a << 13) + b * remainder + c * square + 0x47e7) >> 15;
    if mantissa == 0x100_0000 {
        mantissa >>= 1;
        exponent += 1;
    }
    if exponent <= 0 {
        f32::from_bits(sign)
    } else {
        f32::from_bits(sign | ((exponent as u32) << 23) | (mantissa as u32 - 0x80_0000))
    }
}

/// Fast-math SiLU instruction order observed in the saved CUDA scan kernel.
/// The reciprocal is approximate; ordinary inference must not call this function.
pub fn silu(x: f32) -> f32 {
    let e = exp2(ftz(-ftz(x) * std::f32::consts::LOG2_E));
    ftz(ftz(x) * reciprocal(ftz(1. + e)))
}

fn ftz(x: f32) -> f32 {
    if x.is_subnormal() {
        f32::from_bits(x.to_bits() & 0x8000_0000)
    } else {
        x
    }
}

fn fma(a: f32, b: f32, c: f32) -> f32 {
    ftz(ftz(a).mul_add(ftz(b), ftz(c)))
}

/// Reproduce the positive-input log1p sequence in the retained CUDA 13.0.2 sm_75
/// scan disassembly (0x1420..0x1560). This is a version-specific diagnostic.
fn log1p_positive(x: f32) -> f32 {
    debug_assert!(x.is_finite() && x >= 0.);
    let sum = f64::from(x) + 1.;
    let mut toward_zero = sum as f32;
    if f64::from(toward_zero) > sum {
        toward_zero = f32::from_bits(toward_zero.to_bits() - 1);
    }
    let k = toward_zero.to_bits().wrapping_sub(0x3f40_0000) & 0xff80_0000;
    let scale = f32::from_bits(0x4080_0000u32.wrapping_sub(k));
    let mantissa = f32::from_bits(x.to_bits().wrapping_sub(k));
    let reduced = ftz(mantissa + fma(scale, 0.25, -1.));
    let mut polynomial = f32::from_bits(0xbd39_bf78);
    for coefficient in [
        0x3dd8_0012,
        0xbe07_78e0,
        0x3e14_6475,
        0xbe2a_68dd,
        0x3e4c_af9e,
        0xbe80_0042,
        0x3eaa_aae6,
        0xbf00_0000,
    ] {
        polynomial = fma(reduced, polynomial, f32::from_bits(coefficient));
    }
    polynomial = ftz(reduced * polynomial);
    polynomial = fma(reduced, polynomial, reduced);
    let exponent = (k as i32) as f32 * f32::from_bits(0x3400_0000);
    fma(exponent, f32::from_bits(0x3f31_7218), polynomial)
}

/// Threshold-20 softplus from the observed scan instructions; not a runtime option.
pub fn softplus(logit: f32) -> f32 {
    if logit.is_nan() {
        return f32::from_bits(0x7fff_ffff);
    }
    if logit > 20. {
        return logit;
    }
    log1p_positive(exp2(ftz(logit * std::f32::consts::LOG2_E)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_saved_frequency_convolution_and_gate() {
        let convolution = include_bytes!("../../tests/fixtures/sfu-surrounding/convolution.f32");
        let gate = include_bytes!("../../tests/fixtures/sfu-surrounding/gate.f32");
        assert_eq!(convolution.len(), 2808 * 40);
        assert_eq!(gate.len(), 2808 * 12);
        for row in convolution.as_chunks::<40>().0 {
            let v: Vec<_> = row
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect();
            let mut acc = v[0];
            for pair in v[1..9].as_chunks::<2>().0 {
                acc = pair[0].mul_add(pair[1], acc);
            }
            assert_eq!(silu(acc).to_bits(), v[9].to_bits());
        }
        for row in gate.as_chunks::<12>().0 {
            let v: Vec<_> = row
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect();
            assert_eq!((v[1] * silu(v[0])).to_bits(), v[2].to_bits());
        }
    }

    #[test]
    fn reciprocal_special_values_and_powers() {
        for sign in [0, 0x8000_0000] {
            for bits in [0, 1, 0x7f_ffff] {
                assert_eq!(
                    reciprocal(f32::from_bits(sign | bits)).to_bits(),
                    sign | 0x7f80_0000
                );
            }
            assert_eq!(
                reciprocal(f32::from_bits(sign | 0x7f80_0000)).to_bits(),
                sign
            );
            for exponent in 1..255 {
                let input = f32::from_bits(sign | (exponent << 23));
                let expected = ftz(1. / input);
                assert_eq!(reciprocal(input).to_bits(), expected.to_bits());
            }
        }
        assert!(reciprocal(f32::NAN).is_nan());
        assert_eq!(silu(0.).to_bits(), 0);
        assert_eq!(silu(-0.).to_bits(), 0x8000_0000);
    }

    #[test]
    fn matches_independent_t4_witnesses() {
        let data = include_bytes!("../../tests/fixtures/sfu-exp2/witnesses.u32");
        assert_eq!(data.len(), 4866 * 24);
        for row in data.as_chunks::<24>().0 {
            let words: Vec<_> = row
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| u32::from_le_bytes(*b))
                .collect();
            assert_eq!(words[3], words[4], "CUDA exp2f differs from inline PTX");
            assert_eq!(words[3], words[5], "upstream C differs from CUDA");
            let value = exp2(f32::from_bits(words[2]));
            assert_eq!(
                value.to_bits(),
                words[3],
                "group {} sample {} input {:08x}",
                words[0],
                words[1],
                words[2]
            );
        }
    }

    #[test]
    fn special_values_integers_and_underflow_boundary() {
        for x in [0., -0., f32::from_bits(1), -f32::from_bits(1)] {
            assert_eq!(exp2(x).to_bits(), 1f32.to_bits());
        }
        for exponent in -126..=127 {
            assert_eq!(
                exp2(exponent as f32).to_bits(),
                ((exponent + 127) as u32) << 23
            );
        }
        assert_eq!(exp2(f32::from_bits((-126f32).to_bits() + 1)), 0.);
        assert_eq!(exp2(-128.), 0.);
        assert_eq!(exp2(f32::NEG_INFINITY), 0.);
        assert_eq!(exp2(128.), f32::INFINITY);
        assert_eq!(exp2(f32::INFINITY), f32::INFINITY);
        assert_eq!(exp2(f32::NAN).to_bits(), 0x7fff_ffff);
        assert_eq!(softplus(f32::NEG_INFINITY), 0.);
        assert_eq!(softplus(f32::INFINITY), f32::INFINITY);
        assert_eq!(softplus(21.), 21.);
        assert!(softplus(f32::NAN).is_nan());
    }
}
