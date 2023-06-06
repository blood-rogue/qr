use image::{ImageBuffer, RgbImage, Pixel, ImageFormat};
use reed_solomon::Encoder;

use crate::utils::*;

pub fn encode(data: String) -> Pixels {
    let mut modules = Vec::new();
    modules.extend_from_slice(&MODE);
    modules.extend_from_slice(&pad_left(u8_to_bits(data.len() as u8).to_vec(), 0, 9));

    let chunks = data.as_bytes().chunks_exact(2);

    for chunk in chunks.clone() {
        let n1 = ALPHAS.get(&char::from_u32(chunk[0] as u32).unwrap()).unwrap();
        let n2 = ALPHAS.get(&char::from_u32(chunk[1] as u32).unwrap()).unwrap();

        let n = (n1 * 45) + n2;
        modules.extend_from_slice(&u16_to_bits(n)[5..]);
    }

    if !chunks.remainder().is_empty() {
        let bits = u8_to_bits(*ALPHAS.get(&char::from_u32(chunks.remainder()[0] as u32).unwrap()).unwrap() as u8);
        modules.extend_from_slice(&bits[2..])
    }

    let n = modules.len();

    match n {
        0..=859 => pad_right_mut(&mut modules, 0, (n + 11) & !7),
        860..=864 => pad_right_mut(&mut modules, 0, 864 - n),
        _ => {}
    }

    if modules.len() < 864 {
        let n1 = u8_to_bits(236);
        let n2 = u8_to_bits(17);

        let n = (864 - modules.len()) / 8;
        let odd = n % 2 != 0;

        for _ in (0..(n/2)*2).step_by(2) {
            modules.extend_from_slice(&n1);
            modules.extend_from_slice(&n2);
        }

        if odd {
            modules.extend_from_slice(&n1);
        }
    }

    let coeffs: Vec<_> = modules.chunks_exact(8).map(|chunk| bits_to_u8(chunk.try_into().unwrap())).collect();

    let enc = Encoder::new(26);
    let data = enc.encode(coeffs.as_slice());

    modules = Vec::new();
    for &coeff in &data[..] {
        modules.extend_from_slice(&u8_to_bits(coeff));
    }

    modules.extend_from_slice(&[0; 7]);

    let mut pixels = [[false; PIXELS]; PIXELS];

    set_finder_patterns(&mut pixels);

    for (&(x, y), &bit) in POS.iter().zip(modules.iter()) {
        pixels[y][x] = if bit == 1 { true } else { false }
    }

    let mut masks = [[[true; PIXELS]; PIXELS]; 8];

    masks[0] = set_mask(pixels, |x, y| (x + y) % 2 == 0);
    masks[1] = set_mask(pixels, |_, y| y % 2 == 0);
    masks[2] = set_mask(pixels, |x, _| x % 3 == 0);
    masks[3] = set_mask(pixels, |x, y| (x + y) % 3 == 0);
    masks[4] = set_mask(pixels, |x, y| (y / 2 + x / 3) % 2 == 0);
    masks[5] = set_mask(pixels, |x, y| (((x * y) % 2) + ((x * y) % 3)) == 0);
    masks[6] = set_mask(pixels, |x, y| (((x * y) % 2) + ((x * y) % 3)) % 2 == 0);
    masks[7] = set_mask(pixels, |x, y| (((x + y) % 2) + ((x * y) % 3)) % 2 == 0);

    let mut penalties = [0usize; 8];
    for i in 0..8 {
        penalties[i] = calc_penalty(masks[i]);
    }

    let min_penalty_idx = penalties.iter().enumerate().min_by(|(_, a), (_, b)| a.cmp(b)).map(|(index, _)| index).unwrap();

    pixels = masks[min_penalty_idx];

    set_format_bits(&mut pixels, min_penalty_idx);

    pixels
}

fn set_finder_patterns(pixels: &mut Pixels) {
    for i in 0..7 {
        pixels[0][i] = true;
        pixels[0][PIXELS - 1 - i] = true;

        pixels[6][i] = true;
        pixels[6][PIXELS - 1 - i] = true;

        pixels[PIXELS - 7][i] = true;
        pixels[PIXELS - 1][i] = true;
    }

    for i in 0..5 {
        pixels[1 + i][0] = true;
        pixels[1 + i][6] = true;
        pixels[1 + i][PIXELS - 7] = true;
        pixels[1 + i][PIXELS - 1] = true;
        pixels[31 + i][0] = true;
        pixels[31 + i][6] = true;
        pixels[28][28 + i] = true;
        pixels[28 + i][28] = true;
        pixels[32][32 - i] = true;
        pixels[32 - i][32] = true;
    }

    for i in 8..29 {
        if i % 2 == 0 {
            pixels[i][6] = true;
            pixels[6][i] = true;
        }
    }

    for i in 0..3 {
        for j in 0..3 {
            pixels[2 + j][2 + i] = true;
            pixels[2 + j][32 + i] = true;
            pixels[32 + j][2 + i] = true;
        }
    }

    pixels[29][8] = true;
    pixels[PIXELS - 7][PIXELS - 7] = true;
}

fn set_mask<F>(pixels: Pixels, f: F) -> Pixels
where F: Fn(usize, usize) -> bool
{
    let mut out = pixels;
    for (x, y) in POS {
        if f(x, y) {
            out[y][x] = !pixels[y][x];
        }
    }

    out
}

fn set_format_bits(pixels: &mut Pixels, mask_id: usize) {
    let format_bits = MASK_FORMAT_STRINGS[mask_id];

    for ((&(x1, y1), &(x2, y2)), &bit) in FORMAT_POS_1.iter().zip(FORMAT_POS_2.iter()).zip(format_bits.iter()) {
        if bit == 1 {
            pixels[y1][x1] = true;
            pixels[y2][x2] = true;
        } else {
            pixels[y1][x1] = false;
            pixels[y2][x2] = false;
        }
    }
}

fn rotate(pixels: Pixels) -> Pixels {
    let mut out_pixels = [[false; PIXELS]; PIXELS];

    for i in 0..PIXELS {
        for j in 0..PIXELS {
            out_pixels[i][j] = pixels[j][i]
        }
    }

    out_pixels
}

#[derive(Debug)]
struct Penalty {
    horiz_run_penalty: usize,
    vert_run_penalty: usize,
    box_penalty: usize,
    finder_penalty: usize,
    dark_penalty: usize
}

impl Penalty {
    fn finalize(self) -> usize {
        self.box_penalty + self.dark_penalty + self.finder_penalty + self.horiz_run_penalty + self.vert_run_penalty
    }
}

fn calc_penalty(pixels: Pixels) -> usize {
    let mut penalty = Penalty { horiz_run_penalty: 0, vert_run_penalty: 0, box_penalty: 0, finder_penalty: 0, dark_penalty: 0 };

    for row in pixels {
        let mut recent = row[0];
        penalty.horiz_run_penalty += row.iter()
            .fold(vec![1usize], |acc, &bit| {
                let mut ret = acc;

                if bit == recent {
                    *ret.last_mut().unwrap() += 1;
                } else {
                    recent = bit;
                    ret.push(1)
                }

                ret
            }).iter()
            .filter(|&&num_same| num_same >= 5)
            .map(|num_same| num_same - 2)
            .fold(0, |acc, penalty| acc + penalty);
    }

    for col in rotate(pixels) {
        let mut recent = col[0];
        penalty.vert_run_penalty += col.iter()
            .fold(vec![1usize], |acc, &bit| {
                let mut ret = acc;

                if bit == recent {
                    *ret.last_mut().unwrap() += 1;
                } else {
                    recent = bit;
                    ret.push(1)
                }

                ret
            }).iter()
            .filter(|&&num_same| num_same >= 5)
            .map(|num_same| num_same - 2)
            .fold(0, |acc, penalty| acc + penalty);
    }

    for x in 0..PIXELS-1 {
        for y in 0..PIXELS-1 {
            if pixels[y][x] == pixels[y][x+1] && pixels[y][x+1] == pixels[y+1][x] && pixels[y+1][x] == pixels[y+1][x+1] {
                penalty.box_penalty += 3
            }
        }
    }

    for row in pixels {
        let row_str = pixel_array_to_str(row);
        if row_str.contains("10111010000") || row_str.contains("00001011101") {
            penalty.finder_penalty += 40
        }
    }

    for col in rotate(pixels) {
        let col_str = pixel_array_to_str(col);
        if col_str.contains("10111010000") || col_str.contains("00001011101") {
            penalty.finder_penalty += 40
        }
    }

    let mut num_dark = 0i32;
    for row in pixels {
        for pixel in row {
            if pixel {
                num_dark += 1;
            }
        }
    }

    let percent = (num_dark / 1369) * 100;
    let lower = (((percent / 5) * 5) - 50).abs();
    let upper = ((((percent / 5) + 1) * 5) - 50).abs();

    penalty.dark_penalty += (lower.min(upper) * 10) as usize;

    penalty.finalize()
}

pub fn to_png(pixels: Pixels, file: String) {
    let mut img: RgbImage = ImageBuffer::from_pixel(410, 410, WHITE.to_rgb());

    for (i, row) in pixels.iter().enumerate() {
        for (j, &bit) in row.iter().enumerate() {
            for x in 0..10 {
                for y in 0..10 {
                    img.put_pixel((20 + (j * 10) + x) as u32, (20 + (i * 10) + y) as u32, (if bit { BLACK } else { WHITE }).to_rgb())
                }
            }
        }
    }

    img.save_with_format(file, ImageFormat::Png).unwrap();
}