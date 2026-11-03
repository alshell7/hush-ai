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
//! CUDA 13.0.88 exponential and older NVIDIA reciprocal-square-root diagnostics.
//! Arithmetic is independently probed on T4; never selected by ordinary inference.
//! LayerNorm follows PyTorch v2.11.0; its vendored notice is in the development history archive.

/// CUDA libdevice's accurate exponential range reduction followed by MUFU.EX2.
/// Saturation bounds the directed-rounding step, so its double-precision product
/// and sum are exact; floor then implements the required FP32 rounding downward.
pub fn exp(x: f32) -> f32 {
    if x.is_nan() {
        return f32::from_bits(0x7fff_ffff);
    }
    let hi = f32::from_bits(1069066811);
    let lo = f32::from_bits(849703008);
    let magic = f32::from_bits(1262485504);
    let sat = x.mul_add(hi / 252., 0.5).clamp(0., 1.);
    let rounded = (f64::from(sat) * 252. + f64::from((magic - 126.) + 127.)).floor() as f32;
    let exponent = rounded - (magic + 127.);
    let residual = x.mul_add(hi, -exponent);
    let residual = x.mul_add(lo, residual);
    let scale = f32::from_bits(rounded.to_bits() << 23);
    super::sfu::exp2(residual) * scale
}

const RSQ: [[i64; 3]; 128] = [
    [67108861, -65531, 754],
    [66590637, -64024, 725],
    [66084239, -62575, 698],
    [65589219, -61179, 672],
    [65105160, -59835, 648],
    [64631661, -58539, 625],
    [64168345, -57289, 603],
    [63714853, -56083, 582],
    [63270843, -54919, 562],
    [62835986, -53794, 543],
    [62409972, -52708, 526],
    [61992511, -51657, 508],
    [61583314, -50641, 492],
    [61182118, -49658, 476],
    [60788659, -48706, 461],
    [60402696, -47784, 446],
    [60023990, -46891, 433],
    [59652323, -46026, 420],
    [59287472, -45186, 407],
    [58929237, -44372, 395],
    [58577418, -43582, 383],
    [58231825, -42815, 372],
    [57892277, -42071, 362],
    [57558601, -41348, 352],
    [57230629, -40645, 342],
    [56908203, -39962, 332],
    [56591163, -39298, 323],
    [56279364, -38652, 314],
    [55972662, -38023, 305],
    [55670920, -37412, 298],
    [55374004, -36816, 290],
    [55081792, -36237, 283],
    [54794158, -35672, 275],
    [54510983, -35122, 268],
    [54232150, -34586, 262],
    [53957556, -34063, 255],
    [53687091, -33553, 248],
    [53420652, -33056, 242],
    [53158142, -32571, 236],
    [52899461, -32098, 231],
    [52644524, -31636, 225],
    [52393234, -31185, 220],
    [52145511, -30745, 215],
    [51901269, -30315, 210],
    [51660427, -29895, 205],
    [51422906, -29485, 201],
    [51188634, -29084, 196],
    [50957532, -28691, 191],
    [50729534, -28308, 187],
    [50504567, -27933, 183],
    [50282567, -27566, 179],
    [50063470, -27208, 176],
    [49847213, -26857, 172],
    [49633733, -26513, 168],
    [49422973, -26177, 165],
    [49214874, -25848, 162],
    [49009384, -25525, 158],
    [48806447, -25209, 154],
    [48606010, -24900, 151],
    [48408021, -24597, 148],
    [48212432, -24300, 145],
    [48019192, -24009, 143],
    [47828262, -23724, 140],
    [47639589, -23444, 137],
    [47453129, -46337, 533],
    [47086690, -45272, 513],
    [46728614, -44247, 493],
    [46378581, -43260, 475],
    [46036298, -42310, 459],
    [45701484, -41393, 442],
    [45373873, -40510, 427],
    [45053204, -39657, 412],
    [44739242, -38834, 398],
    [44431751, -38038, 384],
    [44130517, -37270, 371],
    [43835325, -36527, 359],
    [43545981, -35809, 348],
    [43262290, -35113, 336],
    [42984071, -34440, 326],
    [42711157, -33789, 316],
    [42443371, -33157, 306],
    [42180561, -32545, 297],
    [41922576, -31952, 288],
    [41669265, -31376, 279],
    [41420489, -30817, 271],
    [41176119, -30275, 263],
    [40936023, -29749, 256],
    [40700079, -29237, 248],
    [40468167, -28740, 241],
    [40240174, -28257, 235],
    [40015993, -27788, 229],
    [39795520, -27331, 222],
    [39578647, -26887, 217],
    [39365286, -26454, 210],
    [39155335, -26033, 205],
    [38948711, -25623, 199],
    [38745319, -25224, 195],
    [38545084, -24835, 190],
    [38347923, -24456, 185],
    [38153754, -24086, 180],
    [37962506, -23726, 176],
    [37774105, -23374, 171],
    [37588482, -23031, 167],
    [37405570, -22697, 163],
    [37225300, -22370, 159],
    [37047613, -22051, 155],
    [36872445, -21740, 152],
    [36699741, -21436, 148],
    [36529438, -21139, 145],
    [36361486, -20849, 142],
    [36195831, -20565, 138],
    [36032415, -20288, 136],
    [35871195, -20017, 133],
    [35712121, -19752, 130],
    [35555146, -19492, 126],
    [35400221, -19239, 124],
    [35247302, -18991, 122],
    [35096351, -18748, 119],
    [34947318, -18510, 117],
    [34800172, -18277, 114],
    [34654867, -18049, 112],
    [34511368, -17826, 110],
    [34369639, -17607, 107],
    [34229640, -17393, 105],
    [34091338, -17183, 103],
    [33954697, -16977, 101],
    [33819687, -16775, 99],
    [33686279, -16578, 97],
];

/// MUFU.RSQ with input flush-to-zero, adapted from the unchanged envytools table.
/// CUDA's non-FTZ rsqrtf differs on subnormals. LayerNorm variance plus epsilon
/// is positive and at least 1e-5; do not use this as a general rsqrtf replacement.
pub fn rsqrt_ftz(x: f32) -> f32 {
    let bits = x.to_bits();
    let magnitude = bits & 0x7fff_ffff;
    let sign = bits & 0x8000_0000;
    if magnitude > 0x7f80_0000 {
        return f32::from_bits(0x7fff_ffff);
    }
    if magnitude < 0x0080_0000 {
        return f32::from_bits(sign | 0x7f80_0000);
    }
    if sign != 0 {
        return f32::from_bits(0x7fff_ffff);
    }
    if magnitude == 0x7f80_0000 {
        return 0.;
    }
    let e = (magnitude >> 23) as i32 - 127;
    let fraction = (magnitude & 0x7f_ffff) | (((e & 1) as u32) << 23);
    let mut exponent = 126 - (e >> 1);
    let mut mantissa = if fraction == 0 {
        exponent += 1;
        0x80_0000
    } else {
        let [a, b, c] = RSQ[(fraction >> 17) as usize];
        let remainder = i64::from(fraction & 0x1_ffff);
        let mut square = 0;
        for i in 0..17 {
            if remainder & (1 << i) != 0 {
                square += remainder >> (18 - i);
            }
        }
        square >>= 1;
        ((a << 14) + b * remainder + ((c * square) << 2) + 0x7fff) >> 16
    };
    if mantissa == 0x100_0000 {
        mantissa >>= 1;
        exponent += 1;
    }
    f32::from_bits(((exponent as u32) << 23) | (mantissa as u32 - 0x80_0000))
}

/// PyTorch 2.11 vectorized LayerNorm statistics for the measured 64-channel shape.
/// Four adjacent values per lane, shuffle-down reduction, then two empty warps.
/// FMA placement follows the retained sm_75 instructions, not a fitted ordering.
pub fn layer_norm_stats_64(row: &[f32]) -> (f32, f32) {
    assert_eq!(row.len(), 64);
    #[derive(Clone, Copy, Default)]
    struct Stats {
        mean: f32,
        m2: f32,
        count: f32,
    }
    fn combine(current: Stats, other: Stats) -> Stats {
        let count = other.count + current.count;
        if count == 0. {
            return Stats::default();
        }
        let delta = current.mean - other.mean;
        let inverse = 1. / count;
        let a = other.count * inverse;
        let b = current.count * inverse;
        Stats {
            mean: other.mean.mul_add(a, b * current.mean),
            m2: b.mul_add(other.count * (delta * delta), other.m2 + current.m2),
            count,
        }
    }
    let mut lanes = [Stats::default(); 32];
    for (lane, values) in lanes.iter_mut().zip(row.as_chunks::<4>().0) {
        for value in values {
            let delta = value - lane.mean;
            lane.count += 1.;
            lane.mean = delta.mul_add(1. / lane.count, lane.mean);
            lane.m2 = delta.mul_add(value - lane.mean, lane.m2);
        }
    }
    for offset in [16, 8, 4, 2, 1] {
        let previous = lanes;
        for i in 0..32 - offset {
            lanes[i] = combine(previous[i], previous[i + offset]);
        }
    }
    let stats = combine(combine(lanes[0], Stats::default()), Stats::default());
    (stats.mean, rsqrt_ftz(stats.m2 / 64. + 1e-5))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_independent_exp_rsqrt_witnesses() {
        let data = include_bytes!("../../tests/fixtures/exp-norm/witnesses.u32");
        assert_eq!(data.len(), 5396 * 36);
        for row in data.as_chunks::<36>().0 {
            let v: Vec<_> = row
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| u32::from_le_bytes(*b))
                .collect();
            let x = f32::from_bits(v[2]);
            assert_eq!(exp(x).to_bits(), v[3], "exp group {} index {}", v[0], v[1]);
            assert_eq!(
                rsqrt_ftz(x).to_bits(),
                v[5],
                "rsqrt group {} index {}",
                v[0],
                v[1]
            );
        }
    }

    #[test]
    fn matches_captured_layer_norm_statistics_and_output() {
        let data = include_bytes!("../../tests/fixtures/exp-norm/norm.f32");
        assert_eq!(data.len(), 30 * 258 * 4);
        for row in data.as_chunks::<{ 258 * 4 }>().0 {
            let v: Vec<_> = row
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect();
            let (mean, rstd) = layer_norm_stats_64(&v[..64]);
            assert_eq!(mean.to_bits(), v[64].to_bits());
            assert_eq!(rstd.to_bits(), v[65].to_bits());
            for i in 0..64 {
                let y = (rstd * (v[i] - mean)).mul_add(v[66 + i], v[130 + i]);
                assert_eq!(y.to_bits(), v[194 + i].to_bits());
            }
        }
    }
}
