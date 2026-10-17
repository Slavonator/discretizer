// input.rs
use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossterm::event::{self, Event as CtEvent};

// Реэкспорт, чтобы main.rs не зависел от crossterm напрямую
pub use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
    MouseButton, MouseEvent, MouseEventKind,
};

/// Событие ввода.
#[derive(Clone, Debug)]
pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize { width: u16, height: u16 },
}

/// Источник событий ввода с терминала.
///
/// При создании переводит терминал в raw mode и включает отслеживание мыши.
/// Читает stdin в отдельном потоке, события складывает в канал.
/// При `Drop` восстанавливает терминал.
pub struct Input {
    rx: Receiver<Event>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    enabled: bool,
}

impl Input {
    pub fn new() -> io::Result<Self> {
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(
            io::stdout(),
            crossterm::event::EnableMouseCapture,
        )?;

        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);

        let thread = thread::spawn(move || {
            let poll_timeout = Duration::from_millis(50);
            while !stop_clone.load(Ordering::Relaxed) {
                match event::poll(poll_timeout) {
                    Ok(true) => match event::read() {
                        Ok(CtEvent::Key(k)) => {
                            if tx.send(Event::Key(k)).is_err() { break; }
                        }
                        Ok(CtEvent::Mouse(m)) => {
                            if tx.send(Event::Mouse(m)).is_err() { break; }
                        }
                        Ok(CtEvent::Resize(w, h)) => {
                            if tx.send(Event::Resize { width: w, height: h }).is_err() { break; }
                        }
                        Ok(_) => {}
                        Err(_) => break,
                    },
                    Ok(false) => {}
                    Err(_) => break,
                }
            }
        });

        Ok(Self { rx, stop, thread: Some(thread), enabled: true })
    }

    /// Неблокирующее чтение. `None`, если очередь событий пуста.
    pub fn poll(&self) -> Option<Event> {
        match self.rx.try_recv() {
            Ok(e) => Some(e),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => None,
        }
    }

    /// Блокирующее ожидание события. `None`, если поток ввода завершился.
    pub fn wait(&self) -> Option<Event> {
        self.rx.recv().ok()
    }

    /// Ожидание с таймаутом. Пригодится для event loop без busy-wait.
    pub fn wait_timeout(&self, timeout: Duration) -> Option<Event> {
        self.rx.recv_timeout(timeout).ok()
    }

    /// Выключить raw mode и отслеживание мыши. Идемпотентно.
    pub fn disable(&mut self) {
        if !self.enabled { return; }
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        let _ = crossterm::execute!(
            io::stdout(),
            crossterm::event::DisableMouseCapture,
        );
        let _ = crossterm::terminal::disable_raw_mode();
        self.enabled = false;
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        self.disable();
    }
}


/// Виртуальный экран терминала: альтернативный буфер + двойная буферизация с диффом.
///
/// Пока `Screen` жив, терминал работает в альтернативном режиме — то, что мы
/// печатаем, не уходит в scrollback основного экрана. При `Drop` (или явном
/// `leave`) терминал возвращается в исходное состояние.
pub struct Screen {
    width: usize,
    height: usize,
    front: Vec<char>,
    back: Vec<char>,
    entered: bool,
}

impl Screen {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            front: Vec::new(),
            back: Vec::new(),
            entered: false,
        }
    }

    /// Войти в альтернативный экран, спрятать курсор, очистить экран.
    /// Идемпотентно.
    pub fn enter(&mut self) -> io::Result<()> {
        if self.entered {
            return Ok(());
        }
        let mut out = io::stdout().lock();
        out.write_all(b"\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H")?;
        out.flush()?;
        self.entered = true;
        Ok(())
    }

    /// Выйти из альтернативного экрана, вернуть курсор.
    /// Идемпотентно.
    pub fn leave(&mut self) -> io::Result<()> {
        if !self.entered {
            return Ok(());
        }
        let mut out = io::stdout().lock();
        out.write_all(b"\x1b[?25h\x1b[?1049l")?;
        out.flush()?;
        self.entered = false;
        Ok(())
    }

    pub fn width(&self) -> usize { self.width }
    pub fn height(&self) -> usize { self.height }

    pub fn resize(&mut self, width: usize, height: usize) {
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;
        let n = width * height;
        self.front = vec![' '; n];
        self.back = vec![' '; n];
    }

    pub fn clear_back(&mut self, fill: char) {
        self.back.fill(fill);
    }

    pub fn back_mut(&mut self) -> &mut [char] {
        &mut self.back
    }

    pub fn flush(&mut self) -> io::Result<usize> {
        if self.width == 0 || self.height == 0 {
            return Ok(0);
        }

        let mut out = io::stdout().lock();
        let mut changed = 0usize;
        let mut cursor: Option<(usize, usize)> = None;

        for y in 0..self.height {
            let mut x = 0;
            while x < self.width {
                let idx = y * self.width + x;

                if self.front[idx] == self.back[idx] {
                    x += 1;
                    continue;
                }

                if cursor != Some((x, y)) {
                    write!(out, "\x1b[{};{}H", y + 1, x + 1)?;
                }

                let run_start = x;
                while x < self.width
                    && self.front[y * self.width + x] != self.back[y * self.width + x]
                {
                    write!(out, "{}", self.back[y * self.width + x])?;
                    x += 1;
                }
                changed += x - run_start;
                cursor = Some((x, y));
            }
        }

        out.flush()?;
        std::mem::swap(&mut self.front, &mut self.back);
        Ok(changed)
    }
}

impl Default for Screen {
    fn default() -> Self { Self::new() }
}

impl Drop for Screen {
    fn drop(&mut self) {
        let _ = self.leave();
    }
}

