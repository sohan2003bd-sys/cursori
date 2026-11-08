//! Cursor Painter — ভারী pixel কাজের Rust/WASM kernels.
//! ছবির ডেটা RGBA (Uint8) আকারে আসে; ফলাফল dst-এ লেখা হয়।
use std::slice;

#[no_mangle]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    Box::into_raw(vec![0u8; len].into_boxed_slice()) as *mut u8
}

#[no_mangle]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, len: usize) {
    drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)));
}

/// Box blur (অনুভূমিক + উল্লম্ব, sliding window)। শুধু RGB; alpha = 0 (আগের JS-এর মতোই)।
#[no_mangle]
pub unsafe extern "C" fn blur_rgb(src: *const u8, dst: *mut u8, w: usize, h: usize, r: usize) {
    let s = slice::from_raw_parts(src, w * h * 4);
    let d = slice::from_raw_parts_mut(dst, w * h * 4);
    let (ri, wi, hi) = (r as isize, w as isize, h as isize);
    let mut t = vec![0f32; w * h * 4];

    // অনুভূমিক ধাপ
    for y in 0..h {
        let row = y * w;
        for c in 0..3 {
            let (mut sum, mut n) = (0f64, 0f64);
            let mut x = 0isize;
            while x <= ri && x < wi {
                sum += s[(row + x as usize) * 4 + c] as f64;
                n += 1.0;
                x += 1;
            }
            for x in 0..wi {
                t[(row + x as usize) * 4 + c] = (sum / n) as f32;
                let (a, b) = (x - ri, x + ri + 1);
                if a >= 0 { sum -= s[(row + a as usize) * 4 + c] as f64; n -= 1.0; }
                if b < wi { sum += s[(row + b as usize) * 4 + c] as f64; n += 1.0; }
            }
        }
    }

    // উল্লম্ব ধাপ: সারি ধরে এগোয় (মেমরি ক্রমানুসারে পড়া, দ্রুত)
    let mut acc = vec![0f64; w * 3];
    let mut n = 0f64;
    let add = |acc: &mut [f64], y: usize, sign: f64| {
        let base = y * w * 4;
        for x in 0..w {
            for c in 0..3 { acc[x * 3 + c] += sign * t[base + x * 4 + c] as f64; }
        }
    };
    let mut y0 = 0isize;
    while y0 <= ri && y0 < hi { add(&mut acc, y0 as usize, 1.0); n += 1.0; y0 += 1; }
    for y in 0..hi {
        let base = y as usize * w * 4;
        for x in 0..w {
            for c in 0..3 { d[base + x * 4 + c] = (acc[x * 3 + c] / n + 0.5) as u8; }
        }
        let (a, b) = (y - ri, y + ri + 1);
        if a >= 0 { add(&mut acc, a as usize, -1.0); n -= 1.0; }
        if b < hi { add(&mut acc, b as usize, 1.0); n += 1.0; }
    }
}

/// কার্টুন রূপের edge-preserving (bilateral) এক ধাপ: কেন্দ্র + ৪ পাশের pixel, `sp` দূরত্বে।
#[no_mangle]
pub unsafe extern "C" fn bilateral(src: *const u8, dst: *mut u8, w: usize, h: usize, sp: usize) {
    let a = slice::from_raw_parts(src, w * h * 4);
    let b = slice::from_raw_parts_mut(dst, w * h * 4);
    let mut lut = [0f32; 766];
    for i in 0..766 { lut[i] = (-((i * i) as f32) / 3200.0).exp(); }
    let sp = sp as isize;
    let taps = [(0isize, 0isize), (sp, 0), (-sp, 0), (0, sp), (0, -sp)];
    let (wi, hi) = (w as isize, h as isize);
    for y in 0..hi {
        for x in 0..wi {
            let i = (y as usize * w + x as usize) * 4;
            let (r0, g0, b0) = (a[i] as i32, a[i + 1] as i32, a[i + 2] as i32);
            let (mut sr, mut sg, mut sb, mut sw) = (0f32, 0f32, 0f32, 0f32);
            for &(dx, dy) in taps.iter() {
                let (xx, yy) = (x + dx, y + dy);
                if xx < 0 || xx >= wi || yy < 0 || yy >= hi { continue; }
                let t = (yy as usize * w + xx as usize) * 4;
                let (tr, tg, tb) = (a[t] as i32, a[t + 1] as i32, a[t + 2] as i32);
                let k = lut[((tr - r0).abs() + (tg - g0).abs() + (tb - b0).abs()) as usize];
                sr += tr as f32 * k; sg += tg as f32 * k; sb += tb as f32 * k; sw += k;
            }
            b[i] = (sr / sw + 0.5) as u8;
            b[i + 1] = (sg / sw + 0.5) as u8;
            b[i + 2] = (sb / sw + 0.5) as u8;
            b[i + 3] = 255;
        }
    }
}
