//! Hands an application's keyboard from one of its windows to another without
//! activating the application or reordering its windows.
//!
//! The window server tells an application which of its windows holds the
//! keyboard by posting event records to the process. Posting the same records
//! makes the application move its key window, which accessibility cannot do
//! for an application the person has used: an `AXMain` write only raises the
//! window there. The records join the queue that keys posted to the process
//! use, so a hand-over, the keys and the hand-back are handled in that order.

use std::ffi::c_void;
use std::time::Duration;

const RECORD_LEN: usize = 0xf8;
const LENGTH_OFFSET: usize = 0x04;
const KIND_OFFSET: usize = 0x08;
const POINT_OFFSET: usize = 0x20;
const POINT_LEN: usize = 0x10;
const BUTTON_OFFSET: usize = 0x3a;
const WINDOW_OFFSET: usize = 0x3c;
const FOCUS_OFFSET: usize = 0x8a;
const KIND_POINTER_DOWN: u8 = 0x01;
const KIND_POINTER_UP: u8 = 0x02;
const KIND_WINDOW_FOCUS: u8 = 0x0d;
const FOCUS_GAINED: u8 = 0x01;
const FOCUS_LOST: u8 = 0x02;
const PRIMARY_BUTTON: u8 = 0x10;
const HID_SYSTEM_STATE: i32 = 1;
const EVENT_KEY_DOWN: u32 = 10;
const EVENT_FLAGS_CHANGED: u32 = 12;
const PRIMARY_MOUSE_BUTTON: u32 = 0;
const SECONDARY_MOUSE_BUTTON: u32 = 1;
const FRONT_AS_THE_PERSON_CHOSE: u32 = 0x200;

type Record = [u8; RECORD_LEN];
type PostEventRecord = unsafe extern "C" fn(*const ProcessSerial, *const u8) -> i32;
type ProcessForPid = unsafe extern "C" fn(i32, *mut ProcessSerial) -> i32;
type FrontProcess = unsafe extern "C" fn(*mut ProcessSerial) -> i32;
type ProcessPid = unsafe extern "C" fn(*const ProcessSerial, *mut i32) -> i32;
type SetFrontProcess = unsafe extern "C" fn(*const ProcessSerial, u32, u32) -> i32;

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct ProcessSerial {
    high: u32,
    low: u32,
}

#[derive(Clone, Copy)]
struct Bridge {
    post: PostEventRecord,
    process_for_pid: ProcessForPid,
    front_process: Option<FrontProcess>,
    process_pid: Option<ProcessPid>,
    set_front_process: Option<SetFrontProcess>,
}

/// Whether this system offers the hand-over at all.
pub(crate) fn available() -> bool {
    bridge().is_some()
}

/// Moves the keyboard of process `pid` from window `from` to window `to`.
/// The application handles the loss before the gain only when `settle`
/// separates them. Returns whether every record was accepted; the caller
/// observes the result.
pub(crate) fn hand_over(pid: i32, from: Option<u32>, to: u32, settle: Duration) -> bool {
    let Some(post) = poster(pid) else {
        return false;
    };
    if let Some(from) = from {
        if !post(focus_record(from, FOCUS_LOST)) {
            return false;
        }
        std::thread::sleep(settle);
    }
    post(focus_record(to, FOCUS_GAINED))
        && post(pointer_record(to, KIND_POINTER_DOWN))
        && post(pointer_record(to, KIND_POINTER_UP))
}

/// Takes the focus from `window` and leaves it as the window the application
/// will give the keyboard to, which is how a background application rests.
pub(crate) fn release(pid: i32, window: u32) -> bool {
    poster(pid).is_some_and(|post| post(focus_record(window, FOCUS_LOST)))
}

fn poster(pid: i32) -> Option<impl Fn(Record) -> bool> {
    let bridge = bridge()?;
    let mut serial = ProcessSerial::default();
    if unsafe { (bridge.process_for_pid)(pid, &mut serial) } != 0 {
        return None;
    }
    Some(move |record: Record| unsafe { (bridge.post)(&serial, record.as_ptr()) } == 0)
}

/// Whether process `pid` is the application the window server has in front.
/// An application that was handed the keyboard answers that it is frontmost
/// itself, so its own accessibility attribute cannot tell. `None` when this
/// system does not offer the question.
pub(crate) fn is_front_process(pid: i32) -> Option<bool> {
    let bridge = bridge()?;
    let front_process = bridge.front_process?;
    let mut front = ProcessSerial::default();
    let mut asked = ProcessSerial::default();
    let answered = unsafe { front_process(&mut front) } == 0
        && unsafe { (bridge.process_for_pid)(pid, &mut asked) } == 0;
    answered.then_some(front == asked)
}

/// The process the window server has in front, or `None` when this system
/// does not offer the question.
pub(crate) fn front_pid() -> Option<i32> {
    let bridge = bridge()?;
    let (front_process, process_pid) = (bridge.front_process?, bridge.process_pid?);
    let mut front = ProcessSerial::default();
    let mut pid = 0;
    let answered =
        unsafe { front_process(&mut front) } == 0 && unsafe { process_pid(&front, &mut pid) } == 0;
    (answered && pid > 0).then_some(pid)
}

/// Puts process `pid` in front with `window` as its front window, the way the
/// window server does when the person picks that window. Returns whether the
/// request was accepted; the caller observes the result.
pub(crate) fn bring_forward(pid: i32, window: u32) -> bool {
    let Some((bridge, set_front_process)) =
        bridge().and_then(|bridge| Some((bridge, bridge.set_front_process?)))
    else {
        return false;
    };
    let mut serial = ProcessSerial::default();
    let known = unsafe { (bridge.process_for_pid)(pid, &mut serial) } == 0;
    known && unsafe { set_front_process(&serial, window, FRONT_AS_THE_PERSON_CHOSE) } == 0
}

/// How long ago the person last used the physical keyboard, or `None` while a
/// mouse button is held, which is a drag or a press in progress.
pub(crate) fn person_idle_for() -> Option<Duration> {
    let held = [PRIMARY_MOUSE_BUTTON, SECONDARY_MOUSE_BUTTON]
        .into_iter()
        .any(|button| unsafe { CGEventSourceButtonState(HID_SYSTEM_STATE, button) });
    if held {
        return None;
    }
    let seconds = [EVENT_KEY_DOWN, EVENT_FLAGS_CHANGED]
        .into_iter()
        .map(|event| unsafe { CGEventSourceSecondsSinceLastEventType(HID_SYSTEM_STATE, event) })
        .fold(f64::INFINITY, f64::min);
    Duration::try_from_secs_f64(seconds.max(0.0)).ok()
}

fn focus_record(window: u32, focus: u8) -> Record {
    let mut record = blank_record(window);
    record[KIND_OFFSET] = KIND_WINDOW_FOCUS;
    record[FOCUS_OFFSET] = focus;
    record
}

/// A pointer press that names the window and no point inside it, so the
/// application makes the window key and no control receives a click.
fn pointer_record(window: u32, kind: u8) -> Record {
    let mut record = blank_record(window);
    record[KIND_OFFSET] = kind;
    record[BUTTON_OFFSET] = PRIMARY_BUTTON;
    record[POINT_OFFSET..POINT_OFFSET + POINT_LEN].fill(0xff);
    record
}

fn blank_record(window: u32) -> Record {
    let mut record = [0_u8; RECORD_LEN];
    record[LENGTH_OFFSET] = RECORD_LEN as u8;
    record[WINDOW_OFFSET..WINDOW_OFFSET + 4].copy_from_slice(&window.to_ne_bytes());
    record
}

fn bridge() -> Option<Bridge> {
    static BRIDGE: std::sync::OnceLock<Option<Bridge>> = std::sync::OnceLock::new();
    *BRIDGE.get_or_init(|| {
        let window_server = unsafe { dlopen(WINDOW_SERVER_CLIENT.as_ptr(), RTLD_LAZY) };
        let post = symbol(window_server, c"SLPSPostEventRecordTo")?;
        let services = unsafe { dlopen(APPLICATION_SERVICES.as_ptr(), RTLD_LAZY) };
        let process_for_pid = symbol(services, c"GetProcessForPID")?;
        let front_process = symbol(window_server, c"_SLPSGetFrontProcess")
            .map(|symbol| unsafe { std::mem::transmute::<*mut c_void, FrontProcess>(symbol) });
        let process_pid = symbol(services, c"GetProcessPID")
            .map(|symbol| unsafe { std::mem::transmute::<*mut c_void, ProcessPid>(symbol) });
        let set_front_process = symbol(window_server, c"_SLPSSetFrontProcessWithOptions")
            .map(|symbol| unsafe { std::mem::transmute::<*mut c_void, SetFrontProcess>(symbol) });
        Some(Bridge {
            post: unsafe { std::mem::transmute::<*mut c_void, PostEventRecord>(post) },
            process_for_pid: unsafe {
                std::mem::transmute::<*mut c_void, ProcessForPid>(process_for_pid)
            },
            front_process,
            process_pid,
            set_front_process,
        })
    })
}

fn symbol(handle: *mut c_void, name: &std::ffi::CStr) -> Option<*mut c_void> {
    if handle.is_null() {
        return None;
    }
    let symbol = unsafe { dlsym(handle, name.as_ptr()) };
    (!symbol.is_null()).then_some(symbol)
}

const WINDOW_SERVER_CLIENT: &std::ffi::CStr =
    c"/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight";
const APPLICATION_SERVICES: &std::ffi::CStr =
    c"/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices";
const RTLD_LAZY: i32 = 1;

unsafe extern "C" {
    fn dlopen(path: *const std::ffi::c_char, mode: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const std::ffi::c_char) -> *mut c_void;
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    fn CGEventSourceButtonState(state: i32, button: u32) -> bool;
}

#[cfg(test)]
#[path = "window_key_handoff_tests.rs"]
mod tests;
