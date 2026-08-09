//! Robust JPEG-domain steganography (QIM: Quantization Index Modulation).
//!
//! Port of the algorithm validated empirically in `channel_simulator/dct_variants.py`
//! (encode_dct_qim / decode_dct_qim) against a simulated WhatsApp / Instagram /
//! Telegram / Facebook / Twitter channel (resize + JPEG recompression + metadata
//! strip): 100% payload-recovery rate across every platform and cover-image type
//! tested at the "max" width preset. See channel_simulator/BASELINE_RESULTS.md and
//! run_matrix_realistic.py for the methodology and numbers behind these constants.
//!
//! WHY THIS EXISTS: the original DWT (Haar 2D) encoder in `stego.rs` embeds in
//! spatial-domain LSBs and is documented (BASELINE_RESULTS.md) to NOT survive any
//! of the above platforms -- they all re-encode images as JPEG, which recomputes
//! every pixel from scratch and wipes spatial-domain LSBs. This module embeds
//! directly in JPEG DCT coefficients instead, which is the domain those platforms'
//! own re-compression operates in, and adds redundancy + error correction so the
//! payload survives being re-quantized at a different JPEG quality.
//!
//! KEY DESIGN POINTS carried over from the validated Python prototype (see comments
//! inline in dct_variants.py for the empirical reasoning behind each):
//!   - Coefficient-domain (position-based) embedding cannot survive an actual pixel
//!     resize -- resampling recomputes every 8x8 block from scratch. So embed always
//!     pre-resizes the cover to a width safe for every platform we care about (never
//!     guesses a single destination platform), guaranteeing every downstream resize
//!     is a no-op.
//!   - Redundancy copies of a bit MUST land in spatially decorrelated coefficients.
//!     Naive consecutive repetition puts all copies in the same 8x8 block, so a
//!     block that quantizes badly (e.g. a flat/low-energy region) makes every copy
//!     wrong the same way -- majority voting over correlated failures does nothing.
//!     This uses a single fixed pseudorandom permutation of all coefficient
//!     positions (same seed on encode and decode) so redundant copies scatter
//!     across the whole image regardless of payload size.
//!
//! VERIFICATION STATUS: the algorithm above (QIM math, bit packing, permutation,
//! Reed-Solomon chunking) is plain, deterministic Rust with no external unknowns.
//! The one part written against documentation rather than a local compiler is the
//! `ffi` submodule (libjpeg DCT coefficient read/write via mozjpeg-sys, following
//! the standard jpegtran.c coefficient-copy pattern). Build with
//! `cargo build --release --bin stegstr-cli` and run
//! `stegstr-cli embed --robust ...` / `stegstr-cli decode ...` to confirm; if the
//! FFI section fails to compile, the error will be localized to `ffi::read_y_coefficients`
//! / `ffi::write_y_coefficients` and is very likely a small signature mismatch
//! against the exact mozjpeg-sys version resolved by Cargo, not a logic error.

use std::path::Path;

// ---------------------------------------------------------------------------
// Shared constants (mirror dct_stego.py / dct_variants.py)
// ---------------------------------------------------------------------------

const MAGIC: &[u8] = b"STEGSTR";
const MAGIC_LEN: usize = 7;
const LENGTH_BYTES: usize = 4;

/// Standard JPEG zigzag order: zigzag position -> (row, col) in an 8x8 block.
const ZIGZAG_2D: [(usize, usize); 64] = [
    (0, 0), (0, 1), (1, 0), (2, 0), (1, 1), (0, 2), (0, 3), (1, 2), (2, 1), (3, 0),
    (4, 0), (3, 1), (2, 2), (1, 3), (0, 4), (0, 5), (1, 4), (2, 3), (3, 2), (4, 1),
    (5, 0), (6, 0), (5, 1), (4, 2), (3, 3), (2, 4), (1, 5), (0, 6), (0, 7), (1, 6),
    (2, 5), (3, 4), (4, 3), (5, 2), (6, 1), (7, 0), (7, 1), (6, 2), (5, 3), (4, 4),
    (3, 5), (2, 6), (1, 7), (2, 7), (3, 6), (4, 5), (5, 4), (6, 3), (7, 2), (7, 3),
    (6, 4), (5, 5), (4, 6), (3, 7), (4, 7), (5, 6), (6, 5), (7, 4), (7, 5), (6, 6),
    (5, 7), (6, 7), (7, 6), (7, 7),
];
/// Mid-frequency AC zigzag indices used for embedding (skip DC at 0).
const AC_COUNT: usize = 24;

/// Swept empirically against realistic (non-flat) photos; see
/// channel_simulator/sweep_delta.py. Below ~26 the worst-case per-bit error rate
/// on textured/high-frequency content climbs fast; above ~36 returns diminish and
/// visible distortion grows.
const QIM_DELTA: f64 = 32.0;
const QIM_RS_NSYM: usize = 128;
const QIM_REPEAT: usize = 5;
const QIM_EMBED_QUALITY: u8 = 80;
const QIM_HEADER_BITS: usize = 16; // codeword length prefix (u16, up to 65535 bytes)
const QIM_HEADER_REPEAT: usize = 9; // extra margin: losing the header loses the whole payload
const QIM_PERM_SEED: u64 = 20231115;
const QIM_ERASURE_MARGIN: f64 = QIM_DELTA / 6.0;

/// Universal pre-resize width presets. Coefficient-domain embedding cannot survive
/// an actual resize (see module docs), so embed always shrinks the cover to a width
/// no platform in the target set will touch -- never resize based on a guessed
/// destination, since forwarding through a second platform or guessing wrong
/// defeats it entirely.
pub enum Robustness {
    /// Safe for WhatsApp (800), Instagram (1080), Telegram (1280) -- the three
    /// platforms the spec requires surviving. Higher resolution output.
    Standard,
    /// Also safe for Twitter/X-style aggressive downscaling (600). Empirically
    /// 100% pass rate across every platform x cover-type combination tested.
    /// Default: safer with only a modest resolution cost.
    Max,
}

impl Robustness {
    fn max_width(&self) -> u32 {
        match self {
            Robustness::Standard => 768,
            Robustness::Max => 576,
        }
    }
}

impl Default for Robustness {
    fn default() -> Self {
        Robustness::Max
    }
}

// ---------------------------------------------------------------------------
// Bit packing
// ---------------------------------------------------------------------------

fn to_bits(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 8);
    for &b in data {
        for i in (0..8).rev() {
            out.push((b >> i) & 1);
        }
    }
    out
}

fn from_bits(bits: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bits.len() / 8);
    for chunk in bits.chunks(8) {
        if chunk.len() < 8 {
            break;
        }
        let mut byte = 0u8;
        for &bit in chunk {
            byte = (byte << 1) | (bit & 1);
        }
        out.push(byte);
    }
    out
}

/// Concatenate `repeat` full copies of `bits`. Combined with the fixed
/// permutation below, each copy lands in a spatially distant part of the image.
fn interleave_repeat(bits: &[u8], repeat: usize) -> Vec<u8> {
    if repeat <= 1 {
        return bits.to_vec();
    }
    let mut out = Vec::with_capacity(bits.len() * repeat);
    for _ in 0..repeat {
        out.extend_from_slice(bits);
    }
    out
}

/// Inverse of interleave_repeat: `bits`/`margins` is `repeat` concatenated passes
/// of length n; majority-vote per position across passes.
fn deinterleave_majority(bits: &[u8], margins: &[f64], repeat: usize) -> (Vec<u8>, Vec<f64>) {
    if repeat <= 1 {
        return (bits.to_vec(), margins.to_vec());
    }
    let n = bits.len() / repeat;
    let mut out_bits = Vec::with_capacity(n);
    let mut out_margins = Vec::with_capacity(n);
    for i in 0..n {
        let mut ones = 0usize;
        let mut min_margin = f64::INFINITY;
        for p in 0..repeat {
            if bits[i + p * n] == 1 {
                ones += 1;
            }
            min_margin = min_margin.min(margins[i + p * n]);
        }
        out_bits.push(if ones > repeat / 2 { 1 } else { 0 });
        out_margins.push(min_margin);
    }
    (out_bits, out_margins)
}

// ---------------------------------------------------------------------------
// Fixed pseudorandom permutation (spatial decorrelation)
// ---------------------------------------------------------------------------

/// splitmix64: small, fully-specified, dependency-free PRNG. Deliberately NOT
/// using the `rand` crate here: its internal algorithm can change across major
/// versions, which would silently break decode compatibility for images embedded
/// by an older encoder build. This format only ever needs to agree with itself.
fn splitmix64_next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// A single deterministic pseudorandom shuffle of every coefficient position in
/// the image, seeded identically on encode and decode. Consecutive slices of a
/// true random permutation are scattered across the WHOLE image regardless of
/// how small the payload is relative to total capacity -- this is what actually
/// spatially decorrelates the `repeat` copies of each bit (see module docs).
fn fixed_permutation(n: usize) -> Vec<u32> {
    let mut perm: Vec<u32> = (0..n as u32).collect();
    let mut state = QIM_PERM_SEED;
    // Fisher-Yates
    for i in (1..n).rev() {
        let r = splitmix64_next(&mut state);
        let j = (r % (i as u64 + 1)) as usize;
        perm.swap(i, j);
    }
    perm
}

// ---------------------------------------------------------------------------
// QIM embed/detect
// ---------------------------------------------------------------------------

fn qim_embed(x: f64, bit: u8, delta: f64) -> i32 {
    let cell = (x / delta).round() * delta;
    let offset = if bit == 1 { delta / 4.0 } else { -delta / 4.0 };
    (cell + offset).round() as i32
}

fn qim_detect_with_margin(z: f64, delta: f64) -> (u8, f64) {
    let cell = (z / delta).round() * delta;
    let r0 = cell - delta / 4.0;
    let r1 = cell + delta / 4.0;
    let d0 = (z - r0).abs();
    let d1 = (z - r1).abs();
    let bit = if d0 <= d1 { 0 } else { 1 };
    (bit, (d0 - d1).abs())
}

// ---------------------------------------------------------------------------
// Reed-Solomon (GF(256), 255-byte blocks) with manual chunking, matching how
// Python's `reedsolo` auto-chunks messages longer than one RS block.
// ---------------------------------------------------------------------------

mod rs {
    use reed_solomon::{Decoder, Encoder};

    const BLOCK: usize = 255;

    pub fn encode(data: &[u8], nsym: usize) -> Vec<u8> {
        let enc = Encoder::new(nsym);
        let data_per_chunk = BLOCK - nsym;
        let mut out = Vec::with_capacity(data.len() + data.len() / data_per_chunk.max(1) * nsym + nsym);
        if data.is_empty() {
            let encoded = enc.encode(&[]);
            out.extend_from_slice(&encoded);
            return out;
        }
        for chunk in data.chunks(data_per_chunk) {
            let encoded = enc.encode(chunk);
            out.extend_from_slice(&encoded);
        }
        out
    }

    /// `erasure_positions` are byte offsets into the (post-encode) codeword.
    pub fn decode(codeword: &[u8], nsym: usize, erasure_positions: &[usize]) -> Result<Vec<u8>, String> {
        let dec = Decoder::new(nsym);
        let mut out = Vec::with_capacity(codeword.len());
        for (chunk_idx, chunk) in codeword.chunks(BLOCK).enumerate() {
            let base = chunk_idx * BLOCK;
            let local_erasures: Vec<u8> = erasure_positions
                .iter()
                .filter(|&&p| p >= base && p < base + chunk.len())
                .map(|&p| (p - base) as u8)
                .collect();
            let mut buf = chunk.to_vec();
            let erasures_opt = if local_erasures.is_empty() { None } else { Some(&local_erasures[..]) };
            let recovered = dec
                .correct(&mut buf, erasures_opt)
                .map_err(|e| format!("RS decode failed: {:?}", e))?;
            out.extend_from_slice(recovered.data());
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// Payload framing: MAGIC + u32 length + payload -> RS codeword -> u16 codeword
// length header. Mirrors dct_stego.py's _wrap_payload / _unwrap_payload.
// ---------------------------------------------------------------------------

fn wrap_payload(payload: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(MAGIC_LEN + LENGTH_BYTES + payload.len());
    raw.extend_from_slice(MAGIC);
    raw.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    raw.extend_from_slice(payload);
    rs::encode(&raw, QIM_RS_NSYM)
}

fn unwrap_payload(codeword: &[u8], erasures: &[usize]) -> Option<Vec<u8>> {
    let decoded = rs::decode(codeword, QIM_RS_NSYM, erasures).ok()?;
    if decoded.len() < MAGIC_LEN + LENGTH_BYTES || &decoded[..MAGIC_LEN] != MAGIC {
        return None;
    }
    let plen = u32::from_be_bytes(decoded[MAGIC_LEN..MAGIC_LEN + LENGTH_BYTES].try_into().ok()?) as usize;
    let start = MAGIC_LEN + LENGTH_BYTES;
    let end = start.checked_add(plen)?;
    decoded.get(start..end).map(|s| s.to_vec())
}

// ---------------------------------------------------------------------------
// Coefficient stream: (block_row, block_col, ac_zigzag_index) for every 8x8
// luma block, mid-frequency AC positions only. Same order used by encode/decode.
// ---------------------------------------------------------------------------

fn coeff_stream(blocks_wide: usize, blocks_high: usize) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::with_capacity(blocks_wide * blocks_high * AC_COUNT);
    for by in 0..blocks_high {
        for bx in 0..blocks_wide {
            for zi in 0..AC_COUNT {
                out.push((by, bx, zi));
            }
        }
    }
    out
}

fn zigzag_rc(zi_offset_by_one: usize) -> (usize, usize) {
    // AC index k (1-based within the 24 mid-frequency positions) maps to
    // zigzag position k (skipping DC at 0), same as Python's AC_INDICES = 1..=24.
    ZIGZAG_2D[zi_offset_by_one + 1]
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Embed `payload` into `cover_path` (any image format the `image` crate reads),
/// returning JPEG bytes. See module docs for the robustness rationale.
pub fn encode(cover_path: &Path, payload: &[u8], robustness: Robustness) -> Result<Vec<u8>, String> {
    let img = image::open(cover_path).map_err(|e| e.to_string())?.to_rgb8();
    let max_w = robustness.max_width();
    let img = if img.width() > max_w {
        let ratio = max_w as f64 / img.width() as f64;
        let new_h = ((img.height() as f64) * ratio).round().max(1.0) as u32;
        image::imageops::resize(&img, max_w, new_h, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    let tmp_in = std::env::temp_dir().join(format!("stegstr_qim_in_{}.jpg", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp_in).map_err(|e| e.to_string())?;
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut f, QIM_EMBED_QUALITY);
        enc.encode(&img, img.width(), img.height(), image::ExtendedColorType::Rgb8)
            .map_err(|e| e.to_string())?;
    }

    let result = (|| -> Result<Vec<u8>, String> {
        let mut jpeg = ffi::read_y_coefficients(&tmp_in)?;
        let stream = coeff_stream(jpeg.blocks_wide, jpeg.blocks_high);

        let codeword = wrap_payload(payload);
        let codeword_bits = to_bits(&codeword);
        let header_bits = to_bits(&(codeword.len() as u16).to_be_bytes());

        let perm = fixed_permutation(stream.len());
        let interleaved_header = interleave_repeat(&header_bits, QIM_HEADER_REPEAT);
        let interleaved_codeword = interleave_repeat(&codeword_bits, QIM_REPEAT);
        let needed = interleaved_header.len() + interleaved_codeword.len();
        if needed > perm.len() {
            return Err(format!(
                "Payload too large: need {} coefficients (header+codeword x redundancy), have {}",
                needed,
                perm.len()
            ));
        }

        let write_bit = |jpeg: &mut ffi::YCoefficients, slot: u32, bit: u8| {
            let (by, bx, zi) = stream[slot as usize];
            let (dy, dx) = zigzag_rc(zi);
            let c = jpeg.get(by, bx, dy, dx) as f64;
            let v = qim_embed(c, bit, QIM_DELTA).clamp(-32767, 32767) as i16;
            jpeg.set(by, bx, dy, dx, v);
        };

        for (i, &bit) in interleaved_header.iter().enumerate() {
            write_bit(&mut jpeg, perm[i], bit);
        }
        for (i, &bit) in interleaved_codeword.iter().enumerate() {
            write_bit(&mut jpeg, perm[interleaved_header.len() + i], bit);
        }

        let tmp_out = std::env::temp_dir().join(format!("stegstr_qim_out_{}.jpg", std::process::id()));
        ffi::write_y_coefficients(&tmp_in, &jpeg, &tmp_out)?;
        let bytes = std::fs::read(&tmp_out).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&tmp_out);
        Ok(bytes)
    })();

    let _ = std::fs::remove_file(&tmp_in);
    result
}

/// Extract a payload previously embedded with [`encode`]. Returns `Ok(None)` (not
/// an error) when the image has no valid QIM payload, so callers can fall back to
/// trying the DWT decoder -- a plain, non-Stegstr JPEG is an expected input, not a
/// failure.
pub fn decode(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let jpeg = match ffi::read_y_coefficients(path) {
        Ok(j) => j,
        Err(_) => return Ok(None), // not a readable JPEG at all
    };
    let stream = coeff_stream(jpeg.blocks_wide, jpeg.blocks_high);
    let perm = fixed_permutation(stream.len());

    let read_bit = |slot: u32| -> (u8, f64) {
        let (by, bx, zi) = stream[slot as usize];
        let (dy, dx) = zigzag_rc(zi);
        let c = jpeg.get(by, bx, dy, dx) as f64;
        qim_detect_with_margin(c, QIM_DELTA)
    };

    let n_header_slots = QIM_HEADER_BITS * QIM_HEADER_REPEAT;
    if n_header_slots > perm.len() {
        return Ok(None);
    }
    let mut header_bits_raw = Vec::with_capacity(n_header_slots);
    let mut header_margins_raw = Vec::with_capacity(n_header_slots);
    for i in 0..n_header_slots {
        let (bit, margin) = read_bit(perm[i]);
        header_bits_raw.push(bit);
        header_margins_raw.push(margin);
    }
    let (header_bits, _) = deinterleave_majority(&header_bits_raw, &header_margins_raw, QIM_HEADER_REPEAT);
    if header_bits.len() < QIM_HEADER_BITS {
        return Ok(None);
    }
    let header_bytes = from_bits(&header_bits[..QIM_HEADER_BITS]);
    if header_bytes.len() < 2 {
        return Ok(None);
    }
    let codeword_len = u16::from_be_bytes([header_bytes[0], header_bytes[1]]) as usize;
    let n_codeword_bits = codeword_len * 8;
    let needed = n_header_slots + n_codeword_bits * QIM_REPEAT;
    if codeword_len == 0 || needed > perm.len() {
        return Ok(None);
    }

    let mut codeword_bits_raw = Vec::with_capacity(n_codeword_bits * QIM_REPEAT);
    let mut codeword_margins_raw = Vec::with_capacity(n_codeword_bits * QIM_REPEAT);
    for i in n_header_slots..needed {
        let (bit, margin) = read_bit(perm[i]);
        codeword_bits_raw.push(bit);
        codeword_margins_raw.push(margin);
    }
    let (codeword_bits, bit_margins) = deinterleave_majority(&codeword_bits_raw, &codeword_margins_raw, QIM_REPEAT);
    let codeword = from_bits(&codeword_bits);

    let mut erasures = Vec::new();
    for idx in 0..codeword_len {
        let start = idx * 8;
        let end = start + 8;
        if end > bit_margins.len() {
            break;
        }
        let min_margin = bit_margins[start..end].iter().cloned().fold(f64::INFINITY, f64::min);
        if min_margin < QIM_ERASURE_MARGIN {
            erasures.push(idx);
        }
    }

    Ok(unwrap_payload(&codeword, &erasures))
}

// ---------------------------------------------------------------------------
// libjpeg DCT coefficient FFI. Isolated here so a signature mismatch against the
// resolved mozjpeg-sys version stays localized -- everything above this line is
// plain deterministic Rust with no external unknowns. Follows the standard
// jpegtran.c coefficient-copy pattern (jpeg_read_coefficients ->
// access_virt_barray for in-place mutation of the Y component only ->
// jpeg_copy_critical_parameters -> jpeg_write_coefficients), which preserves
// quantization tables and the Cb/Cr planes byte-for-byte unchanged.
// ---------------------------------------------------------------------------
mod ffi {
    use super::Path;
    use mozjpeg_sys::*;
    use std::ffi::CString;
    use std::os::raw::c_char;

    /// Owned copy of the Y-plane DCT coefficients plus enough layout info to
    /// address (block_row, block_col, coeff_row, coeff_col).
    pub struct YCoefficients {
        pub blocks_wide: usize,
        pub blocks_high: usize,
        data: Vec<[i16; 64]>,
    }

    impl YCoefficients {
        pub fn get(&self, by: usize, bx: usize, dy: usize, dx: usize) -> i16 {
            self.data[by * self.blocks_wide + bx][dy * 8 + dx]
        }
        pub fn set(&mut self, by: usize, bx: usize, dy: usize, dx: usize, v: i16) {
            self.data[by * self.blocks_wide + bx][dy * 8 + dx] = v;
        }
    }

    // libjpeg's JMSG_LENGTH_MAX (a C #define, not exported by bindgen). The
    // compiler's expected-type error against format_message's actual generated
    // signature is ground truth here, not the C header's nominal value.
    const JMSG_LENGTH_MAX: usize = 80;

    unsafe extern "C-unwind" fn error_exit(cinfo: &mut jpeg_common_struct) {
        let buf = [0u8; JMSG_LENGTH_MAX];
        if let Some(fmt) = (*cinfo.err).format_message {
            fmt(cinfo, &buf);
        }
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        let msg = String::from_utf8_lossy(&buf[..end]).into_owned();
        panic!("libjpeg error: {}", msg);
    }

    fn path_to_cstring(p: &Path) -> Result<CString, String> {
        CString::new(p.to_string_lossy().as_bytes()).map_err(|e| e.to_string())
    }

    /// Read just the Y-plane coefficients into an owned Rust structure. Does NOT
    /// need to keep the libjpeg session alive afterward (see write_y_coefficients,
    /// which re-opens the same file for the actual in-place-mutation write path).
    pub fn read_y_coefficients(path: &Path) -> Result<YCoefficients, String> {
        std::panic::catch_unwind(|| read_y_coefficients_inner(path))
            .map_err(|_| "libjpeg error while reading coefficients".to_string())?
    }

    fn read_y_coefficients_inner(path: &Path) -> Result<YCoefficients, String> {
        unsafe {
            let mut err: jpeg_error_mgr = std::mem::zeroed();
            jpeg_std_error(&mut err);
            err.error_exit = Some(error_exit);

            let mut cinfo: jpeg_decompress_struct = std::mem::zeroed();
            cinfo.common.err = &mut err;
            jpeg_create_decompress(&mut cinfo);

            let c_path = path_to_cstring(path)?;
            let mode = CString::new("rb").unwrap();
            let fp = libc_fopen(c_path.as_ptr(), mode.as_ptr());
            if fp.is_null() {
                jpeg_destroy_decompress(&mut cinfo);
                return Err(format!("cannot open {}", path.display()));
            }
            jpeg_stdio_src(&mut cinfo, fp as *mut _);
            jpeg_read_header(&mut cinfo, true as boolean);
            let coef_arrays = jpeg_read_coefficients(&mut cinfo);
            if coef_arrays.is_null() {
                jpeg_destroy_decompress(&mut cinfo);
                libc_fclose(fp);
                return Err("jpeg_read_coefficients returned null".to_string());
            }

            let comp = &*cinfo.comp_info; // component 0 = Y for standard JFIF ordering
            let blocks_wide = comp.width_in_blocks as usize;
            let blocks_high = comp.height_in_blocks as usize;
            let mut data = vec![[0i16; 64]; blocks_wide * blocks_high];

            let access = (*cinfo.common.mem).access_virt_barray.expect("access_virt_barray missing");
            let y_array = *coef_arrays; // coefficient array for component 0
            let v_samp = comp.v_samp_factor.max(1) as u32;
            let mut blk_y: u32 = 0;
            while blk_y < blocks_high as u32 {
                let rows = v_samp.min(blocks_high as u32 - blk_y);
                let buffer = access(&mut cinfo.common, y_array, blk_y, rows, false as boolean);
                for offset_y in 0..rows as usize {
                    let row_ptr = *buffer.add(offset_y);
                    for bx in 0..blocks_wide {
                        let block = *row_ptr.add(bx);
                        data[(blk_y as usize + offset_y) * blocks_wide + bx] = block;
                    }
                }
                blk_y += rows;
            }

            jpeg_destroy_decompress(&mut cinfo);
            libc_fclose(fp);
            Ok(YCoefficients { blocks_wide, blocks_high, data })
        }
    }

    /// Re-open `src_path` (the same file `read_y_coefficients` was called on),
    /// overwrite the Y-plane blocks with `modified`, and write the result to
    /// `dst_path`. Cb/Cr planes and quantization tables pass through unchanged
    /// via jpeg_copy_critical_parameters + reusing the same coef_arrays pointer.
    pub fn write_y_coefficients(src_path: &Path, modified: &YCoefficients, dst_path: &Path) -> Result<(), String> {
        let src_path = src_path.to_path_buf();
        let dst_path = dst_path.to_path_buf();
        let blocks_wide = modified.blocks_wide;
        let blocks_high = modified.blocks_high;
        let data = modified.data.clone();
        std::panic::catch_unwind(move || {
            write_y_coefficients_inner(&src_path, blocks_wide, blocks_high, &data, &dst_path)
        })
        .map_err(|_| "libjpeg error while writing coefficients".to_string())?
    }

    fn write_y_coefficients_inner(
        src_path: &Path,
        blocks_wide: usize,
        blocks_high: usize,
        data: &[[i16; 64]],
        dst_path: &Path,
    ) -> Result<(), String> {
        unsafe {
            let mut src_err: jpeg_error_mgr = std::mem::zeroed();
            jpeg_std_error(&mut src_err);
            src_err.error_exit = Some(error_exit);
            let mut dst_err: jpeg_error_mgr = std::mem::zeroed();
            jpeg_std_error(&mut dst_err);
            dst_err.error_exit = Some(error_exit);

            let mut srcinfo: jpeg_decompress_struct = std::mem::zeroed();
            srcinfo.common.err = &mut src_err;
            jpeg_create_decompress(&mut srcinfo);
            let mut dstinfo: jpeg_compress_struct = std::mem::zeroed();
            dstinfo.common.err = &mut dst_err;
            jpeg_create_compress(&mut dstinfo);

            let src_c = path_to_cstring(src_path)?;
            let rb = CString::new("rb").unwrap();
            let in_fp = libc_fopen(src_c.as_ptr(), rb.as_ptr());
            if in_fp.is_null() {
                return Err(format!("cannot reopen {}", src_path.display()));
            }
            jpeg_stdio_src(&mut srcinfo, in_fp as *mut _);
            jpeg_read_header(&mut srcinfo, true as boolean);
            let coef_arrays = jpeg_read_coefficients(&mut srcinfo);
            if coef_arrays.is_null() {
                return Err("jpeg_read_coefficients returned null".to_string());
            }

            // Mutate component 0 (Y) in place with our modified coefficients.
            let comp = &*srcinfo.comp_info;
            let access = (*srcinfo.common.mem).access_virt_barray.expect("access_virt_barray missing");
            let y_array = *coef_arrays;
            let v_samp = comp.v_samp_factor.max(1) as u32;
            let mut blk_y: u32 = 0;
            while blk_y < blocks_high as u32 {
                let rows = v_samp.min(blocks_high as u32 - blk_y);
                let buffer = access(&mut srcinfo.common, y_array, blk_y, rows, true as boolean);
                for offset_y in 0..rows as usize {
                    let row_ptr = *buffer.add(offset_y);
                    for bx in 0..blocks_wide {
                        let block_ptr = row_ptr.add(bx);
                        *block_ptr = data[(blk_y as usize + offset_y) * blocks_wide + bx];
                    }
                }
                blk_y += rows;
            }

            let dst_c = path_to_cstring(dst_path)?;
            let wb = CString::new("wb").unwrap();
            let out_fp = libc_fopen(dst_c.as_ptr(), wb.as_ptr());
            if out_fp.is_null() {
                return Err(format!("cannot open {} for writing", dst_path.display()));
            }
            jpeg_stdio_dest(&mut dstinfo, out_fp as *mut _);
            jpeg_copy_critical_parameters(&mut srcinfo, &mut dstinfo);
            jpeg_write_coefficients(&mut dstinfo, coef_arrays);
            jpeg_finish_compress(&mut dstinfo);
            jpeg_destroy_compress(&mut dstinfo);
            libc_fclose(out_fp);

            jpeg_finish_decompress(&mut srcinfo);
            jpeg_destroy_decompress(&mut srcinfo);
            libc_fclose(in_fp);

            Ok(())
        }
    }

    // libjpeg's stdio source/dest managers want a real C FILE*; go through libc
    // directly rather than pulling in a whole extra crate for two functions.
    extern "C" {
        #[link_name = "fopen"]
        fn libc_fopen(path: *const c_char, mode: *const c_char) -> *mut std::ffi::c_void;
        #[link_name = "fclose"]
        fn libc_fclose(f: *mut std::ffi::c_void) -> i32;
    }
}
