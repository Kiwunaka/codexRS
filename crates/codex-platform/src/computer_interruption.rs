use std::io;

#[cfg(windows)]
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
#[cfg(windows)]
use std::thread::{self, JoinHandle};
#[cfg(windows)]
use std::time::Duration;

#[cfg(windows)]
use crossbeam_channel::{Receiver, bounded};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputerUseTurnKey {
    pub thread_id: String,
    pub turn_id: String,
    pub window_id: Option<String>,
}

pub struct ComputerUseInterruptionMonitor {
    #[cfg(windows)]
    interrupted: Receiver<ComputerUseTurnKey>,
    #[cfg(windows)]
    user_input: Receiver<ComputerUseTurnKey>,
    #[cfg(windows)]
    shared: Arc<Mutex<MonitorState>>,
    #[cfg(windows)]
    shutdown: Arc<AtomicBool>,
    #[cfg(windows)]
    thread: Option<JoinHandle<()>>,
}

#[cfg(windows)]
#[derive(Default)]
struct MonitorState {
    generation: u64,
    turn: Option<ComputerUseTurnKey>,
}

impl ComputerUseInterruptionMonitor {
    pub fn new() -> io::Result<Self> {
        #[cfg(windows)]
        {
            let (interrupted_tx, interrupted) = bounded(1);
            let (user_input_tx, user_input) = bounded(1);
            let shared = Arc::new(Mutex::new(MonitorState::default()));
            let shutdown = Arc::new(AtomicBool::new(false));
            let thread_shared = Arc::clone(&shared);
            let thread_shutdown = Arc::clone(&shutdown);
            let thread = thread::Builder::new()
                .name("codexrs-computer-use-interruption".to_owned())
                .spawn(move || {
                    let mut observed_generation = u64::MAX;
                    let mut escape = EscapeEdge::default();
                    let mut physical_input = PhysicalInputEdge::default();
                    let mut user_input_reported = false;
                    while !thread_shutdown.load(Ordering::Acquire) {
                        let Some((generation, turn)) = thread_shared
                            .lock()
                            .ok()
                            .map(|state| (state.generation, state.turn.clone()))
                        else {
                            break;
                        };
                        let escape_is_down = winsafe::GetAsyncKeyState(winsafe::co::VK::ESCAPE);
                        let input_sample = PhysicalInputSample::read();
                        if observed_generation != generation {
                            observed_generation = generation;
                            escape.reset(escape_is_down);
                            physical_input.reset(input_sample);
                            user_input_reported = false;
                        } else if turn.is_some() && escape.observe(escape_is_down) {
                            let turn = thread_shared.lock().ok().and_then(|mut state| {
                                if state.generation != observed_generation {
                                    return None;
                                }
                                state.generation = state.generation.wrapping_add(1);
                                state.turn.take()
                            });
                            if let Some(turn) = turn {
                                let _ = interrupted_tx.try_send(turn);
                            }
                        } else {
                            let user_changed_input = physical_input.observe(input_sample);
                            if !user_input_reported
                                && user_changed_input
                                && turn.as_ref().is_some_and(target_window_is_foreground)
                                && let Some(turn) = turn
                            {
                                let _ = user_input_tx.try_send(turn);
                                user_input_reported = true;
                            }
                        }
                        thread::sleep(Duration::from_millis(8));
                    }
                })?;
            Ok(Self {
                interrupted,
                user_input,
                shared,
                shutdown,
                thread: Some(thread),
            })
        }

        #[cfg(not(windows))]
        Ok(Self {})
    }

    pub fn arm(
        &self,
        thread_id: impl Into<String>,
        turn_id: impl Into<String>,
        window_id: Option<String>,
    ) {
        #[cfg(windows)]
        if let Ok(mut state) = self.shared.lock() {
            state.generation = state.generation.wrapping_add(1);
            state.turn = Some(ComputerUseTurnKey {
                thread_id: thread_id.into(),
                turn_id: turn_id.into(),
                window_id,
            });
        }

        #[cfg(not(windows))]
        {
            let _ = (thread_id.into(), turn_id.into(), window_id);
        }
    }

    pub fn disarm(&self) {
        #[cfg(windows)]
        if let Ok(mut state) = self.shared.lock() {
            state.generation = state.generation.wrapping_add(1);
            state.turn = None;
        }
    }

    pub fn disarm_turn(&self, thread_id: &str, turn_id: &str) {
        #[cfg(windows)]
        if let Ok(mut state) = self.shared.lock()
            && state
                .turn
                .as_ref()
                .is_some_and(|turn| turn.thread_id == thread_id && turn.turn_id == turn_id)
        {
            state.generation = state.generation.wrapping_add(1);
            state.turn = None;
        }

        #[cfg(not(windows))]
        let _ = (thread_id, turn_id);
    }

    #[must_use]
    pub fn active_turn(&self) -> Option<ComputerUseTurnKey> {
        #[cfg(windows)]
        {
            self.shared.lock().ok().and_then(|state| state.turn.clone())
        }

        #[cfg(not(windows))]
        None
    }

    #[must_use]
    pub fn try_recv(&self) -> Option<ComputerUseTurnKey> {
        #[cfg(windows)]
        {
            self.interrupted.try_recv().ok()
        }

        #[cfg(not(windows))]
        None
    }

    #[must_use]
    pub fn try_recv_user_input(&self) -> Option<ComputerUseTurnKey> {
        #[cfg(windows)]
        {
            self.user_input.try_recv().ok()
        }

        #[cfg(not(windows))]
        None
    }
}

#[cfg(windows)]
impl Drop for ComputerUseInterruptionMonitor {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(windows)]
const MONITORED_INPUT_KEYS: &[winsafe::co::VK] = &[
    winsafe::co::VK::LBUTTON,
    winsafe::co::VK::RBUTTON,
    winsafe::co::VK::MBUTTON,
    winsafe::co::VK::XBUTTON1,
    winsafe::co::VK::XBUTTON2,
    winsafe::co::VK::BACK,
    winsafe::co::VK::TAB,
    winsafe::co::VK::CLEAR,
    winsafe::co::VK::RETURN,
    winsafe::co::VK::PAUSE,
    winsafe::co::VK::CAPITAL,
    winsafe::co::VK::SPACE,
    winsafe::co::VK::PRIOR,
    winsafe::co::VK::NEXT,
    winsafe::co::VK::END,
    winsafe::co::VK::HOME,
    winsafe::co::VK::LEFT,
    winsafe::co::VK::UP,
    winsafe::co::VK::RIGHT,
    winsafe::co::VK::DOWN,
    winsafe::co::VK::SELECT,
    winsafe::co::VK::PRINT,
    winsafe::co::VK::EXECUTE,
    winsafe::co::VK::SNAPSHOT,
    winsafe::co::VK::INSERT,
    winsafe::co::VK::DELETE,
    winsafe::co::VK::HELP,
    winsafe::co::VK::CHAR_0,
    winsafe::co::VK::CHAR_1,
    winsafe::co::VK::CHAR_2,
    winsafe::co::VK::CHAR_3,
    winsafe::co::VK::CHAR_4,
    winsafe::co::VK::CHAR_5,
    winsafe::co::VK::CHAR_6,
    winsafe::co::VK::CHAR_7,
    winsafe::co::VK::CHAR_8,
    winsafe::co::VK::CHAR_9,
    winsafe::co::VK::CHAR_A,
    winsafe::co::VK::CHAR_B,
    winsafe::co::VK::CHAR_C,
    winsafe::co::VK::CHAR_D,
    winsafe::co::VK::CHAR_E,
    winsafe::co::VK::CHAR_F,
    winsafe::co::VK::CHAR_G,
    winsafe::co::VK::CHAR_H,
    winsafe::co::VK::CHAR_I,
    winsafe::co::VK::CHAR_J,
    winsafe::co::VK::CHAR_K,
    winsafe::co::VK::CHAR_L,
    winsafe::co::VK::CHAR_M,
    winsafe::co::VK::CHAR_N,
    winsafe::co::VK::CHAR_O,
    winsafe::co::VK::CHAR_P,
    winsafe::co::VK::CHAR_Q,
    winsafe::co::VK::CHAR_R,
    winsafe::co::VK::CHAR_S,
    winsafe::co::VK::CHAR_T,
    winsafe::co::VK::CHAR_U,
    winsafe::co::VK::CHAR_V,
    winsafe::co::VK::CHAR_W,
    winsafe::co::VK::CHAR_X,
    winsafe::co::VK::CHAR_Y,
    winsafe::co::VK::CHAR_Z,
    winsafe::co::VK::LWIN,
    winsafe::co::VK::RWIN,
    winsafe::co::VK::APPS,
    winsafe::co::VK::NUMPAD0,
    winsafe::co::VK::NUMPAD1,
    winsafe::co::VK::NUMPAD2,
    winsafe::co::VK::NUMPAD3,
    winsafe::co::VK::NUMPAD4,
    winsafe::co::VK::NUMPAD5,
    winsafe::co::VK::NUMPAD6,
    winsafe::co::VK::NUMPAD7,
    winsafe::co::VK::NUMPAD8,
    winsafe::co::VK::NUMPAD9,
    winsafe::co::VK::MULTIPLY,
    winsafe::co::VK::ADD,
    winsafe::co::VK::SEPARATOR,
    winsafe::co::VK::SUBTRACT,
    winsafe::co::VK::DECIMAL,
    winsafe::co::VK::DIVIDE,
    winsafe::co::VK::F1,
    winsafe::co::VK::F2,
    winsafe::co::VK::F3,
    winsafe::co::VK::F4,
    winsafe::co::VK::F5,
    winsafe::co::VK::F6,
    winsafe::co::VK::F7,
    winsafe::co::VK::F8,
    winsafe::co::VK::F9,
    winsafe::co::VK::F10,
    winsafe::co::VK::F11,
    winsafe::co::VK::F12,
    winsafe::co::VK::F13,
    winsafe::co::VK::F14,
    winsafe::co::VK::F15,
    winsafe::co::VK::F16,
    winsafe::co::VK::F17,
    winsafe::co::VK::F18,
    winsafe::co::VK::F19,
    winsafe::co::VK::F20,
    winsafe::co::VK::F21,
    winsafe::co::VK::F22,
    winsafe::co::VK::F23,
    winsafe::co::VK::F24,
    winsafe::co::VK::NUMLOCK,
    winsafe::co::VK::SCROLL,
    winsafe::co::VK::LSHIFT,
    winsafe::co::VK::RSHIFT,
    winsafe::co::VK::LCONTROL,
    winsafe::co::VK::RCONTROL,
    winsafe::co::VK::LMENU,
    winsafe::co::VK::RMENU,
    winsafe::co::VK::OEM_1,
    winsafe::co::VK::OEM_PLUS,
    winsafe::co::VK::OEM_COMMA,
    winsafe::co::VK::OEM_MINUS,
    winsafe::co::VK::OEM_PERIOD,
    winsafe::co::VK::OEM_2,
    winsafe::co::VK::OEM_3,
    winsafe::co::VK::OEM_4,
    winsafe::co::VK::OEM_5,
    winsafe::co::VK::OEM_6,
    winsafe::co::VK::OEM_7,
    winsafe::co::VK::OEM_8,
    winsafe::co::VK::OEM_102,
];

#[cfg(windows)]
#[derive(Clone, Copy, Default)]
struct PhysicalInputSample {
    cursor: Option<(i32, i32)>,
    pressed: u128,
}

#[cfg(windows)]
impl PhysicalInputSample {
    fn read() -> Self {
        let cursor = winsafe::GetCursorPos().ok().map(|point| (point.x, point.y));
        let mut pressed = 0_u128;
        for (index, key) in MONITORED_INPUT_KEYS.iter().copied().enumerate().take(128) {
            if winsafe::GetAsyncKeyState(key) {
                pressed |= 1_u128 << index;
            }
        }
        Self { cursor, pressed }
    }
}

#[cfg(windows)]
#[derive(Default)]
struct PhysicalInputEdge {
    cursor: Option<(i32, i32)>,
    pressed: u128,
}

#[cfg(windows)]
impl PhysicalInputEdge {
    fn reset(&mut self, sample: PhysicalInputSample) {
        self.cursor = sample.cursor;
        self.pressed = sample.pressed;
    }

    fn observe(&mut self, sample: PhysicalInputSample) -> bool {
        let cursor_moved = self
            .cursor
            .zip(sample.cursor)
            .is_some_and(|(before, after)| before != after);
        let key_pressed = sample.pressed & !self.pressed != 0;
        self.reset(sample);
        cursor_moved || key_pressed
    }
}

#[cfg(windows)]
fn target_window_is_foreground(turn: &ComputerUseTurnKey) -> bool {
    use winsafe::prelude::{Handle, user_Hwnd};

    let Some(window_id) = turn
        .window_id
        .as_deref()
        .and_then(|window_id| window_id.parse::<usize>().ok())
    else {
        return false;
    };
    winsafe::HWND::GetForegroundWindow()
        .as_ref()
        .is_some_and(|window| window.ptr() as usize == window_id)
}

#[derive(Default)]
struct EscapeEdge {
    was_down: bool,
}

impl EscapeEdge {
    fn reset(&mut self, is_down: bool) {
        self.was_down = is_down;
    }

    fn observe(&mut self, is_down: bool) -> bool {
        let pressed = is_down && !self.was_down;
        self.was_down = is_down;
        pressed
    }
}

#[cfg(test)]
mod tests {
    use super::EscapeEdge;
    #[cfg(windows)]
    use super::{MONITORED_INPUT_KEYS, PhysicalInputEdge, PhysicalInputSample};

    #[test]
    fn escape_interrupts_only_on_a_fresh_physical_press() {
        let mut edge = EscapeEdge::default();
        edge.reset(true);
        assert!(!edge.observe(true));
        assert!(!edge.observe(false));
        assert!(edge.observe(true));
        assert!(!edge.observe(true));
    }

    #[cfg(windows)]
    #[test]
    fn physical_input_edges_ignore_the_arm_baseline_and_held_keys() {
        assert!(MONITORED_INPUT_KEYS.len() <= 128);
        let mut edge = PhysicalInputEdge::default();
        edge.reset(PhysicalInputSample {
            cursor: Some((10, 20)),
            pressed: 1,
        });
        assert!(!edge.observe(PhysicalInputSample {
            cursor: Some((10, 20)),
            pressed: 1,
        }));
        assert!(edge.observe(PhysicalInputSample {
            cursor: Some((11, 20)),
            pressed: 1,
        }));
        assert!(edge.observe(PhysicalInputSample {
            cursor: Some((11, 20)),
            pressed: 3,
        }));
        assert!(!edge.observe(PhysicalInputSample {
            cursor: Some((11, 20)),
            pressed: 3,
        }));
    }
}
