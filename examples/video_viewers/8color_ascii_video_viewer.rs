//! ASCII видео-плеер с 8-цветной палитрой ANSI и дизерингом Байера.
//! Оптимизирован: LUT (5-битный квантователь) строится один раз,
//! рендер кадра — только квантование + lookup + запись байт.
//!
//! Требует ffmpeg и ffplay в PATH.
//!
//! Использование:
//!     cargo run --release --example ascii-video-8 -- путь/к/видео.mp4

use std::io::{self, ErrorKind, Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

const TARGET_FPS: u32 = 30;

// ---- 8 базовых цветов ANSI ----
const ANSI_PALETTE_8: [[u8; 3]; 8] = [
    [0, 0, 0],       // 0 Black
    [128, 0, 0],     // 1 Red
    [0, 128, 0],     // 2 Green
    [128, 128, 0],   // 3 Yellow
    [0, 0, 128],     // 4 Blue
    [128, 0, 128],   // 5 Magenta
    [0, 128, 128],   // 6 Cyan
    [192, 192, 192], // 7 White
];

// Варианты: (индекс символа, доля fg, инвертировать?)
// ch_idx: 0=' ', 1='░', 2='▒', 3='█'
const VARIANTS: [(u8, f32, bool); 5] = [
    (0, 0.0,  false), // ' '
    (1, 0.25, false), // '░'
    (2, 0.5,  false), // '▒'
    (1, 0.25, true),  // '░' с обменом fg/bg = 75% текста
    (3, 1.0,  false), // '█'
];

// Готовые UTF-8 байты символов.
const CHARS: [&[u8]; 4] = [
    b" ",
    b"\xe2\x96\x91", // ░ U+2591
    b"\xe2\x96\x92", // ▒ U+2592
    b"\xe2\x96\x88", // █ U+2588
];

// Готовые ANSI SGR последовательности.
const FG_ESCAPES: [&[u8]; 8] = [
    b"\x1b[30m", b"\x1b[31m", b"\x1b[32m", b"\x1b[33m",
    b"\x1b[34m", b"\x1b[35m", b"\x1b[36m", b"\x1b[37m",
];
const BG_ESCAPES: [&[u8]; 8] = [
    b"\x1b[40m", b"\x1b[41m", b"\x1b[42m", b"\x1b[43m",
    b"\x1b[44m", b"\x1b[45m", b"\x1b[46m", b"\x1b[47m",
];

// Предвычисленные смещения Байера: (bayer/16 - 0.5) * 10.
// Dithering работает как порог перед квантованием.
const BAYER_OFFSET: [[f32; 4]; 4] = [
    [-5.000,  0.000, -3.750,  1.250],
    [ 2.500, -2.500,  3.750, -1.250],
    [-3.125,  1.875, -4.375,  0.625],
    [ 4.375, -0.625,  3.125, -1.875],
];

// ---- LUT ----
// 5 бит на канал = 32 уровня, 32^3 = 32768 ячеек по 1 байту = 32 КБ.
// Шаг 8 на канал: dithering (±5) перекрывает больше половины шага —
// работает, но LUT-строительство занимает копейки даже в debug.
const LUT_BITS: usize = 5;
const LUT_SIZE: usize = 1 << LUT_BITS;
const LUT_LEN: usize = LUT_SIZE * LUT_SIZE * LUT_SIZE;
const LUT_SHIFT: usize = 8 - LUT_BITS; // 3

/// Упаковка (fg, bg, ch_idx) в один u8.
#[inline(always)]
fn pack(fg: u8, bg: u8, ch_idx: u8) -> u8 {
    (fg & 7) | ((bg & 7) << 3) | ((ch_idx & 3) << 6)
}

#[derive(Clone, Copy)]
struct Entry {
    r: i32,
    g: i32,
    b: i32,
    packed: u8,
}

/// Строим LUT один раз при старте.
fn build_lut() -> Vec<u8> {
    // 1. Перебираем все комбинации (fg, bg, variant) и квантуем результат
    //    в i32-каналы. Дубликаты по (r,g,b) отбрасываем — при равенстве
    //    цвета первый (по порядку VARIANTS) вариант предпочтительнее,
    //    так как даёт более "чистый" символ.
    let mut entries: Vec<Entry> = Vec::with_capacity(200);
    let mut seen = std::collections::HashSet::new();

    for bg in 0u8..8 {
        for fg in 0u8..8 {
            for &(ch_idx, ratio, swap) in &VARIANTS {
                let (fg_a, bg_a) = if swap { (bg, fg) } else { (fg, bg) };
                let fc = ANSI_PALETTE_8[fg_a as usize];
                let bc = ANSI_PALETTE_8[bg_a as usize];

                let r = (fc[0] as f32 * ratio + bc[0] as f32 * (1.0 - ratio)).round() as i32;
                let g = (fc[1] as f32 * ratio + bc[1] as f32 * (1.0 - ratio)).round() as i32;
                let b = (fc[2] as f32 * ratio + bc[2] as f32 * (1.0 - ratio)).round() as i32;

                if seen.insert((r, g, b)) {
                    entries.push(Entry { r, g, b, packed: pack(fg_a, bg_a, ch_idx) });
                }
            }
        }
    }

    // 2. Заполняем LUT: для каждой 5-битной ячейки — ближайший достижимый цвет.
    let mut lut = vec![0u8; LUT_LEN];
    let scale = 255.0 / (LUT_SIZE - 1) as f32;

    for ri in 0..LUT_SIZE {
        let r = (ri as f32 * scale).round() as i32;
        for gi in 0..LUT_SIZE {
            let g = (gi as f32 * scale).round() as i32;
            for bi in 0..LUT_SIZE {
                let b = (bi as f32 * scale).round() as i32;

                let mut best_d = i32::MAX;
                let mut best = 0u8;
                for e in &entries {
                    let dr = e.r - r;
                    let dg = e.g - g;
                    let db = e.b - b;
                    let d = dr * dr + dg * dg + db * db;
                    if d < best_d {
                        best_d = d;
                        best = e.packed;
                        if d == 0 {
                            break;
                        }
                    }
                }
                lut[(ri << (2 * LUT_BITS)) | (gi << LUT_BITS) | bi] = best;
            }
        }
    }

    lut
}

// ---- main ----

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let path = match args.get(1) {
        Some(p) => p.clone(),
        None => {
            eprintln!("Usage: ascii-video-8 <video-file>");
            std::process::exit(1);
        }
    };

    let (cols, rows) = terminal_size::terminal_size()
        .map(|(w, h)| (w.0 as usize, h.0 as usize))
        .unwrap_or((80, 24));
    let cols = cols.max(1);
    let rows = rows.max(1);

    // LUT: одна таблица на весь сеанс.
    let lut = build_lut();

    let mut child = spawn_ffmpeg(&path, cols, rows)?;
    let mut ffmpeg_out = child.stdout.take().expect("stdout is piped");

    let mut audio_child = Command::new("ffplay")
        .args([
            "-nodisp",
            "-autoexit",
            "-loglevel", "error",
            "-sync", "audio",
            path.as_str(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()?;

    crossterm::terminal::enable_raw_mode()?;
    let mut out = io::stdout().lock();
    write!(out, "\x1b[?1049h\x1b[?25l\x1b[?7l\x1b[2J")?;
    out.flush()?;

    let result = play(&mut ffmpeg_out, &mut out, cols, rows, &lut);

    let _ = write!(out, "\x1b[0m\x1b[?7h\x1b[?25h\x1b[?1049l");
    let _ = out.flush();
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = child.kill();
    let _ = child.wait();
    let _ = audio_child.kill();
    let _ = audio_child.wait();

    result.map_err(Into::into)
}

fn spawn_ffmpeg(path: &str, cols: usize, rows: usize) -> io::Result<Child> {
    let scale = format!("scale={cols}:{rows}:flags=bilinear");
    Command::new("ffmpeg")
        .args([
            "-i", path,
            "-an",
            "-f", "rawvideo",
            "-pix_fmt", "rgb24",
            "-vf", &scale,
            "-r", &TARGET_FPS.to_string(),
            "-loglevel", "error",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
}

fn play(
    src: &mut impl Read,
    out: &mut impl Write,
    cols: usize,
    rows: usize,
    lut: &[u8],
) -> io::Result<()> {
    let frame_size = cols * rows * 3;
    let mut frame = vec![0u8; frame_size];
    // Резерв под ~4 байта/пиксель в среднем.
    let mut render_buf: Vec<u8> = Vec::with_capacity(cols * rows * 6 + 16);
    let frame_dur = Duration::from_micros(1_000_000 / TARGET_FPS as u64);
    let mut last_frame = Instant::now();

    loop {
        while event::poll(Duration::from_millis(0))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    let quit = matches!(k.code, KeyCode::Esc | KeyCode::Char('q'))
                        || (matches!(k.code, KeyCode::Char('c'))
                            && k.modifiers.contains(KeyModifiers::CONTROL));
                    if quit {
                        return Ok(());
                    }
                }
            }
        }

        match src.read_exact(&mut frame) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(e),
        }

        render_frame(&frame, cols, rows, lut, &mut render_buf);
        out.write_all(&render_buf)?;
        out.flush()?;

        let elapsed = last_frame.elapsed();
        if elapsed < frame_dur {
            std::thread::sleep(frame_dur - elapsed);
        }
        last_frame = Instant::now();
    }
}

fn render_frame(frame: &[u8], cols: usize, rows: usize, lut: &[u8], out: &mut Vec<u8>) {
    out.clear();
    out.extend_from_slice(b"\x1b[H");

    let mut cur_fg: u8 = u8::MAX;
    let mut cur_bg: u8 = u8::MAX;

    for y in 0..rows {
        let dith_row = &BAYER_OFFSET[y & 3];
        let row_base = y * cols * 3;

        for x in 0..cols {
            let off = dith_row[x & 3];
            let i = row_base + x * 3;

            // Дизеринг + квантование к 5 битам.
            let r = ((frame[i] as f32 + off).clamp(0.0, 255.0) as usize) >> LUT_SHIFT;
            let g = ((frame[i + 1] as f32 + off).clamp(0.0, 255.0) as usize) >> LUT_SHIFT;
            let b = ((frame[i + 2] as f32 + off).clamp(0.0, 255.0) as usize) >> LUT_SHIFT;

            let idx = (r << (2 * LUT_BITS)) | (g << LUT_BITS) | b;
            let packed = lut[idx];
            let fg = packed & 7;
            let bg = (packed >> 3) & 7;
            let ch_idx = ((packed >> 6) & 3) as usize;

            // Экономим байты: escape-последовательность — только при смене цвета.
            if fg != cur_fg {
                out.extend_from_slice(FG_ESCAPES[fg as usize]);
                cur_fg = fg;
            }
            if bg != cur_bg {
                out.extend_from_slice(BG_ESCAPES[bg as usize]);
                cur_bg = bg;
            }
            out.extend_from_slice(CHARS[ch_idx]);
        }
        if y + 1 < rows {
            out.extend_from_slice(b"\r\n");
        }
    }
}