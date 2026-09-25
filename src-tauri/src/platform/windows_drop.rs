//! The Windows folder handoff. Dragging a pet starts an OLE drag offering one
//! virtual file, a hidden `.perch-handoff-<guid>`. Explorer writes it wherever
//! the pet is dropped: an open folder, a folder icon or the desktop. Explorer
//! never tells the drag source where that was, so for the length of the
//! handoff Perch listens to the shell's own change notifications for a file
//! with that unique name, and takes its parent directory as the folder. No
//! window is inspected, nothing is captured and no input is hooked.
//!
//! Not yet run on Windows hardware; see docs/verification.md.
use super::handoff;
use super::windows::{is_click_through, set_click_through};
use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    os::windows::ffi::OsStringExt,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, OnceLock,
    },
};

type Handle = isize;
type Bool = i32;
type HResult = i32;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}
const fn ole_guid(data1: u32) -> Guid {
    Guid {
        data1,
        data2: 0,
        data3: 0,
        data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
    }
}
const IID_IUNKNOWN: Guid = ole_guid(0x0000_0000);
const IID_IDATAOBJECT: Guid = ole_guid(0x0000_010E);
const IID_IDROPSOURCE: Guid = ole_guid(0x0000_0121);

#[repr(C)]
#[derive(Clone, Copy)]
struct FormatEtc {
    format: u16,
    device: *mut c_void,
    aspect: u32,
    index: i32,
    tymed: u32,
}
#[repr(C)]
struct StgMedium {
    tymed: u32,
    global: Handle,
    release: *mut c_void,
}
#[repr(C, packed(1))]
#[derive(Clone, Copy)]
struct FileDescriptorW {
    flags: u32,
    clsid: Guid,
    size: [i32; 2],
    point: [i32; 2],
    attributes: u32,
    created: [u32; 2],
    accessed: [u32; 2],
    written: [u32; 2],
    size_high: u32,
    size_low: u32,
    name: [u16; 260],
}
#[repr(C, packed(1))]
struct FileGroupDescriptorW {
    count: u32,
    files: [FileDescriptorW; 1],
}
const _: () = assert!(std::mem::size_of::<FileGroupDescriptorW>() == 596);
#[repr(C, packed(1))]
struct ChangeNotifyEntry {
    pidl: *mut c_void,
    recursive: Bool,
}
#[repr(C)]
struct WndClassW {
    style: u32,
    procedure: extern "system" fn(Handle, u32, usize, isize) -> isize,
    class_extra: i32,
    window_extra: i32,
    instance: Handle,
    icon: Handle,
    cursor: Handle,
    background: Handle,
    menu: *const u16,
    class: *const u16,
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(class: *const WndClassW) -> u16;
    fn CreateWindowExW(
        ex_style: u32,
        class: *const u16,
        name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Handle,
        menu: Handle,
        instance: Handle,
        param: *mut c_void,
    ) -> Handle;
    fn DefWindowProcW(window: Handle, message: u32, wparam: usize, lparam: isize) -> isize;
    fn PostMessageW(window: Handle, message: u32, wparam: usize, lparam: isize) -> Bool;
    fn SetTimer(window: Handle, id: usize, milliseconds: u32, callback: *const c_void) -> usize;
    fn KillTimer(window: Handle, id: usize) -> Bool;
    fn RegisterClipboardFormatW(name: *const u16) -> u32;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Handle;
    fn GlobalAlloc(flags: u32, bytes: usize) -> Handle;
    fn GlobalLock(memory: Handle) -> *mut c_void;
    fn GlobalUnlock(memory: Handle) -> Bool;
}
#[link(name = "ole32")]
extern "system" {
    fn OleInitialize(reserved: *mut c_void) -> HResult;
    fn OleUninitialize();
    fn DoDragDrop(
        data: *mut c_void,
        source: *mut c_void,
        allowed: u32,
        effect: *mut u32,
    ) -> HResult;
    fn CoCreateGuid(guid: *mut Guid) -> HResult;
    fn CoTaskMemFree(memory: *mut c_void);
}
#[link(name = "shell32")]
extern "system" {
    fn SHCreateStdEnumFmtEtc(
        count: u32,
        formats: *const FormatEtc,
        out: *mut *mut c_void,
    ) -> HResult;
    fn SHGetSpecialFolderLocation(window: Handle, folder: i32, pidl: *mut *mut c_void) -> HResult;
    fn SHChangeNotifyRegister(
        window: Handle,
        sources: i32,
        events: i32,
        message: u32,
        count: i32,
        entries: *const ChangeNotifyEntry,
    ) -> u32;
    fn SHChangeNotifyDeregister(registration: u32) -> Bool;
    fn SHChangeNotification_Lock(
        change: Handle,
        process: u32,
        pidls: *mut *mut *mut c_void,
        event: *mut i32,
    ) -> Handle;
    fn SHChangeNotification_Unlock(lock: Handle) -> Bool;
    fn SHGetPathFromIDListW(pidl: *const c_void, path: *mut u16) -> Bool;
}

const S_OK: HResult = 0;
const E_NOTIMPL: HResult = 0x8000_4001_u32 as i32;
const E_NOINTERFACE: HResult = 0x8000_4002_u32 as i32;
const E_POINTER: HResult = 0x8000_4003_u32 as i32;
const E_OUTOFMEMORY: HResult = 0x8007_000E_u32 as i32;
const DV_E_FORMATETC: HResult = 0x8004_0064_u32 as i32;
const OLE_E_ADVISENOTSUPPORTED: HResult = 0x8004_0003_u32 as i32;
const DRAGDROP_S_DROP: HResult = 0x0004_0100;
const DRAGDROP_S_CANCEL: HResult = 0x0004_0101;
const DRAGDROP_S_USEDEFAULTCURSORS: HResult = 0x0004_0102;
const DROPEFFECT_COPY: u32 = 1;
const MK_LBUTTON: u32 = 1;
const DATADIR_GET: u32 = 1;
const DVASPECT_CONTENT: u32 = 1;
const TYMED_HGLOBAL: u32 = 1;
const GMEM_MOVEABLE: u32 = 2;
const FD_ATTRIBUTES: u32 = 0x4;
const FD_FILESIZE: u32 = 0x40;
const FD_UNICODE: u32 = 0x8000_0000;
const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
const CSIDL_DESKTOP: i32 = 0;
const SHCNRF_SHELL_LEVEL: i32 = 0x2;
const SHCNRF_NEW_DELIVERY: i32 = 0x8000;
const SHCNE_RENAMEITEM: i32 = 0x1;
const SHCNE_CREATE: i32 = 0x2;
const SHCNE_UPDATEDIR: i32 = 0x1000;
const SHCNE_UPDATEITEM: i32 = 0x2000;
const HWND_MESSAGE: Handle = -3;
const MAX_PATH: usize = 260;
const WM_TIMER: u32 = 0x0113;
const WM_APP: u32 = 0x8000;
const WM_PERCH_DRAG: u32 = WM_APP + 1;
const WM_PERCH_CHANGED: u32 = WM_APP + 2;
const GIVE_UP_TIMER: usize = 1;
/// How long after the drop Explorer has to write the file. A synchronous
/// drop has written it before the drag even returns.
const GIVE_UP_AFTER_MS: u32 = 15_000;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

struct Formats {
    descriptor: u16,
    contents: u16,
}
fn formats() -> &'static Formats {
    static FORMATS: OnceLock<Formats> = OnceLock::new();
    FORMATS.get_or_init(|| unsafe {
        Formats {
            descriptor: RegisterClipboardFormatW(wide("FileGroupDescriptorW").as_ptr()) as u16,
            contents: RegisterClipboardFormatW(wide("FileContents").as_ptr()) as u16,
        }
    })
}

/// The one virtual file the drag offers. Explorer reads its descriptor and
/// contents and writes the file itself; it cannot replace anything, because
/// its name is a fresh GUID.
#[repr(C)]
struct DataObject {
    vtbl: &'static DataObjectVtbl,
    refs: AtomicU32,
    descriptor: Vec<u8>,
    fetched: Arc<AtomicBool>,
}
#[repr(C)]
struct DataObjectVtbl {
    query_interface:
        unsafe extern "system" fn(*mut DataObject, *const Guid, *mut *mut c_void) -> HResult,
    add_ref: unsafe extern "system" fn(*mut DataObject) -> u32,
    release: unsafe extern "system" fn(*mut DataObject) -> u32,
    get_data:
        unsafe extern "system" fn(*mut DataObject, *const FormatEtc, *mut StgMedium) -> HResult,
    get_data_here:
        unsafe extern "system" fn(*mut DataObject, *const FormatEtc, *mut StgMedium) -> HResult,
    query_get_data: unsafe extern "system" fn(*mut DataObject, *const FormatEtc) -> HResult,
    get_canonical_format_etc:
        unsafe extern "system" fn(*mut DataObject, *const FormatEtc, *mut FormatEtc) -> HResult,
    set_data: unsafe extern "system" fn(
        *mut DataObject,
        *const FormatEtc,
        *const StgMedium,
        Bool,
    ) -> HResult,
    enum_format_etc: unsafe extern "system" fn(*mut DataObject, u32, *mut *mut c_void) -> HResult,
    d_advise: unsafe extern "system" fn(
        *mut DataObject,
        *const FormatEtc,
        u32,
        *mut c_void,
        *mut u32,
    ) -> HResult,
    d_unadvise: unsafe extern "system" fn(*mut DataObject, u32) -> HResult,
    enum_d_advise: unsafe extern "system" fn(*mut DataObject, *mut *mut c_void) -> HResult,
}
static DATA_OBJECT_VTBL: DataObjectVtbl = DataObjectVtbl {
    query_interface: data_query_interface,
    add_ref: data_add_ref,
    release: data_release,
    get_data: data_get,
    get_data_here: data_get_here,
    query_get_data: data_query_get,
    get_canonical_format_etc: data_canonical,
    set_data: data_set,
    enum_format_etc: data_enum,
    d_advise: data_advise,
    d_unadvise: data_unadvise,
    enum_d_advise: data_enum_advise,
};

impl DataObject {
    fn create(name: &str, fetched: Arc<AtomicBool>) -> *mut DataObject {
        let mut file_name = [0_u16; 260];
        for (slot, unit) in file_name.iter_mut().zip(name.encode_utf16()) {
            *slot = unit;
        }
        let group = FileGroupDescriptorW {
            count: 1,
            files: [FileDescriptorW {
                flags: FD_ATTRIBUTES | FD_FILESIZE | FD_UNICODE,
                clsid: Guid {
                    data1: 0,
                    data2: 0,
                    data3: 0,
                    data4: [0; 8],
                },
                size: [0; 2],
                point: [0; 2],
                attributes: FILE_ATTRIBUTE_HIDDEN,
                created: [0; 2],
                accessed: [0; 2],
                written: [0; 2],
                size_high: 0,
                size_low: handoff::MARKER.len() as u32,
                name: file_name,
            }],
        };
        let descriptor = unsafe {
            std::slice::from_raw_parts(
                &group as *const FileGroupDescriptorW as *const u8,
                std::mem::size_of::<FileGroupDescriptorW>(),
            )
        }
        .to_vec();
        Box::into_raw(Box::new(DataObject {
            vtbl: &DATA_OBJECT_VTBL,
            refs: AtomicU32::new(1),
            descriptor,
            fetched,
        }))
    }
}

fn offered() -> [FormatEtc; 2] {
    let f = formats();
    let entry = |format, index| FormatEtc {
        format,
        device: std::ptr::null_mut(),
        aspect: DVASPECT_CONTENT,
        index,
        tymed: TYMED_HGLOBAL,
    };
    [entry(f.descriptor, -1), entry(f.contents, 0)]
}

/// Which of the offered payloads `format` asks for, if any, and whether it is
/// the file's contents.
unsafe fn requested(this: &DataObject, format: *const FormatEtc) -> Option<(&[u8], bool)> {
    let format = format.as_ref()?;
    if format.tymed & TYMED_HGLOBAL == 0 || format.aspect != DVASPECT_CONTENT {
        return None;
    }
    let f = formats();
    if format.format == f.descriptor {
        Some((&this.descriptor, false))
    } else if format.format == f.contents && (format.index == 0 || format.index == -1) {
        Some((handoff::MARKER, true))
    } else {
        None
    }
}

unsafe extern "system" fn data_query_interface(
    this: *mut DataObject,
    iid: *const Guid,
    out: *mut *mut c_void,
) -> HResult {
    if out.is_null() || iid.is_null() {
        return E_POINTER;
    }
    if *iid == IID_IUNKNOWN || *iid == IID_IDATAOBJECT {
        data_add_ref(this);
        *out = this.cast();
        S_OK
    } else {
        *out = std::ptr::null_mut();
        E_NOINTERFACE
    }
}
unsafe extern "system" fn data_add_ref(this: *mut DataObject) -> u32 {
    (*this).refs.fetch_add(1, Ordering::AcqRel) + 1
}
unsafe extern "system" fn data_release(this: *mut DataObject) -> u32 {
    let left = (*this).refs.fetch_sub(1, Ordering::AcqRel) - 1;
    if left == 0 {
        drop(Box::from_raw(this));
    }
    left
}
unsafe extern "system" fn data_get(
    this: *mut DataObject,
    format: *const FormatEtc,
    medium: *mut StgMedium,
) -> HResult {
    if medium.is_null() {
        return E_POINTER;
    }
    let this = &*this;
    let Some((bytes, contents)) = requested(this, format) else {
        return DV_E_FORMATETC;
    };
    if contents {
        this.fetched.store(true, Ordering::Release);
    }
    let global = GlobalAlloc(GMEM_MOVEABLE, bytes.len());
    if global == 0 {
        return E_OUTOFMEMORY;
    }
    let target = GlobalLock(global);
    if target.is_null() {
        return E_OUTOFMEMORY;
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), target.cast(), bytes.len());
    GlobalUnlock(global);
    *medium = StgMedium {
        tymed: TYMED_HGLOBAL,
        global,
        release: std::ptr::null_mut(),
    };
    S_OK
}
unsafe extern "system" fn data_get_here(
    _: *mut DataObject,
    _: *const FormatEtc,
    _: *mut StgMedium,
) -> HResult {
    E_NOTIMPL
}
unsafe extern "system" fn data_query_get(
    this: *mut DataObject,
    format: *const FormatEtc,
) -> HResult {
    if requested(&*this, format).is_some() {
        S_OK
    } else {
        DV_E_FORMATETC
    }
}
unsafe extern "system" fn data_canonical(
    _: *mut DataObject,
    _: *const FormatEtc,
    out: *mut FormatEtc,
) -> HResult {
    if let Some(out) = out.as_mut() {
        out.device = std::ptr::null_mut();
    }
    E_NOTIMPL
}
unsafe extern "system" fn data_set(
    _: *mut DataObject,
    _: *const FormatEtc,
    _: *const StgMedium,
    _: Bool,
) -> HResult {
    E_NOTIMPL
}
unsafe extern "system" fn data_enum(
    _: *mut DataObject,
    direction: u32,
    out: *mut *mut c_void,
) -> HResult {
    if out.is_null() {
        return E_POINTER;
    }
    if direction != DATADIR_GET {
        *out = std::ptr::null_mut();
        return E_NOTIMPL;
    }
    let formats = offered();
    SHCreateStdEnumFmtEtc(formats.len() as u32, formats.as_ptr(), out)
}
unsafe extern "system" fn data_advise(
    _: *mut DataObject,
    _: *const FormatEtc,
    _: u32,
    _: *mut c_void,
    _: *mut u32,
) -> HResult {
    OLE_E_ADVISENOTSUPPORTED
}
unsafe extern "system" fn data_unadvise(_: *mut DataObject, _: u32) -> HResult {
    OLE_E_ADVISENOTSUPPORTED
}
unsafe extern "system" fn data_enum_advise(_: *mut DataObject, out: *mut *mut c_void) -> HResult {
    if let Some(out) = out.as_mut() {
        *out = std::ptr::null_mut();
    }
    OLE_E_ADVISENOTSUPPORTED
}

/// Drives the drag: it drops when the button comes up and cancels on Escape.
#[repr(C)]
struct DropSource {
    vtbl: &'static DropSourceVtbl,
    refs: AtomicU32,
    /// A drag that starts after the button was already released cancels
    /// instead of dropping wherever the pointer happens to be.
    pressed: Cell<bool>,
}
#[repr(C)]
struct DropSourceVtbl {
    query_interface:
        unsafe extern "system" fn(*mut DropSource, *const Guid, *mut *mut c_void) -> HResult,
    add_ref: unsafe extern "system" fn(*mut DropSource) -> u32,
    release: unsafe extern "system" fn(*mut DropSource) -> u32,
    query_continue_drag: unsafe extern "system" fn(*mut DropSource, Bool, u32) -> HResult,
    give_feedback: unsafe extern "system" fn(*mut DropSource, u32) -> HResult,
}
static DROP_SOURCE_VTBL: DropSourceVtbl = DropSourceVtbl {
    query_interface: source_query_interface,
    add_ref: source_add_ref,
    release: source_release,
    query_continue_drag: source_continue,
    give_feedback: source_feedback,
};
unsafe extern "system" fn source_query_interface(
    this: *mut DropSource,
    iid: *const Guid,
    out: *mut *mut c_void,
) -> HResult {
    if out.is_null() || iid.is_null() {
        return E_POINTER;
    }
    if *iid == IID_IUNKNOWN || *iid == IID_IDROPSOURCE {
        source_add_ref(this);
        *out = this.cast();
        S_OK
    } else {
        *out = std::ptr::null_mut();
        E_NOINTERFACE
    }
}
unsafe extern "system" fn source_add_ref(this: *mut DropSource) -> u32 {
    (*this).refs.fetch_add(1, Ordering::AcqRel) + 1
}
unsafe extern "system" fn source_release(this: *mut DropSource) -> u32 {
    let left = (*this).refs.fetch_sub(1, Ordering::AcqRel) - 1;
    if left == 0 {
        drop(Box::from_raw(this));
    }
    left
}
/// OLE passes the button state it tracks for the drag; Perch reads none itself.
unsafe extern "system" fn source_continue(
    this: *mut DropSource,
    escape: Bool,
    keys: u32,
) -> HResult {
    let this = &*this;
    if escape != 0 {
        return DRAGDROP_S_CANCEL;
    }
    if keys & MK_LBUTTON != 0 {
        this.pressed.set(true);
        return S_OK;
    }
    if this.pressed.get() {
        DRAGDROP_S_DROP
    } else {
        DRAGDROP_S_CANCEL
    }
}
unsafe extern "system" fn source_feedback(_: *mut DropSource, _: u32) -> HResult {
    DRAGDROP_S_USEDEFAULTCURSORS
}

/// A handoff waiting to learn where its file landed.
struct Watch {
    pet: u64,
    name: String,
    registration: u32,
    root: *mut c_void,
}
thread_local! {
    static RECEIVER: Cell<Handle> = const { Cell::new(0) };
    static WATCH: RefCell<Option<Watch>> = const { RefCell::new(None) };
}

/// Start a handoff drag for `pet` from the overlay. Runs on the overlay's
/// window thread. The drag itself runs from a posted message, outside the
/// toolkit's event handler, so the toolkit keeps delivering events, and the
/// pet keeps following the cursor, while the drag's modal loop runs.
pub fn begin(overlay: Handle, pet: u64) -> Result<(), String> {
    let receiver = receiver()?;
    if unsafe { PostMessageW(receiver, WM_PERCH_DRAG, pet as usize, overlay) } == 0 {
        return Err("The folder drag could not start.".into());
    }
    Ok(())
}

/// A message-only window on the UI thread: it runs the drag and receives the
/// shell's change notifications.
fn receiver() -> Result<Handle, String> {
    let existing = RECEIVER.get();
    if existing != 0 {
        return Ok(existing);
    }
    let class = wide("PerchFolderHandoff");
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let definition = WndClassW {
            style: 0,
            procedure: receive,
            class_extra: 0,
            window_extra: 0,
            instance,
            icon: 0,
            cursor: 0,
            background: 0,
            menu: std::ptr::null(),
            class: class.as_ptr(),
        };
        RegisterClassW(&definition);
        let window = CreateWindowExW(
            0,
            class.as_ptr(),
            std::ptr::null(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            0,
            instance,
            std::ptr::null_mut(),
        );
        if window == 0 {
            return Err("The folder drag could not start.".into());
        }
        RECEIVER.set(window);
        Ok(window)
    }
}

extern "system" fn receive(window: Handle, message: u32, wparam: usize, lparam: isize) -> isize {
    match message {
        WM_PERCH_DRAG => drag(window, lparam, wparam as u64),
        WM_PERCH_CHANGED => changed(window, wparam as Handle, lparam as u32),
        WM_TIMER if wparam == GIVE_UP_TIMER => give_up(window),
        _ => return unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
    0
}

fn drag(receiver: Handle, overlay: Handle, pet: u64) {
    // A new drag replaces a handoff still waiting for its file.
    stop_watching(receiver);
    let started = unique().and_then(|unique| {
        let name = handoff::file_name(&unique);
        watch(receiver, pet, &name).map(|()| name)
    });
    let name = match started {
        Ok(name) => name,
        Err(error) => {
            super::pet_ui::deliver(serde_json::json!({"type": "error", "message": error}));
            super::pet_ui::deliver(
                serde_json::json!({"type": "dragEnded", "petId": pet, "accepted": false}),
            );
            return;
        }
    };
    let fetched = Arc::new(AtomicBool::new(false));
    let data = DataObject::create(&name, fetched.clone());
    let source = Box::into_raw(Box::new(DropSource {
        vtbl: &DROP_SOURCE_VTBL,
        refs: AtomicU32::new(1),
        pressed: Cell::new(false),
    }));
    // The overlay covers the whole screen. While it catches the pointer, the
    // drop would land on the overlay itself, so it lets the pointer through
    // for the drag and gets its own state back afterwards.
    let was_click_through = is_click_through(overlay);
    set_click_through(overlay, true);
    let mut effect = 0_u32;
    let result = unsafe {
        let ole = OleInitialize(std::ptr::null_mut());
        let result = DoDragDrop(data.cast(), source.cast(), DROPEFFECT_COPY, &mut effect);
        if ole >= 0 {
            OleUninitialize();
        }
        data_release(data);
        source_release(source);
        result
    };
    set_click_through(overlay, was_click_through);
    super::pet_ui::deliver(
        serde_json::json!({"type": "dragEnded", "petId": pet, "accepted": effect != 0}),
    );

    let waiting = WATCH.with_borrow(|w| w.as_ref().is_some_and(|w| w.name == name));
    if !waiting {
        // Explorer already wrote the file during the drop and it is bound.
        return;
    }
    let dropped = result == DRAGDROP_S_DROP && (effect != 0 || fetched.load(Ordering::Acquire));
    if dropped {
        unsafe { SetTimer(receiver, GIVE_UP_TIMER, GIVE_UP_AFTER_MS, std::ptr::null()) };
    } else {
        stop_watching(receiver);
    }
}

fn unique() -> Result<String, String> {
    let mut g = Guid {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    if unsafe { CoCreateGuid(&mut g) } < 0 {
        return Err("The folder drag could not start.".into());
    }
    let d = g.data4;
    Ok(format!(
        "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        g.data1, g.data2, g.data3, d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]
    ))
}

/// Listen, for the length of this handoff, to the shell's notifications of
/// new and changed items anywhere under the desktop.
fn watch(receiver: Handle, pet: u64, name: &str) -> Result<(), String> {
    let failed = || "Windows could not watch for the folder drop.".to_string();
    unsafe {
        let mut root = std::ptr::null_mut();
        if SHGetSpecialFolderLocation(0, CSIDL_DESKTOP, &mut root) < 0 || root.is_null() {
            return Err(failed());
        }
        let entry = ChangeNotifyEntry {
            pidl: root,
            recursive: 1,
        };
        let registration = SHChangeNotifyRegister(
            receiver,
            SHCNRF_SHELL_LEVEL | SHCNRF_NEW_DELIVERY,
            SHCNE_CREATE | SHCNE_RENAMEITEM | SHCNE_UPDATEITEM | SHCNE_UPDATEDIR,
            WM_PERCH_CHANGED,
            1,
            &entry,
        );
        if registration == 0 {
            CoTaskMemFree(root);
            return Err(failed());
        }
        WATCH.set(Some(Watch {
            pet,
            name: name.to_string(),
            registration,
            root,
        }));
    }
    Ok(())
}

fn stop_watching(receiver: Handle) {
    unsafe { KillTimer(receiver, GIVE_UP_TIMER) };
    if let Some(watch) = WATCH.take() {
        unsafe {
            SHChangeNotifyDeregister(watch.registration);
            CoTaskMemFree(watch.root);
        }
    }
}

fn changed(receiver: Handle, change: Handle, process: u32) {
    let mut paths = Vec::new();
    unsafe {
        let mut pidls: *mut *mut c_void = std::ptr::null_mut();
        let mut event = 0;
        let lock = SHChangeNotification_Lock(change, process, &mut pidls, &mut event);
        if lock == 0 {
            return;
        }
        if !pidls.is_null() {
            for i in 0..2 {
                let pidl = *pidls.add(i);
                let mut buffer = [0_u16; MAX_PATH];
                if !pidl.is_null() && SHGetPathFromIDListW(pidl, buffer.as_mut_ptr()) != 0 {
                    let length = buffer.iter().position(|&c| c == 0).unwrap_or(MAX_PATH);
                    paths.push(PathBuf::from(std::ffi::OsString::from_wide(
                        &buffer[..length],
                    )));
                }
            }
        }
        SHChangeNotification_Unlock(lock);
    }
    let found = WATCH.with_borrow(|watch| {
        let watch = watch.as_ref()?;
        let folder = handoff::landed_in(&watch.name, &paths, |p| p.is_file())?;
        Some((watch.pet, folder.join(&watch.name), folder))
    });
    let Some((pet, file, folder)) = found else {
        return;
    };
    stop_watching(receiver);
    super::pet_ui::deliver(serde_json::json!({
        "type": "bind",
        "petId": pet,
        "folder": folder.to_string_lossy(),
    }));
    handoff::remove_later(file);
}

fn give_up(receiver: Handle) {
    let waiting = WATCH.with_borrow(|w| w.is_some());
    stop_watching(receiver);
    if waiting {
        super::pet_ui::deliver(serde_json::json!({
            "type": "error",
            "message": "Windows did not report where the pet was dropped. Choose the folder in the Assistant instead.",
        }));
    }
}
