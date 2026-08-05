//! A small safe wrapper over the Win32 task dialog.
//!
//! Author: PratikP1
//!
//! `TaskDialogIndirect` is the modern Windows dialog: a large main
//! instruction, body text, command-link buttons, an expandable details pane, a
//! footer, and an optional progress bar.  Everything it draws is a real system
//! control, so screen readers and keyboard navigation work without any bespoke
//! accessibility code, and it scales correctly on high-DPI displays.
//!
//! Only the pieces Wixen actually uses are wrapped.

use std::{
    ffi::c_void,
    io, iter,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

// ─── Win32 types ─────────────────────────────────────────────────────────────

type Hwnd = *mut c_void;
type Hinstance = *mut c_void;
type Hresult = i32;

/// `TASKDIALOGCONFIG` and `TASKDIALOG_BUTTON` are declared inside
/// `pshpack1.h` / `poppack.h`, so they are packed to a single byte with no
/// padding.  Getting this wrong shifts every field and corrupts the call, so
/// the layout is asserted against the documented size in the tests below.
#[repr(C, packed(1))]
struct TaskDialogConfig {
    cb_size: u32,
    hwnd_parent: Hwnd,
    h_instance: Hinstance,
    dw_flags: u32,
    dw_common_buttons: u32,
    psz_window_title: *const u16,
    /// Union with `hMainIcon`; we always pass a `MAKEINTRESOURCE` value.
    psz_main_icon: *const u16,
    psz_main_instruction: *const u16,
    psz_content: *const u16,
    c_buttons: u32,
    p_buttons: *const TaskDialogButton,
    n_default_button: i32,
    c_radio_buttons: u32,
    p_radio_buttons: *const TaskDialogButton,
    n_default_radio_button: i32,
    psz_verification_text: *const u16,
    psz_expanded_information: *const u16,
    psz_expanded_control_text: *const u16,
    psz_collapsed_control_text: *const u16,
    /// Union with `hFooterIcon`.
    psz_footer_icon: *const u16,
    psz_footer: *const u16,
    pfn_callback: TaskDialogCallback,
    lp_callback_data: isize,
    cx_width: u32,
}

#[repr(C, packed(1))]
struct TaskDialogButton {
    n_button_id: i32,
    psz_button_text: *const u16,
}

type TaskDialogCallback =
    Option<unsafe extern "system" fn(Hwnd, u32, usize, isize, isize) -> Hresult>;

#[link(name = "comctl32")]
unsafe extern "system" {
    fn TaskDialogIndirect(
        config: *const TaskDialogConfig,
        pressed_button: *mut i32,
        pressed_radio_button: *mut i32,
        verification_checked: *mut i32,
    ) -> Hresult;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn SendMessageW(hwnd: Hwnd, message: u32, wparam: usize, lparam: isize) -> isize;
}

// ─── Constants ───────────────────────────────────────────────────────────────

const TDF_ALLOW_DIALOG_CANCELLATION: u32 = 0x0008;
const TDF_USE_COMMAND_LINKS: u32 = 0x0010;
const TDF_EXPAND_FOOTER_AREA: u32 = 0x0040;
const TDF_SHOW_PROGRESS_BAR: u32 = 0x0200;
const TDF_CALLBACK_TIMER: u32 = 0x0800;
const TDF_POSITION_RELATIVE_TO_WINDOW: u32 = 0x1000;
const TDF_SIZE_TO_CONTENT: u32 = 0x0100_0000;

pub const TDCBF_CANCEL: u32 = 0x0008;
pub const TDCBF_CLOSE: u32 = 0x0020;

pub const IDOK: i32 = 1;
pub const IDCANCEL: i32 = 2;

/// `MAKEINTRESOURCEW(-1)`; the negative icon ids are passed as pointer values.
const TD_WARNING_ICON: *const u16 = 0xFFFF as *const u16;
const TD_INFORMATION_ICON: *const u16 = 0xFFFD as *const u16;
const TD_SHIELD_ICON: *const u16 = 0xFFFC as *const u16;

const TDN_DIALOG_CONSTRUCTED: u32 = 7;
const TDN_TIMER: u32 = 4;
const TDN_HELP: u32 = 9;
const TDN_BUTTON_CLICKED: u32 = 2;

const WM_USER: u32 = 0x0400;
const TDM_CLICK_BUTTON: u32 = WM_USER + 102;
const TDM_SET_PROGRESS_BAR_RANGE: u32 = WM_USER + 105;
const TDM_SET_PROGRESS_BAR_POS: u32 = WM_USER + 106;
const TDM_UPDATE_ELEMENT_TEXT: u32 = WM_USER + 114;
const TDM_ENABLE_BUTTON: u32 = WM_USER + 111;

const TDE_CONTENT: usize = 0;

const S_OK: Hresult = 0;
const S_FALSE: Hresult = 1;

// ─── Public surface ──────────────────────────────────────────────────────────

/// Which system icon the dialog shows beside its main instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    None,
    Information,
    Warning,
    Shield,
}

impl Icon {
    fn as_resource(self) -> *const u16 {
        match self {
            Icon::None => std::ptr::null(),
            Icon::Information => TD_INFORMATION_ICON,
            Icon::Warning => TD_WARNING_ICON,
            Icon::Shield => TD_SHIELD_ICON,
        }
    }
}

/// A task dialog, described in terms of what the user sees.
pub struct Dialog {
    title: Vec<u16>,
    main_instruction: Vec<u16>,
    content: Vec<u16>,
    icon: Icon,
    buttons: Vec<(i32, Vec<u16>)>,
    use_command_links: bool,
    common_buttons: u32,
    default_button: i32,
    expanded_information: Option<Vec<u16>>,
    /// Label while the pane is collapsed, i.e. the invitation to expand.
    collapsed_control_text: Option<Vec<u16>>,
    /// Label while the pane is expanded, i.e. the invitation to collapse.
    expanded_control_text: Option<Vec<u16>>,
    footer: Option<Vec<u16>>,
    allow_cancel: bool,
}

impl Dialog {
    pub fn new(title: &str, main_instruction: &str, content: &str) -> Self {
        Self {
            title: to_wide(title),
            main_instruction: to_wide(main_instruction),
            content: to_wide(content),
            icon: Icon::None,
            buttons: Vec::new(),
            use_command_links: false,
            common_buttons: 0,
            default_button: 0,
            expanded_information: None,
            collapsed_control_text: None,
            expanded_control_text: None,
            footer: None,
            allow_cancel: true,
        }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = icon;
        self
    }

    /// Add a push button.  An `&` marks the access key.
    pub fn button(mut self, id: i32, text: &str) -> Self {
        self.buttons.push((id, to_wide(text)));
        self
    }

    /// Add a large command-link button: `title` on the first line, `note` in
    /// smaller text beneath it.  The screen reader announces both.
    pub fn command_link(mut self, id: i32, title: &str, note: &str) -> Self {
        self.use_command_links = true;
        self.buttons
            .push((id, to_wide(&format!("{title}\n{note}"))));
        self
    }

    pub fn common_buttons(mut self, buttons: u32) -> Self {
        self.common_buttons = buttons;
        self
    }

    /// The button that has focus when the dialog opens.
    pub fn default_button(mut self, id: i32) -> Self {
        self.default_button = id;
        self
    }

    /// Content for the collapsible details pane.
    ///
    /// Windows shows `expand_label` while the pane is closed and
    /// `collapse_label` while it is open, so they must differ: a button that
    /// still reads "Show details" once the details are showing tells a screen
    /// reader user the opposite of the truth.  Leaving the expanded label unset
    /// does not help — Windows then copies the collapsed one.
    pub fn details(mut self, expand_label: &str, collapse_label: &str, information: &str) -> Self {
        self.collapsed_control_text = Some(to_wide(expand_label));
        self.expanded_control_text = Some(to_wide(collapse_label));
        self.expanded_information = Some(to_wide(information));
        self
    }

    pub fn footer(mut self, footer: &str) -> Self {
        self.footer = Some(to_wide(footer));
        self
    }

    /// Whether Esc and the title-bar close button dismiss the dialog.
    pub fn allow_cancel(mut self, allow: bool) -> Self {
        self.allow_cancel = allow;
        self
    }

    fn flags(&self) -> u32 {
        let mut flags = TDF_SIZE_TO_CONTENT | TDF_POSITION_RELATIVE_TO_WINDOW;
        if self.allow_cancel {
            flags |= TDF_ALLOW_DIALOG_CANCELLATION;
        }
        if self.use_command_links {
            flags |= TDF_USE_COMMAND_LINKS;
        }
        if self.expanded_information.is_some() {
            // Expanding grows the footer area rather than the body, which keeps
            // the buttons where the user left them.
            flags |= TDF_EXPAND_FOOTER_AREA;
        }
        flags
    }

    fn config(&self, extra_flags: u32, callback: TaskDialogCallback, data: isize) -> Config<'_> {
        let buttons: Vec<TaskDialogButton> = self
            .buttons
            .iter()
            .map(|(id, text)| TaskDialogButton {
                n_button_id: *id,
                psz_button_text: text.as_ptr(),
            })
            .collect();

        let config = TaskDialogConfig {
            cb_size: size_of::<TaskDialogConfig>() as u32,
            hwnd_parent: std::ptr::null_mut(),
            h_instance: std::ptr::null_mut(),
            dw_flags: self.flags() | extra_flags,
            dw_common_buttons: self.common_buttons,
            psz_window_title: self.title.as_ptr(),
            psz_main_icon: self.icon.as_resource(),
            psz_main_instruction: self.main_instruction.as_ptr(),
            psz_content: self.content.as_ptr(),
            c_buttons: buttons.len() as u32,
            p_buttons: buttons.as_ptr(),
            n_default_button: self.default_button,
            c_radio_buttons: 0,
            p_radio_buttons: std::ptr::null(),
            n_default_radio_button: 0,
            psz_verification_text: std::ptr::null(),
            psz_expanded_information: optional_ptr(self.expanded_information.as_ref()),
            psz_expanded_control_text: optional_ptr(self.expanded_control_text.as_ref()),
            psz_collapsed_control_text: optional_ptr(self.collapsed_control_text.as_ref()),
            psz_footer_icon: std::ptr::null(),
            psz_footer: optional_ptr(self.footer.as_ref()),
            pfn_callback: callback,
            lp_callback_data: data,
            cx_width: 0,
        };

        Config {
            config,
            _buttons: buttons,
            _dialog: self,
        }
    }

    /// Show the dialog and return the id of the button the user chose.
    pub fn show(&self) -> io::Result<i32> {
        let config = self.config(0, Some(help_callback), 0);
        invoke(&config.config)
    }
}

/// Keeps the button array alive for exactly as long as the config that points
/// into it.
struct Config<'a> {
    config: TaskDialogConfig,
    _buttons: Vec<TaskDialogButton>,
    _dialog: &'a Dialog,
}

fn optional_ptr(value: Option<&Vec<u16>>) -> *const u16 {
    value.map_or(std::ptr::null(), |text| text.as_ptr())
}

fn invoke(config: &TaskDialogConfig) -> io::Result<i32> {
    let mut pressed = 0i32;

    // SAFETY: `config` is fully initialised, `cb_size` matches its layout, and
    // every string pointer it holds is owned by the `Dialog` that outlives this
    // call.
    let result = unsafe {
        TaskDialogIndirect(
            config,
            &mut pressed,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };

    if result == S_OK {
        Ok(pressed)
    } else {
        Err(io::Error::other(format!(
            "TaskDialogIndirect failed (HRESULT {result:#010x})"
        )))
    }
}

// ─── F1 help ─────────────────────────────────────────────────────────────────

/// Set by the owning module so the wrapper stays free of Wixen specifics.
type HelpHandler = fn();
static HELP_HANDLER: std::sync::OnceLock<HelpHandler> = std::sync::OnceLock::new();

/// Register what F1 does.  The first registration wins.
pub fn set_help_handler(handler: HelpHandler) {
    let _ = HELP_HANDLER.set(handler);
}

unsafe extern "system" fn help_callback(
    _hwnd: Hwnd,
    notification: u32,
    _wparam: usize,
    _lparam: isize,
    _data: isize,
) -> Hresult {
    if notification == TDN_HELP
        && let Some(handler) = HELP_HANDLER.get()
    {
        handler();
    }
    S_OK
}

// ─── Progress dialog ─────────────────────────────────────────────────────────

/// Shared between the worker thread doing the removal and the dialog showing
/// its progress.
pub struct ProgressState {
    completed: AtomicUsize,
    /// Index into the phase descriptions supplied to [`show_progress`].
    phase: AtomicUsize,
    finished: AtomicBool,
}

impl ProgressState {
    pub fn new() -> Self {
        Self {
            completed: AtomicUsize::new(0),
            phase: AtomicUsize::new(0),
            finished: AtomicBool::new(false),
        }
    }

    /// Called from the worker thread after each action.
    pub fn advance(&self, phase_index: usize, completed: usize) {
        self.phase.store(phase_index, Ordering::Relaxed);
        self.completed.store(completed, Ordering::Relaxed);
    }

    /// Called from the worker thread when the removal is over.
    pub fn finish(&self) {
        self.finished.store(true, Ordering::Release);
    }
}

impl Default for ProgressState {
    fn default() -> Self {
        Self::new()
    }
}

/// State the timer callback needs, reachable through `lp_callback_data`.
struct ProgressContext<'a> {
    state: &'a ProgressState,
    total: usize,
    /// One line of body text per phase, indexed by the worker's phase index.
    phase_text: Vec<Vec<u16>>,
    last_shown_phase: std::cell::Cell<Option<usize>>,
}

/// Show a modal progress dialog that closes itself once `state` is finished.
///
/// The caller runs the actual work on another thread; this call blocks until
/// that work reports completion.
pub fn show_progress(
    dialog: &Dialog,
    state: &ProgressState,
    total: usize,
    phase_text: &[String],
) -> io::Result<()> {
    let context = ProgressContext {
        state,
        total,
        phase_text: phase_text.iter().map(|text| to_wide(text)).collect(),
        last_shown_phase: std::cell::Cell::new(None),
    };

    let config = dialog.config(
        TDF_SHOW_PROGRESS_BAR | TDF_CALLBACK_TIMER,
        Some(progress_callback),
        std::ptr::from_ref(&context) as isize,
    );

    invoke(&config.config).map(|_| ())
}

unsafe extern "system" fn progress_callback(
    hwnd: Hwnd,
    notification: u32,
    wparam: usize,
    _lparam: isize,
    data: isize,
) -> Hresult {
    // SAFETY: `data` is the pointer `show_progress` passed, and that borrow
    // outlives the blocking `TaskDialogIndirect` call this callback runs under.
    let context = unsafe { &*(data as *const ProgressContext) };

    match notification {
        TDN_DIALOG_CONSTRUCTED => {
            send(
                hwnd,
                TDM_SET_PROGRESS_BAR_RANGE,
                0,
                make_range(0, context.total),
            );
            // Interrupting a half-finished removal is more dangerous than
            // waiting for it, so Cancel is present but disabled: a screen
            // reader announces it as unavailable rather than it silently
            // doing nothing.
            send(hwnd, TDM_ENABLE_BUTTON, IDCANCEL as usize, 0);
            S_OK
        }
        TDN_TIMER => {
            update_progress(hwnd, context);
            if context.state.finished.load(Ordering::Acquire) {
                // Re-enable before clicking: a disabled button may ignore
                // TDM_CLICK_BUTTON, which would leave the dialog up forever
                // with no way to dismiss it.
                send(hwnd, TDM_ENABLE_BUTTON, IDCANCEL as usize, 1);
                send(hwnd, TDM_CLICK_BUTTON, IDCANCEL as usize, 0);
            }
            S_OK
        }
        // Refuse every close until the worker says it is done.
        TDN_BUTTON_CLICKED if !context.state.finished.load(Ordering::Acquire) => {
            let _ = wparam;
            S_FALSE
        }
        TDN_HELP => {
            if let Some(handler) = HELP_HANDLER.get() {
                handler();
            }
            S_OK
        }
        _ => S_OK,
    }
}

fn update_progress(hwnd: Hwnd, context: &ProgressContext) {
    let completed = context.state.completed.load(Ordering::Relaxed);
    send(hwnd, TDM_SET_PROGRESS_BAR_POS, completed, 0);

    // Only speak when the phase changes.  Rewriting the text on every tick
    // would make a screen reader talk over itself for the whole removal.
    let phase = context.state.phase.load(Ordering::Relaxed);
    if context.last_shown_phase.get() != Some(phase)
        && let Some(text) = context.phase_text.get(phase)
    {
        context.last_shown_phase.set(Some(phase));
        send(
            hwnd,
            TDM_UPDATE_ELEMENT_TEXT,
            TDE_CONTENT,
            text.as_ptr() as isize,
        );
    }
}

fn send(hwnd: Hwnd, message: u32, wparam: usize, lparam: isize) {
    // SAFETY: `hwnd` is the live dialog window the callback was invoked for.
    unsafe { SendMessageW(hwnd, message, wparam, lparam) };
}

/// `MAKELPARAM(low, high)` for the progress bar range.
fn make_range(low: usize, high: usize) -> isize {
    let low = (low & 0xFFFF) as isize;
    let high = (high & 0xFFFF) as isize;
    low | (high << 16)
}

fn to_wide(value: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    // Task dialogs render plain LF correctly, but normalising keeps text that
    // arrives with CRLF — such as an error string from Windows — from picking
    // up a stray carriage return.
    let normalised = value.replace("\r\n", "\n");
    std::ffi::OsStr::new(&normalised)
        .encode_wide()
        .chain(iter::once(0))
        .collect()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// The documented 64-bit size of `TASKDIALOGCONFIG` under `pack(1)`.  If
    /// this drifts, every field after the first mismatch is read from the
    /// wrong offset and the dialog call is corrupt.
    #[cfg(target_pointer_width = "64")]
    const TASKDIALOGCONFIG_SIZE_X64: usize = 160;

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn task_dialog_config_matches_the_win32_layout() {
        assert_eq!(size_of::<TaskDialogConfig>(), TASKDIALOGCONFIG_SIZE_X64);
        assert_eq!(align_of::<TaskDialogConfig>(), 1, "must stay pack(1)");
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn task_dialog_button_matches_the_win32_layout() {
        assert_eq!(size_of::<TaskDialogButton>(), 12);
        assert_eq!(align_of::<TaskDialogButton>(), 1, "must stay pack(1)");
    }

    #[test]
    fn icons_use_the_documented_makeintresource_values() {
        assert_eq!(Icon::Warning.as_resource() as usize, 0xFFFF);
        assert_eq!(Icon::Information.as_resource() as usize, 0xFFFD);
        assert_eq!(Icon::Shield.as_resource() as usize, 0xFFFC);
        assert!(Icon::None.as_resource().is_null());
    }

    #[test]
    fn make_range_packs_low_and_high_words() {
        assert_eq!(make_range(0, 1), 0x0001_0000);
        assert_eq!(make_range(0, 40), 40 << 16);
        assert_eq!(make_range(3, 7), 3 | (7 << 16));
    }

    #[test]
    fn command_links_set_their_flag_and_carry_the_note() {
        let dialog = Dialog::new("t", "m", "c").command_link(100, "McAfee", "Also removes X.");
        assert!(dialog.flags() & TDF_USE_COMMAND_LINKS != 0);
        assert_eq!(dialog.buttons.len(), 1);
        assert_eq!(dialog.buttons[0].0, 100);
        assert_eq!(
            String::from_utf16_lossy(&dialog.buttons[0].1),
            "McAfee\nAlso removes X.\0"
        );
    }

    #[test]
    fn plain_buttons_do_not_turn_the_dialog_into_command_links() {
        let dialog = Dialog::new("t", "m", "c").button(IDOK, "&Remove");
        assert_eq!(dialog.flags() & TDF_USE_COMMAND_LINKS, 0);
    }

    #[test]
    fn cancellation_can_be_switched_off_for_the_progress_screen() {
        let cancellable = Dialog::new("t", "m", "c");
        assert!(cancellable.flags() & TDF_ALLOW_DIALOG_CANCELLATION != 0);

        let fixed = Dialog::new("t", "m", "c").allow_cancel(false);
        assert_eq!(fixed.flags() & TDF_ALLOW_DIALOG_CANCELLATION, 0);
    }

    #[test]
    fn details_expand_into_the_footer_area() {
        let plain = Dialog::new("t", "m", "c");
        assert_eq!(plain.flags() & TDF_EXPAND_FOOTER_AREA, 0);

        let detailed =
            Dialog::new("t", "m", "c").details("Show details", "Hide details", "everything");
        assert!(detailed.flags() & TDF_EXPAND_FOOTER_AREA != 0);
        assert!(detailed.expanded_information.is_some());
    }

    #[test]
    fn the_expando_label_changes_with_the_pane_state() {
        let dialog = Dialog::new("t", "m", "c").details("Show details", "Hide details", "body");

        assert_eq!(
            String::from_utf16_lossy(dialog.collapsed_control_text.as_ref().unwrap()),
            "Show details\0"
        );
        assert_eq!(
            String::from_utf16_lossy(dialog.expanded_control_text.as_ref().unwrap()),
            "Hide details\0"
        );
        assert_ne!(
            dialog.collapsed_control_text, dialog.expanded_control_text,
            "a button that still says \"Show\" while showing is worse than no label"
        );
    }

    #[test]
    fn strings_are_nul_terminated_utf16() {
        let wide = to_wide("Wixen");
        assert_eq!(wide.last(), Some(&0));
        assert_eq!(String::from_utf16_lossy(&wide[..wide.len() - 1]), "Wixen");
    }

    #[test]
    fn crlf_is_normalised_so_it_cannot_double_up() {
        assert_eq!(to_wide("a\r\nb"), to_wide("a\nb"));
    }

    #[test]
    fn progress_state_tracks_position_and_completion() {
        let state = ProgressState::new();
        assert!(!state.finished.load(Ordering::Acquire));

        state.advance(2, 17);
        assert_eq!(state.completed.load(Ordering::Relaxed), 17);
        assert_eq!(state.phase.load(Ordering::Relaxed), 2);

        state.finish();
        assert!(state.finished.load(Ordering::Acquire));
    }
}
