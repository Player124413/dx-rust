//! Small, dependency-free ETC2 encoder for the Android texture cache.
//!
//! It intentionally favours predictable load-time cost over an offline compressor's exhaustive search:
//! differential ETC1-compatible RGB blocks with a bounded endpoint/table search, plus EAC alpha for
//! translucent textures. Opaque textures cost 8 bytes per 4x4 block; alpha textures cost 16. The output
//! is standard ETC2 RGBA/RGB, not a proprietary container. `texture2ddecoder` is used in tests as an
//! independent decoder to catch bit-order and colour regressions.

pub const ENCODER_VERSION: u32 = 1;
const MODS: [[i32; 2]; 8] = [[2, 8], [5, 17], [9, 29], [13, 42], [18, 60], [24, 80], [33, 106], [47, 183]];
const ALPHA_MODS: [[i32; 8]; 16] = [
    [-3, -6, -9, -15, 2, 5, 8, 14], [-3, -7, -10, -13, 2, 6, 9, 12],
    [-2, -5, -8, -13, 1, 4, 7, 12], [-2, -4, -6, -13, 1, 3, 5, 12],
    [-3, -6, -8, -12, 2, 5, 7, 11], [-3, -7, -9, -11, 2, 6, 8, 10],
    [-4, -7, -8, -11, 3, 6, 7, 10], [-3, -5, -8, -11, 2, 4, 7, 10],
    [-2, -6, -8, -10, 1, 5, 7, 9], [-2, -5, -8, -10, 1, 4, 7, 9],
    [-2, -4, -8, -10, 1, 3, 7, 9], [-2, -5, -7, -10, 1, 4, 6, 9],
    [-3, -4, -7, -10, 2, 3, 6, 9], [-1, -2, -3, -10, 0, 1, 2, 9],
    [-4, -6, -8, -9, 3, 5, 7, 8], [-3, -5, -7, -9, 2, 4, 6, 8],
];

pub fn encoded_len(w: u32, h: u32, alpha: bool) -> usize {
    (w.div_ceil(4) as usize) * (h.div_ceil(4) as usize) * if alpha { 16 } else { 8 }
}

fn expand5(v: u8) -> i32 { let v = v as i32; (v << 3) | (v >> 2) }
fn q5(v: i32) -> u8 { ((v.clamp(0, 255) * 31 + 127) / 255) as u8 }
fn subblock(x: usize, y: usize, flip: bool) -> usize { if flip { y / 2 } else { x / 2 } }

/// Encode one 4x4 RGB block. `px` is row-major RGB within the block; edge pixels must be repeated by caller.
fn rgb_block(px: &[[u8; 3]; 16]) -> [u8; 8] {
    let mut best = u64::MAX;
    let mut out = [0u8; 8];
    for flip in [false, true] {
        let mut avg = [[0i32; 3]; 2];
        let mut lo = [[255i32; 3]; 2];
        let mut hi = [[0i32; 3]; 2];
        for y in 0..4 { for x in 0..4 {
            let s = subblock(x, y, flip); let p = px[y * 4 + x];
            for c in 0..3 { avg[s][c] += p[c] as i32; lo[s][c] = lo[s][c].min(p[c] as i32); hi[s][c] = hi[s][c].max(p[c] as i32); }
        }}
        for s in 0..2 { for c in 0..3 { avg[s][c] /= 8; } }
        // Two spreads: the full range preserves edges; the tighter one avoids ringing on noisy texels.
        for spread in [40, 28] {
            let mut base = [[0u8; 3]; 2]; let mut second = [[0u8; 3]; 2]; let mut valid = true;
            for s in 0..2 { for c in 0..3 {
                let d = (hi[s][c] - lo[s][c]) * spread / 100;
                let a = q5(avg[s][c] - d); let b = q5(avg[s][c] + d);
                let diff = b as i32 - a as i32;
                if !(-4..=3).contains(&diff) { valid = false; }
                base[s][c] = a; second[s][c] = (a as i32 + diff.clamp(-4, 3)) as u8;
            }}
            if !valid { continue; }
            let colors = [[expand5(base[0][0]), expand5(base[0][1]), expand5(base[0][2])],
                          [expand5(second[0][0]), expand5(second[0][1]), expand5(second[0][2])]];
            let mut ids = [0u8; 16]; let mut err = 0u64; let mut tables = [0usize; 2];
            for s in 0..2 {
                let mut best_s = u64::MAX; let mut best_t = 0; let mut best_ids = [0u8; 8]; let mut n = 0;
                for y in 0..4 { for x in 0..4 { if subblock(x, y, flip) == s {
                    let p = px[y * 4 + x]; let mut pe = u64::MAX; let mut pi = 0;
                    for t in 0..8 {
                        let mut te = 0u64; let mut ti = 0;
                        for (i, m) in [MODS[t][0], MODS[t][1], -MODS[t][0], -MODS[t][1]].iter().enumerate() {
                            let e = (0..3).map(|c| { let d = p[c] as i32 - (colors[s][c] + m).clamp(0, 255); (d * d) as u64 }).sum::<u64>();
                            if e < pe { pe = e; pi = i as u8; }
                            if e < te { te = e; ti = i as u8; }
                        }
                        if te < best_s { best_s = te; best_t = t; best_ids[n] = ti; }
                    }
                    ids[y * 4 + x] = pi; err += pe; n += 1;
                }}}
                tables[s] = best_t;
                let _ = best_ids;
            }
            // The per-subblock search above chooses a table using all pixels; recompute exact indices for it.
            err = 0;
            for y in 0..4 { for x in 0..4 {
                let s = subblock(x, y, flip); let t = tables[s]; let p = px[y * 4 + x]; let mut pe = u64::MAX;
                for (i, m) in [MODS[t][0], MODS[t][1], -MODS[t][0], -MODS[t][1]].iter().enumerate() {
                    let e = (0..3).map(|c| { let d = p[c] as i32 - (colors[s][c] + m).clamp(0, 255); (d * d) as u64 }).sum::<u64>();
                    if e < pe { pe = e; ids[y * 4 + x] = i as u8; }
                }
                err += pe;
            }}
            if err < best {
                best = err;
                let mut bits = [0u8; 8];
                for c in 0..3 {
                    let a = base[0][c]; let d = second[0][c] as i32 - a as i32;
                    bits[c] = (a << 3) | ((d as u8) & 7);
                }
                bits[3] = ((tables[0] as u8) << 5) | ((tables[1] as u8) << 2) | 2 | flip as u8;
                let mut idx = 0u32;
                for y in 0..4 { for x in 0..4 {
                    let i = x * 4 + y; let id = ids[y * 4 + x] as u32;
                    idx |= ((id >> 1) & 1) << (16 + i); idx |= (id & 1) << i;
                }}
                bits[4..8].copy_from_slice(&idx.to_be_bytes()); out = bits;
            }
        }
    }
    out
}

fn alpha_block(a: &[u8; 16]) -> [u8; 8] {
    let avg = a.iter().map(|v| *v as i32).sum::<i32>() / 16;
    let bases = [0, 128, 255, avg];
    let mut best = u64::MAX; let mut best_word = 0u64;
    for base in bases { for table in 0..16 { for mult in 1..16 {
        let mut err = 0u64; let mut ids = [0u8; 16];
        for y in 0..4 { for x in 0..4 {
            let v = a[y * 4 + x] as i32; let mut pe = u64::MAX;
            for (i, m) in ALPHA_MODS[table].iter().enumerate() {
                let d = v - (base + mult * m).clamp(0, 255); let e = (d * d) as u64;
                if e < pe { pe = e; ids[y * 4 + x] = i as u8; }
            }
            err += pe;
        }}
        if err < best {
            best = err;
            let mut word = ((base as u64) << 56) | ((mult as u64) << 52) | ((table as u64) << 48);
            for y in 0..4 { for x in 0..4 { word = (word << 3) | ids[y * 4 + x] as u64; } }
            best_word = word;
        }
    }}}
    best_word.to_be_bytes()
}

/// Encode a whole RGBA8 image. `alpha=false` discards alpha and writes ETC2 RGB8; otherwise ETC2 RGBA8.
pub fn encode(rgba: &[u8], w: u32, h: u32, alpha: bool) -> Vec<u8> {
    assert_eq!(rgba.len(), w as usize * h as usize * 4);
    let bw = w.div_ceil(4) as usize; let bh = h.div_ceil(4) as usize;
    let mut out = vec![0u8; encoded_len(w, h, alpha)];
    for by in 0..bh { for bx in 0..bw {
        let mut rgb = [[0u8; 3]; 16]; let mut a = [255u8; 16];
        for y in 0..4 { for x in 0..4 {
            let sx = (bx * 4 + x).min(w as usize - 1); let sy = (by * 4 + y).min(h as usize - 1);
            let p = &rgba[(sy * w as usize + sx) * 4..][..4]; let i = y * 4 + x;
            rgb[i] = [p[0], p[1], p[2]]; a[i] = p[3];
        }}
        let o = (by * bw + bx) * if alpha { 16 } else { 8 };
        if alpha { out[o..o + 8].copy_from_slice(&alpha_block(&a)); }
        let ro = if alpha { o + 8 } else { o };
        out[ro..ro + 8].copy_from_slice(&rgb_block(&rgb));
    }}
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn psnr(src: &[u8], dst: &[u32], alpha: bool) -> f64 {
        let mut se = 0f64; let mut n = 0;
        for (p, d) in src.chunks_exact(4).zip(dst) {
            let px = d.to_le_bytes();
            for c in 0..3 { let e = p[c] as f64 - px[c] as f64; se += e * e; n += 1; }
            if alpha { let e = p[3] as f64 - px[3] as f64; se += e * e; n += 1; }
        }
        10.0 * (255.0 * 255.0 / (se / n as f64).max(1e-9)).log10()
    }
    #[test]
    fn independent_decoder_accepts_rgb_and_alpha() {
        let (w, h) = (32, 32); let mut src = vec![0u8; w * h * 4];
        for y in 0..h { for x in 0..w { let i = (y * w + x) * 4; src[i] = (x * 8) as u8; src[i + 1] = (y * 8) as u8; src[i + 2] = ((x + y) * 4) as u8; src[i + 3] = if (x / 4 + y / 4) % 2 == 0 { 255 } else { 0 }; }}
        let rgb = encode(&src, w as u32, h as u32, false); let mut dec = vec![0u32; w * h];
        texture2ddecoder::decode_etc2_rgb(&rgb, w, h, &mut dec).unwrap();
        assert!(psnr(&src, &dec, false) > 30.0, "RGB PSNR {}", psnr(&src, &dec, false));
        let rgba = encode(&src, w as u32, h as u32, true); dec.fill(0);
        texture2ddecoder::decode_etc2_rgba8(&rgba, w, h, &mut dec).unwrap();
        assert!(psnr(&src, &dec, true) > 30.0, "RGBA PSNR {}", psnr(&src, &dec, true));
    }
    #[test]
    fn solid_and_non_multiple_of_four() {
        let src = [200u8, 30, 40, 255].repeat(5 * 3);
        let enc = encode(&src, 5, 3, false); assert_eq!(enc.len(), encoded_len(5, 3, false));
        let mut dec = vec![0u32; 15]; texture2ddecoder::decode_etc2_rgb(&enc, 5, 3, &mut dec).unwrap();
        assert!(dec.iter().all(|p| { let b = p.to_le_bytes(); b[0] > 190 && b[1] < 40 && b[2] < 50 && b[3] == 255 }));
    }
}
