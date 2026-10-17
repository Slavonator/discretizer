//! ASCII видео-плеер: играет видеофайл в терминале через ANSI 24-bit цвет.
//!
//! Требует ffmpeg в PATH.
//!
//! Использование:
//!     cargo run --example ascii-video -- путь/к/видео.mp4

use std::io::{self, ErrorKind, Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

const RAMP: &[u8] = b":~=+*$#%&@";
const TARGET_FPS: u32 = 30;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let path = match args.get(1) {
        Some(p) => p.clone(),
        None => {
            eprintln!("Usage: ascii-video <video-file>");
            std::process::exit(1);
        }
    };

    let (cols, rows) = terminal_size::terminal_size()
        .map(|(w, h)| (w.0 as usize, h.0 as usize))
        .unwrap_or((80, 24));
    let cols = cols.max(1);
    let rows = rows.max(1);

    let mut child = spawn_ffmpeg(&path, cols, rows)?;
    let mut ffmpeg_out = child.stdout.take().expect("stdout is piped");

    let mut audio_child = Command::new("ffplay")
    .args([
        "-nodisp",          // без графического окна
        "-autoexit",        // завершиться по окончании
        "-loglevel", "error",
        "-sync", "audio",   // синхронизация по аудио
        path.as_str(),
    ])
    .stdout(Stdio::null())
    .stderr(Stdio::inherit())
    .spawn()?;

    crossterm::terminal::enable_raw_mode()?;
    let mut out = io::stdout().lock();
    // alt screen, hide cursor, disable autowrap, clear
    write!(out, "\x1b[?1049h\x1b[?25l\x1b[?7l\x1b[2J")?;
    out.flush()?;

    let result = play(&mut ffmpeg_out, &mut out, cols, rows);

    // cleanup, regardless of result
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
            "-an",                              // без звука
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
) -> io::Result<()> {
    let frame_size = cols * rows * 3;
    let mut frame = vec![0u8; frame_size];
    let mut render_buf = String::with_capacity(cols * rows * 20);
    let frame_dur = Duration::from_micros(1_000_000 / TARGET_FPS as u64);
    let mut last_frame = Instant::now();

    loop {
        // non-blocking input
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

        render_buf.clear();
        render_frame(&frame, cols, rows, &mut render_buf);
        out.write_all(render_buf.as_bytes())?;
        out.flush()?;

        let elapsed = last_frame.elapsed();
        if elapsed < frame_dur {
            std::thread::sleep(frame_dur - elapsed);
        }
        last_frame = Instant::now();
    }
}

fn render_frame(frame: &[u8], cols: usize, rows: usize, out: &mut String) {
    use std::fmt::Write as _;

    out.push_str("\x1b[H"); // cursor home
    let mut cur_color: (u16, u16, u16) = (u16::MAX, u16::MAX, u16::MAX);

    for y in 0..rows {
        for x in 0..cols {
            let i = (y * cols + x) * 3;
            let r = frame[i];
            let g = frame[i + 1];
            let b = frame[i + 2];

            let lum = (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) as usize;
            let idx = lum * (RAMP.len() - 1) / 255;
            let ch = RAMP[idx] as char;

            // emit color escape only when color changes
            let color = (r as u16, g as u16, b as u16);
            if color != cur_color {
                let _ = write!(out, "\x1b[38;2;{r};{g};{b}m");
                cur_color = color;
            }
            out.push(ch);
        }
        if y + 1 < rows {
            out.push_str("\r\n");
        }
    }
}