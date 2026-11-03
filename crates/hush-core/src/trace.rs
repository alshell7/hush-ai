//! Development-only operator replay. Never use these allocating paths for timing.
use eyre::{ensure, Result};
use rayon::prelude::*;

pub mod exp_norm;
pub mod sfu;

/// Allocating frequency experiments, excluded from ordinary inference and timing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrequencyMath {
    ScanSfu,
    ConvGateSfu,
    SlicedSfu,
    ExpNormSfu,
}

/// Frequency-row-major inputs to the existing native scan, with activated delta.
pub struct ScanInputs<'a> {
    pub rows: usize,
    pub channels: usize,
    pub delta: &'a [f32],
    pub x: &'a [f32],
    pub a: &'a [f32],
    pub b: &'a [f32],
    pub c: &'a [f32],
    pub d: &'a [f32],
}

/// Diagnostic ablations; none changes normal inference.
#[derive(Clone, Copy, PartialEq)]
pub enum ScanMath {
    Native,
    StandardExp,
    Exp2,
    RecurrenceFma,
    SerialReduction,
    WarpPrefix,
    WarpPrefixFma,
    CudaSourceOrder,
    CudaSfu,
}

/// Native softplus of a separately rounded FP32 delta+bias sum.
/// This is a diagnostic derivation, not an observation of CUDA's internal delta.
pub fn activate_delta(raw: &[f32], bias: &[f32]) -> Result<Vec<f32>> {
    ensure!(
        !bias.is_empty() && !raw.is_empty() && raw.len().is_multiple_of(bias.len()),
        "invalid delta shape"
    );
    ensure!(
        raw.iter().chain(bias).all(|v| v.is_finite()),
        "nonfinite delta input"
    );
    let mut delta: Vec<_> = raw
        .iter()
        .enumerate()
        .map(|(i, v)| v + bias[i % bias.len()])
        .collect();
    crate::ops::softplus_in_place::<false>(&mut delta);
    ensure!(
        delta.iter().all(|v| v.is_finite()),
        "nonfinite activated delta"
    );
    Ok(delta)
}

/// Run the normal FP32 scan kernel from zero state and add D*x in native order.
/// No model is modified; supplied constants make operator isolation explicit.
pub fn replay_scan(input: ScanInputs<'_>) -> Result<Vec<f32>> {
    replay_scan_with_math(input, ScanMath::Native)
}

/// Replay alternate FP32 arithmetic orders. The prefix variants support only 481 rows
/// (32 groups of 16, padded to 512). CudaSourceOrder combines prefix/FMA/exp2/serial
/// reduction, but still uses host transcendental functions, not CUDA instructions.
pub fn replay_scan_with_math(input: ScanInputs<'_>, math: ScanMath) -> Result<Vec<f32>> {
    let ScanInputs {
        rows,
        channels,
        delta,
        x,
        a,
        b,
        c,
        d,
    } = input;
    let size = rows
        .checked_mul(channels)
        .ok_or_else(|| eyre::eyre!("scan shape overflow"))?;
    let states = rows
        .checked_mul(16)
        .ok_or_else(|| eyre::eyre!("scan shape overflow"))?;
    let constants = channels
        .checked_mul(16)
        .ok_or_else(|| eyre::eyre!("scan shape overflow"))?;
    ensure!(
        rows > 0
            && channels > 0
            && delta.len() == size
            && x.len() == size
            && a.len() == constants
            && b.len() == states
            && c.len() == states
            && d.len() == channels,
        "invalid scan shape"
    );
    ensure!(
        [delta, x, a, b, c, d]
            .iter()
            .all(|s| s.iter().all(|v| v.is_finite())),
        "nonfinite scan input"
    );
    let prefix = matches!(
        math,
        ScanMath::WarpPrefix
            | ScanMath::WarpPrefixFma
            | ScanMath::CudaSourceOrder
            | ScanMath::CudaSfu
    );
    ensure!(
        !prefix || rows == 481,
        "prefix diagnostic requires 481 rows"
    );
    #[cfg(target_arch = "x86_64")]
    let kernel = crate::layers::scan_kernel();
    let mut transposed = vec![0.; size];
    transposed
        .par_chunks_mut(rows)
        .enumerate()
        .for_each(|(channel, out)| {
            if prefix {
                let fused = math != ScanMath::WarpPrefix;
                let cuda_order = matches!(math, ScanMath::CudaSourceOrder | ScanMath::CudaSfu);
                let mut terms = vec![[0.; 4]; rows];
                if cuda_order {
                    for (row, value) in out.iter_mut().enumerate() {
                        *value = d[channel] * x[row * channels + channel];
                    }
                }
                for n in 0..16 {
                    let mut pairs = [(1., 0.); 512];
                    for row in 0..rows {
                        let i = row * channels + channel;
                        let a = a[channel * 16 + n];
                        let decay = if math == ScanMath::CudaSfu {
                            sfu::exp2(delta[i] * (a * std::f32::consts::LOG2_E))
                        } else if cuda_order {
                            (delta[i] * (a * std::f32::consts::LOG2_E)).exp2()
                        } else {
                            crate::ops::fast_exp(delta[i] * a)
                        };
                        pairs[row] = (decay, b[row * 16 + n] * (delta[i] * x[i]));
                    }
                    let scanned = warp_prefix(pairs, fused);
                    for row in 0..rows {
                        let state = scanned[row].1;
                        let c = c[row * 16 + n];
                        if cuda_order {
                            out[row] = state.mul_add(c, out[row]);
                        } else {
                            terms[row][n % 4] += state * c;
                        }
                    }
                }
                if !cuda_order {
                    for (row, value) in out.iter_mut().enumerate() {
                        let acc = terms[row];
                        *value = ((acc[0] + acc[1]) + (acc[2] + acc[3]))
                            + d[channel] * x[row * channels + channel];
                    }
                }
                return;
            }
            let mut state = [0.; 16];
            let a = a[channel * 16..][..16].try_into().unwrap();
            for (row, value) in out.iter_mut().enumerate() {
                let i = row * channels + channel;
                let bv = b[row * 16..][..16].try_into().unwrap();
                let cv = c[row * 16..][..16].try_into().unwrap();
                let y = match math {
                    ScanMath::Native => {
                        #[cfg(target_arch = "x86_64")]
                        // SAFETY: selected by the same runtime CPU check as normal inference.
                        unsafe {
                            kernel(&mut state, a, delta[i], delta[i] * x[i], bv, cv)
                        }
                        #[cfg(not(target_arch = "x86_64"))]
                        crate::layers::scan16(&mut state, a, delta[i], delta[i] * x[i], bv, cv)
                    }
                    _ => {
                        let mut acc = [0.; 4];
                        let mut serial = d[channel] * x[i];
                        let dx = delta[i] * x[i];
                        for n in 0..16 {
                            let decay = match math {
                                ScanMath::StandardExp => (delta[i] * a[n]).exp(),
                                ScanMath::Exp2 => {
                                    (delta[i] * (a[n] * std::f32::consts::LOG2_E)).exp2()
                                }
                                ScanMath::RecurrenceFma | ScanMath::SerialReduction => {
                                    crate::ops::fast_exp(delta[i] * a[n])
                                }
                                ScanMath::WarpPrefix
                                | ScanMath::WarpPrefixFma
                                | ScanMath::CudaSourceOrder
                                | ScanMath::CudaSfu => unreachable!(),
                                ScanMath::Native => unreachable!(),
                            };
                            state[n] = if math == ScanMath::RecurrenceFma {
                                decay.mul_add(state[n], dx * bv[n])
                            } else {
                                decay * state[n] + dx * bv[n]
                            };
                            acc[n % 4] += state[n] * cv[n];
                            serial += state[n] * cv[n];
                        }
                        if math == ScanMath::SerialReduction {
                            *value = serial;
                            continue;
                        }
                        (acc[0] + acc[1]) + (acc[2] + acc[3])
                    }
                };
                *value = y + d[channel] * x[i];
            }
        });
    let output: Vec<_> = (0..size)
        .map(|i| transposed[(i % channels) * rows + i / channels])
        .collect();
    ensure!(
        output.iter().all(|v| v.is_finite()),
        "nonfinite scan output"
    );
    Ok(output)
}

type Affine = (f32, f32);

fn compose(earlier: Affine, later: Affine, fused: bool) -> Affine {
    (
        later.0 * earlier.0,
        if fused {
            later.0.mul_add(earlier.1, later.1)
        } else {
            later.0 * earlier.1 + later.1
        },
    )
}

// Independent affine-prefix implementation following the documented CUB 3.0
// BLOCK_SCAN_WARP_SCANS schedule: serial group reduction, offset-doubling warp
// scan, exclusive group seed, serial local inclusive scan. See experiment docs.
// The exact headers and generated GPU instructions were not captured.
fn warp_prefix(mut pairs: [Affine; 512], fused: bool) -> [Affine; 512] {
    let identity = (1., 0.);
    let mut totals = [identity; 32];
    for (lane, chunk) in pairs.as_chunks::<16>().0.iter().enumerate() {
        totals[lane] = chunk[1..]
            .iter()
            .fold(chunk[0], |acc, &item| compose(acc, item, fused));
    }
    for offset in [1, 2, 4, 8, 16] {
        let previous = totals;
        for lane in offset..32 {
            totals[lane] = compose(previous[lane - offset], previous[lane], fused);
        }
    }
    for (lane, chunk) in pairs.as_chunks_mut::<16>().0.iter_mut().enumerate() {
        let mut running = if lane == 0 {
            identity
        } else {
            compose(identity, totals[lane - 1], fused)
        };
        for pair in chunk {
            running = compose(running, *pair, fused);
            *pair = running;
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affine_prefix_preserves_order_and_padding() {
        // Exactly representable recurrence: resets every 7 bins expose reversed
        // composition, off-by-one seeds, lane boundaries and the partial last lane.
        let mut pairs = [(1., 0.); 512];
        for (i, pair) in pairs[..481].iter_mut().enumerate() {
            *pair = (if i % 7 == 0 { 0. } else { 1. }, (i % 5) as f32);
        }
        for fused in [false, true] {
            let scanned = warp_prefix(pairs, fused);
            let mut state = 0.;
            for (i, &(a, b)) in pairs.iter().enumerate() {
                state = a * state + b;
                assert_eq!(scanned[i].1, state, "position {i}, fused {fused}");
            }
        }
    }
}
