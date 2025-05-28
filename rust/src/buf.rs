use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_uchar, c_void};
use std::hash::BuildHasherDefault;
use std::ptr;
use std::sync::{Mutex, OnceLock};

// Logging macro for debugging
// disable for now
#[allow(unused_macros)]
macro_rules! log_buf {
    ($($arg:tt)*) => {
        // eprintln!("[RUST_BUF] {}", format!($($arg)*));
    };
}

// Type definitions matching libxml2
pub type XmlChar = c_uchar;
pub type XmlBufPtr = usize;

// Error flags for buffer state
const BUF_FLAG_OOM: u32 = 1 << 0;
const BUF_FLAG_OVERFLOW: u32 = 1 << 1;
const BUF_FLAG_STATIC: u32 = 1 << 2;

// reference xmlFree and xmlMalloc function pointers
type XmlFreeFunc = unsafe extern "C" fn(*mut c_void);
type XmlMallocFunc = unsafe extern "C" fn(usize) -> *mut c_void;
type XmlReallocFunc = unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void;

extern "C" {
    static xmlFree: XmlFreeFunc;
    static xmlMalloc: XmlMallocFunc;
    static xmlRealloc: XmlReallocFunc;
}

// Forward declarations for C types we need to interface with
#[repr(C)]
pub struct XmlBuffer {
    pub content: *mut XmlChar,
    pub use_: u32,
    pub size: u32,
    pub alloc: c_int,
    pub content_io: *mut XmlChar,
}

// void for placeholders
type VoidPtr = *mut c_void;

#[repr(C)]
pub struct XmlParserInput {
   pub buf: VoidPtr,
   pub filename: *const c_char,
   pub directory: *const c_char,
   pub base: *const XmlChar,
   pub cur: *const XmlChar,
   pub end: *const XmlChar,
   pub length: c_int,
   pub line: c_int,
   pub col: c_int,
   pub consumed: u64,
   pub free: VoidPtr,
   pub encoding: *const XmlChar,
   pub version: *const XmlChar,
   pub flags: c_int,
   pub id: c_int,
   pub parent_consumed: u64,
   pub entity: *mut c_void,
}

// Rust buffer structure
// derive Debug
#[derive(Debug)]
pub struct XmlBuf {
    content: Vec<u8>,
    use_: usize,
    size: usize,
    max_size: usize,
    flags: u32,
    content_offset: usize,     // Offset from start of Vec to current content
    static_mem: Option<usize>, // For static buffers - store as usize for Send/Sync
}

impl XmlBuf {
    fn new(size: usize) -> Result<Self, ()> {
        if size == usize::MAX {
            return Err(());
        }

        let mut content = Vec::with_capacity(size + 1);
        content.resize(size + 1, 0);
        content[0] = 0; // Null terminate

        Ok(XmlBuf {
            content,
            use_: 0,
            size,
            max_size: usize::MAX - 1,
            flags: 0,
            content_offset: 0,
            static_mem: None,
        })
    }

    fn new_from_mem(mem: *const XmlChar, size: usize, is_static: bool) -> Result<Self, ()> {
        if mem.is_null() {
            return Err(());
        }

        if is_static {
            // Check that memory is zero-terminated
            unsafe {
                if *mem.add(size) != 0 {
                    return Err(());
                }
            }

            Ok(XmlBuf {
                content: Vec::new(), // Not used for static buffers
                use_: size,
                size,
                max_size: usize::MAX - 1,
                flags: BUF_FLAG_STATIC,
                content_offset: 0,
                static_mem: Some(mem as usize),
            })
        } else {
            let mut content = Vec::with_capacity(size + 1);
            unsafe {
                let slice = std::slice::from_raw_parts(mem, size);
                content.extend_from_slice(slice);
            }
            content.push(0); // Null terminate

            Ok(XmlBuf {
                content,
                use_: size,
                size,
                max_size: usize::MAX - 1,
                flags: 0,
                content_offset: 0,
                static_mem: None,
            })
        }
    }

    fn is_error(&self) -> bool {
        (self.flags & (BUF_FLAG_OOM | BUF_FLAG_OVERFLOW)) != 0
    }

    fn is_static(&self) -> bool {
        (self.flags & BUF_FLAG_STATIC) != 0
    }

    fn set_overflow(&mut self) {
        if !self.is_error() {
            self.flags |= BUF_FLAG_OVERFLOW;
        }
    }

    fn empty(&mut self) {
        if self.is_error() || self.is_static() {
            return;
        }

        self.use_ = 0;
        self.size += self.content_offset;
        self.content_offset = 0;
        if !self.content.is_empty() {
            self.content[0] = 0;
        }
    }

    fn grow(&mut self, len: usize) -> Result<(), ()> {
        if self.is_error() || self.is_static() {
            return Err(());
        }

        if len <= self.size - self.use_ {
            return Ok(());
        }

        // Check if we can move content to beginning to make space
        if len <= self.content_offset + self.size - self.use_ {
            let content_start = self.content_offset;
            let content_end = content_start + self.use_ + 1;
            self.content.copy_within(content_start..content_end, 0);
            self.size += self.content_offset;
            self.content_offset = 0;
            return Ok(());
        }

        if len > self.max_size - self.use_ {
            self.set_overflow();
            return Err(());
        }

        let new_size = if self.size > len {
            if self.size <= self.max_size / 2 {
                self.size * 2
            } else {
                self.max_size
            }
        } else {
            let size = self.use_ + len;
            if size <= self.max_size - 100 {
                size + 100
            } else {
                size
            }
        };

        // Resize the Vec
        self.content.resize(new_size + 1, 0);

        // If we had offset content, move it to the beginning
        if self.content_offset > 0 {
            let content_start = self.content_offset;
            let content_end = content_start + self.use_ + 1;
            self.content.copy_within(content_start..content_end, 0);
            self.content_offset = 0;
        }

        self.size = new_size;
        Ok(())
    }

    fn add(&mut self, str_ptr: *const XmlChar, len: usize) -> Result<(), ()> {
        if self.is_error() || self.is_static() || str_ptr.is_null() {
            return Err(());
        }

        if len == 0 {
            return Ok(());
        }

        if len > self.size - self.use_ {
            self.grow(len)?;
        }

        unsafe {
            let src_slice = std::slice::from_raw_parts(str_ptr, len);
            let start_pos = self.content_offset + self.use_;
            self.content[start_pos..start_pos + len].copy_from_slice(src_slice);
        }

        self.use_ += len;
        self.content[self.content_offset + self.use_] = 0; // Null terminate

        Ok(())
    }

    fn cat(&mut self, str_ptr: *const XmlChar) -> Result<(), ()> {
        if str_ptr.is_null() {
            return Ok(());
        }

        let len = unsafe { libc::strlen(str_ptr as *const c_char) };

        self.add(str_ptr, len)
    }

    fn avail(&self) -> usize {
        if self.is_error() {
            return 0;
        }
        self.size - self.use_
    }

    fn is_empty(&self) -> bool {
        self.use_ == 0
    }

    fn add_len(&mut self, len: usize) -> Result<(), ()> {
        if self.is_error() || self.is_static() {
            return Err(());
        }

        if len > self.size - self.use_ {
            return Err(());
        }

        self.use_ += len;
        self.content[self.content_offset + self.use_] = 0;
        Ok(())
    }

    fn detach(&mut self) -> Result<*mut XmlChar, ()> {
        if self.is_error() || self.is_static() {
            return Err(());
        }

        // Always allocate with xmlMalloc since the memory will be freed with xmlMemFree
        let result = unsafe {
            let ptr = xmlMalloc(self.use_ + 1) as *mut XmlChar;
            if ptr.is_null() {
                return Err(());
            }
            libc::memcpy(
                ptr as *mut c_void,
                self.content.as_ptr().add(self.content_offset) as *const c_void,
                self.use_ + 1,
            );
            ptr
        };

        // Clear buffer
        self.content.clear();
        self.use_ = 0;
        self.size = 0;
        self.content_offset = 0;

        Ok(result)
    }

    fn content_ptr(&self) -> *const XmlChar {
        if self.is_error() {
            return ptr::null();
        }

        if self.is_static() {
            self.static_mem
                .map(|addr| addr as *const XmlChar)
                .unwrap_or(ptr::null())
        } else {
            self.content.as_ptr().wrapping_add(self.content_offset)
        }
    }
}

// Global storage for buffer handles
static BUFFERS: OnceLock<
    Mutex<HashMap<XmlBufPtr, Box<XmlBuf>, BuildHasherDefault<DefaultHasher>>>,
> = OnceLock::new();

fn get_buffers(
) -> &'static Mutex<HashMap<XmlBufPtr, Box<XmlBuf>, BuildHasherDefault<DefaultHasher>>> {
    BUFFERS.get_or_init(|| Mutex::new(HashMap::with_hasher(BuildHasherDefault::new())))
}

fn next_handle() -> XmlBufPtr {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(5);
    COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

// FFI functions matching the C API

#[no_mangle]
pub extern "C" fn xmlBufCreate(size: usize) -> XmlBufPtr {
    log_buf!("xmlBufCreate(size={})", size);

    let buf = match XmlBuf::new(size) {
        Ok(buf) => buf,
        Err(()) => {
            log_buf!("xmlBufCreate FAILED - could not create buffer");
            return 0;
        }
    };

    let mutex = get_buffers();
    let mut m = mutex.lock().unwrap();
    let handle = next_handle();
    m.insert(handle, Box::new(buf));

    log_buf!("xmlBufCreate SUCCESS - handle={}", handle);
    handle
}

#[no_mangle]
pub extern "C" fn xmlBufCreateMem(mem: *const XmlChar, size: usize, is_static: c_int) -> XmlBufPtr {
    log_buf!(
        "xmlBufCreateMem(mem={:p}, size={}, is_static={})",
        mem,
        size,
        is_static
    );

    let buf = match XmlBuf::new_from_mem(mem, size, is_static != 0) {
        Ok(buf) => buf,
        Err(()) => {
            log_buf!("xmlBufCreateMem FAILED");
            return 0;
        }
    };

    let mutex = get_buffers();
    let mut m = mutex.lock().unwrap();
    let handle = next_handle();
    m.insert(handle, Box::new(buf));

    log_buf!("xmlBufCreateMem SUCCESS - handle={}", handle);
    handle
}

#[no_mangle]
pub extern "C" fn xmlBufFree(buf: XmlBufPtr) {
    log_buf!("xmlBufFree(buf={})", buf);
    if buf == 0 {
        return;
    }

    let mutex = get_buffers();
    let mut m = mutex.lock().unwrap();
    m.remove(&buf);
}

#[no_mangle]
pub extern "C" fn xmlBufEmpty(buf: XmlBufPtr) {
    log_buf!("xmlBufEmpty(buf={})", buf);
    if buf == 0 {
        return;
    }

    let mutex = get_buffers();
    let mut m = mutex.lock().unwrap();
    if let Some(buffer) = m.get_mut(&buf) {
        buffer.empty();
    }
}

#[no_mangle]
pub extern "C" fn xmlBufGrow(buf: XmlBufPtr, len: usize) -> c_int {
    log_buf!("xmlBufGrow(buf={}, len={})", buf, len);
    if buf == 0 {
        return -1;
    }

    let mutex = get_buffers();
    let mut m = mutex.lock().unwrap();
    if let Some(buffer) = m.get_mut(&buf) {
        match buffer.grow(len) {
            Ok(()) => 0,
            Err(()) => -1,
        }
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn xmlBufAdd(buf: XmlBufPtr, str_ptr: *const XmlChar, len: usize) -> c_int {
    log_buf!("xmlBufAdd(buf={}, str_ptr={:p}, len={})", buf, str_ptr, len);

    if buf == 0 {
        log_buf!("xmlBufAdd FAILED - null handle");
        return -1;
    }

    let mutex = get_buffers();
    let mut m = mutex.lock().unwrap();
    if let Some(buffer) = m.get_mut(&buf) {
        match buffer.add(str_ptr, len) {
            Ok(()) => {
                log_buf!("xmlBufAdd SUCCESS - buffer now has {} bytes", buffer.use_);
                0
            }
            Err(()) => {
                log_buf!("xmlBufAdd FAILED - could not add content");
                -1
            }
        }
    } else {
        log_buf!("xmlBufAdd FAILED - handle {} not found", buf);
        -1
    }
}

#[no_mangle]
pub extern "C" fn xmlBufCat(buf: XmlBufPtr, str_ptr: *const XmlChar) -> c_int {
    log_buf!("xmlBufCat(buf={}, str_ptr={:p})", buf, str_ptr);

    if buf == 0 {
        return -1;
    }

    let mutex = get_buffers();
    let mut m = mutex.lock().unwrap();
    if let Some(buffer) = m.get_mut(&buf) {
        match buffer.cat(str_ptr) {
            Ok(()) => 0,
            Err(()) => -1,
        }
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn xmlBufAvail(buf: XmlBufPtr) -> usize {
    log_buf!("xmlBufAvail(buf={})", buf);
    if buf == 0 {
        return 0;
    }

    let mutex = get_buffers();
    let m = mutex.lock().unwrap();
    if let Some(buffer) = m.get(&buf) {
        buffer.avail()
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn xmlBufIsEmpty(buf: XmlBufPtr) -> c_int {
    log_buf!("xmlBufIsEmpty(buf={})", buf);
    if buf == 0 {
        return -1;
    }

    let mutex = get_buffers();
    let m = mutex.lock().unwrap();
    if let Some(buffer) = m.get(&buf) {
        if buffer.is_error() {
            -1
        } else if buffer.is_empty() {
            1
        } else {
            0
        }
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn xmlBufAddLen(buf: XmlBufPtr, len: usize) -> c_int {
    log_buf!("xmlBufAddLen(buf={}, len={})", buf, len);
    if buf == 0 {
        return -1;
    }

    let mutex = get_buffers();
    let mut m = mutex.lock().unwrap();
    if let Some(buffer) = m.get_mut(&buf) {
        match buffer.add_len(len) {
            Ok(()) => 0,
            Err(()) => -1,
        }
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn xmlBufDetach(buf: XmlBufPtr) -> *mut XmlChar {
    log_buf!("xmlBufDetach(buf={})", buf);
    if buf == 0 {
        return ptr::null_mut();
    }

    let mutex = get_buffers();
    let mut m = mutex.lock().unwrap();
    if let Some(buffer) = m.get_mut(&buf) {
        match buffer.detach() {
            Ok(ptr) => ptr,
            Err(()) => ptr::null_mut(),
        }
    } else {
        ptr::null_mut()
    }
}

// Additional functions exported by buf.c but not in the private header

#[no_mangle]
pub extern "C" fn xmlBufContent(buf: XmlBufPtr) -> *const XmlChar {
    log_buf!("xmlBufContent(buf={})", buf);

    if buf == 0 {
        log_buf!("xmlBufContent - NULL handle");
        return ptr::null();
    }

    let mutex = get_buffers();
    let m = mutex.lock().unwrap();
    if let Some(buffer) = m.get(&buf) {
        let ptr = buffer.content_ptr();
        log_buf!("xmlBufContent SUCCESS - ptr={:p}, use={}", ptr, buffer.use_);
        ptr
    } else {
        log_buf!("xmlBufContent FAILED - handle {} not found", buf);
        ptr::null()
    }
}

#[no_mangle]
pub extern "C" fn xmlBufEnd(buf: XmlBufPtr) -> *mut XmlChar {
    if buf == 0 {
        return ptr::null_mut();
    }

    let mutex = get_buffers();
    let m = mutex.lock().unwrap();
    if let Some(buffer) = m.get(&buf) {
        if buffer.is_error() {
            return ptr::null_mut();
        }

        if buffer.is_static() {
            return ptr::null_mut(); // Static buffers are read-only
        }

        buffer
            .content
            .as_ptr()
            .wrapping_add(buffer.content_offset + buffer.use_) as *mut XmlChar
    } else {
        ptr::null_mut()
    }
}

#[no_mangle]
pub extern "C" fn xmlBufUse(buf: XmlBufPtr) -> usize {
    if buf == 0 {
        return 0;
    }

    let mutex = get_buffers();
    let m = mutex.lock().unwrap();
    if let Some(buffer) = m.get(&buf) {
        if buffer.is_error() {
            return 0;
        }
        buffer.use_
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn xmlBufShrink(buf: XmlBufPtr, len: usize) -> usize {
    if buf == 0 {
        return 0;
    }

    let mutex = get_buffers();
    let mut m = mutex.lock().unwrap();
    if let Some(buffer) = m.get_mut(&buf) {
        if buffer.is_error() || len == 0 {
            return 0;
        }

        if len > buffer.use_ {
            return 0;
        }

        buffer.use_ -= len;
        buffer.content_offset += len;
        buffer.size -= len;

        len
    } else {
        0
    }
}

// Legacy xmlBuffer API functions

#[no_mangle]
pub extern "C" fn xmlBufferCreate() -> *mut XmlBuffer {
    log_buf!("xmlBufferCreate()");

    unsafe {
        let buffer = xmlMalloc(std::mem::size_of::<XmlBuffer>()) as *mut XmlBuffer;
        if buffer.is_null() {
            return ptr::null_mut();
        }

        let buf = &mut *buffer;
        buf.use_ = 0;
        buf.size = 256;
        buf.alloc = 1; // XML_BUFFER_ALLOC_IO
        buf.content_io = xmlMalloc(buf.size as usize) as *mut XmlChar;
        if buf.content_io.is_null() {
            xmlFree(buffer as *mut c_void);
            return ptr::null_mut();
        }
        buf.content = buf.content_io;
        *buf.content = 0;

        log_buf!("xmlBufferCreate SUCCESS - buffer={:p}", buffer);
        buffer
    }
}

#[no_mangle]
pub extern "C" fn xmlBufferCreateSize(size: usize) -> *mut XmlBuffer {
    if size >= i32::MAX as usize {
        return ptr::null_mut();
    }

    unsafe {
        let buffer = xmlMalloc(std::mem::size_of::<XmlBuffer>()) as *mut XmlBuffer;
        if buffer.is_null() {
            return ptr::null_mut();
        }

        let buf = &mut *buffer;
        buf.use_ = 0;
        buf.alloc = 1; // XML_BUFFER_ALLOC_IO
        buf.size = if size > 0 { size as u32 + 1 } else { 0 };

        if buf.size > 0 {
            buf.content_io = xmlMalloc(buf.size as usize) as *mut XmlChar;
            if buf.content_io.is_null() {
                xmlFree(buffer as *mut c_void);
                return ptr::null_mut();
            }
            buf.content = buf.content_io;
            *buf.content = 0;
        } else {
            buf.content_io = ptr::null_mut();
            buf.content = ptr::null_mut();
        }

        buffer
    }
}

#[no_mangle]
pub extern "C" fn xmlBufferCreateStatic(mem: *mut c_void, size: usize) -> *mut XmlBuffer {
    let buffer = xmlBufferCreateSize(size);
    if !buffer.is_null() {
        xmlBufferAdd(buffer, mem as *const XmlChar, size as c_int);
    }
    buffer
}

#[no_mangle]
pub extern "C" fn xmlBufferFree(buffer: *mut XmlBuffer) {
    if buffer.is_null() {
        return;
    }

    unsafe {
        let buf = &mut *buffer;
        if buf.alloc == 1 {
            // XML_BUFFER_ALLOC_IO
            xmlFree(buf.content_io as *mut c_void);
        } else {
            xmlFree(buf.content as *mut c_void);
        }
        xmlFree(buffer as *mut c_void);
    }
}

#[no_mangle]
pub extern "C" fn xmlBufferEmpty(buffer: *mut XmlBuffer) {
    if buffer.is_null() {
        return;
    }

    unsafe {
        let buf = &mut *buffer;
        if buf.content.is_null() {
            return;
        }

        buf.use_ = 0;

        if buf.alloc == 1 {
            // XML_BUFFER_ALLOC_IO
            buf.size += (buf.content as usize - buf.content_io as usize) as u32;
            buf.content = buf.content_io;
            *buf.content = 0;
        } else {
            *buf.content = 0;
        }
    }
}

#[no_mangle]
pub extern "C" fn xmlBufferContent(buffer: *const XmlBuffer) -> *const XmlChar {
    if buffer.is_null() {
        return ptr::null();
    }

    unsafe {
        let buf = &*buffer;
        buf.content
    }
}

#[no_mangle]
pub extern "C" fn xmlBufferLength(buffer: *const XmlBuffer) -> c_int {
    if buffer.is_null() {
        return 0;
    }

    unsafe {
        let buf = &*buffer;
        buf.use_ as c_int
    }
}

#[no_mangle]
pub extern "C" fn xmlBufferResize(buffer: *mut XmlBuffer, size: u32) -> c_int {
    if buffer.is_null() {
        return 0;
    }

    unsafe {
        let buf = &*buffer;
        if size < buf.size {
            return 1;
        }
        let res = xmlBufferGrow(buffer, size - buf.use_);
        if res < 0 {
            0
        } else {
            1
        }
    }
}

#[no_mangle]
pub extern "C" fn xmlBufferCat(buffer: *mut XmlBuffer, str_ptr: *const XmlChar) -> c_int {
    xmlBufferAdd(buffer, str_ptr, -1)
}

#[no_mangle]
pub extern "C" fn xmlBufferCCat(buffer: *mut XmlBuffer, str_ptr: *const c_char) -> c_int {
    xmlBufferAdd(buffer, str_ptr as *const XmlChar, -1)
}

#[no_mangle]
pub extern "C" fn xmlBufferAddHead(
    buffer: *mut XmlBuffer,
    str_ptr: *const XmlChar,
    len: c_int,
) -> c_int {
    if buffer.is_null() || str_ptr.is_null() {
        return -1;
    }

    let actual_len = if len < 0 {
        unsafe { libc::strlen(str_ptr as *const c_char) }
    } else {
        len as usize
    };

    if actual_len == 0 {
        return 0;
    }

    unsafe {
        let buf = &mut *buffer;

        // Ensure we have enough space
        if (actual_len as u32) >= buf.size - buf.use_ {
            if xmlBufferGrow(buffer, actual_len as u32) < 0 {
                return -1;
            }
        }

        // Move existing content to make room
        libc::memmove(
            buf.content.wrapping_add(actual_len) as *mut c_void,
            buf.content as *const c_void,
            buf.use_ as usize + 1,
        );
        libc::memmove(
            buf.content as *mut c_void,
            str_ptr as *const c_void,
            actual_len,
        );
        buf.use_ += actual_len as u32;

        0
    }
}

#[no_mangle]
pub extern "C" fn xmlBufferWriteCHAR(buffer: *mut XmlBuffer, string: *const XmlChar) {
    xmlBufferAdd(buffer, string, -1);
}

#[no_mangle]
pub extern "C" fn xmlBufferWriteChar(buffer: *mut XmlBuffer, string: *const c_char) {
    xmlBufferAdd(buffer, string as *const XmlChar, -1);
}

#[no_mangle]
pub extern "C" fn xmlBufferWriteQuotedString(buffer: *mut XmlBuffer, string: *const XmlChar) {
    if buffer.is_null() {
        return;
    }

    if string.is_null() {
        return;
    }

    unsafe {
        let s = std::ffi::CStr::from_ptr(string as *const c_char);
        let string_str = s.to_string_lossy();

        if string_str.contains('"') {
            if string_str.contains('\'') {
                xmlBufferCCat(buffer, "\"".as_ptr() as *const c_char);

                // Replace quotes with &quot;
                for ch in string_str.chars() {
                    if ch == '"' {
                        xmlBufferAdd(buffer, "&quot;".as_ptr(), 6);
                    } else {
                        let mut bytes = [0u8; 4];
                        let encoded = ch.encode_utf8(&mut bytes);
                        xmlBufferAdd(buffer, encoded.as_ptr(), encoded.len() as c_int);
                    }
                }

                xmlBufferCCat(buffer, "\"".as_ptr() as *const c_char);
            } else {
                xmlBufferCCat(buffer, "'".as_ptr() as *const c_char);
                xmlBufferCat(buffer, string);
                xmlBufferCCat(buffer, "'".as_ptr() as *const c_char);
            }
        } else {
            xmlBufferCCat(buffer, "\"".as_ptr() as *const c_char);
            xmlBufferCat(buffer, string);
            xmlBufferCCat(buffer, "\"".as_ptr() as *const c_char);
        }
    }
}

#[no_mangle]
pub extern "C" fn xmlBufferDump(_file: *mut libc::FILE, buffer: *mut XmlBuffer) -> c_int {
    if buffer.is_null() {
        return 0;
    }

    unsafe {
        let buf = &*buffer;
        if buf.content.is_null() {
            return 0;
        }

        libc::fwrite(buf.content as *const c_void, 1, buf.use_ as usize, _file);
        buf.use_ as c_int
    }
}

#[no_mangle]
pub extern "C" fn xmlSetBufferAllocationScheme(_scheme: c_int) {
    // No-op as allocation schemes were removed
}

#[no_mangle]
pub extern "C" fn xmlGetBufferAllocationScheme() -> c_int {
    1 // XML_BUFFER_ALLOC_EXACT
}

#[no_mangle]
pub extern "C" fn xmlBufferSetAllocationScheme(_buffer: *mut XmlBuffer, _scheme: c_int) {
    // No-op as allocation schemes were removed
}

#[no_mangle]
pub extern "C" fn xmlBufferAdd(
    buffer: *mut XmlBuffer,
    str_ptr: *const XmlChar,
    len: c_int,
) -> c_int {
    log_buf!(
        "xmlBufferAdd(buffer={:p}, str_ptr={:p}, len={})",
        buffer,
        str_ptr,
        len
    );

    if buffer.is_null() || str_ptr.is_null() {
        log_buf!("xmlBufferAdd FAILED - null args");
        return -1; // XML_ERR_ARGUMENT equivalent
    }

    let actual_len = if len < 0 {
        unsafe { libc::strlen(str_ptr as *const c_char) }
    } else {
        len as usize
    };

    if actual_len == 0 {
        return 0; // XML_ERR_OK equivalent
    }

    // For now, we'll implement a basic version that assumes xmlBuffer has enough space
    // A full implementation would need proper xmlBuffer structure handling
    unsafe {
        let buf = &mut *buffer;
        // This is a simplified implementation - real implementation would need proper buffer management
        if (buf.use_ as usize + actual_len) < buf.size as usize {
            libc::memcpy(
                buf.content.wrapping_add(buf.use_ as usize) as *mut c_void,
                str_ptr as *const c_void,
                actual_len,
            );
            buf.use_ += actual_len as u32;
            *buf.content.wrapping_add(buf.use_ as usize) = 0; // Null terminate
            log_buf!("xmlBufferAdd SUCCESS - buffer now has {} bytes", buf.use_);
            0 // XML_ERR_OK
        } else {
            -1 // XML_ERR_NO_MEMORY
        }
    }
}

#[no_mangle]
pub extern "C" fn xmlBufferDetach(buffer: *mut XmlBuffer) -> *mut XmlChar {
    if buffer.is_null() {
        return ptr::null_mut();
    }

    unsafe {
        let buf = &mut *buffer;
        let result = if buf.alloc == 1 && buf.content != buf.content_io {
            // XML_BUFFER_ALLOC_IO
            // Need to copy content
            let ptr = xmlMalloc(buf.use_ as usize + 1) as *mut XmlChar;
            if ptr.is_null() {
                return ptr::null_mut();
            }
            libc::memcpy(
                ptr as *mut c_void,
                buf.content as *const c_void,
                buf.use_ as usize + 1,
            );
            xmlFree(buf.content_io as *mut c_void);
            ptr
        } else {
            buf.content
        };

        // Clear buffer
        buf.content_io = ptr::null_mut();
        buf.content = ptr::null_mut();
        buf.size = 0;
        buf.use_ = 0;

        result
    }
}

#[no_mangle]
pub extern "C" fn xmlBufferGrow(buffer: *mut XmlBuffer, len: u32) -> c_int {
    if buffer.is_null() {
        return -1;
    }

    unsafe {
        let buf = &mut *buffer;
        if len < buf.size - buf.use_ {
            return 0;
        }

        // Simplified growth logic
        let new_size = if buf.size > len {
            if buf.size <= i32::MAX as u32 / 2 {
                buf.size * 2
            } else {
                i32::MAX as u32
            }
        } else {
            buf.use_ + len + 1
        };

        // Reallocate buffer - simplified version
        let new_buf =
            xmlRealloc(buf.content as *mut c_void, new_size as usize + 1) as *mut XmlChar;
        if new_buf.is_null() {
            return -1;
        }

        buf.content = new_buf;
        buf.size = new_size;
        (buf.size - buf.use_ - 1) as c_int
    }
}

#[no_mangle]
pub extern "C" fn xmlBufferShrink(buffer: *mut XmlBuffer, len: u32) -> c_int {
    if buffer.is_null() {
        return -1;
    }

    unsafe {
        let buf = &mut *buffer;
        if len == 0 || len > buf.use_ {
            return if len == 0 { 0 } else { -1 };
        }

        buf.use_ -= len;

        if buf.alloc == 1 {
            // XML_BUFFER_ALLOC_IO
            buf.content = buf.content.wrapping_add(len as usize);
            buf.size -= len;
        } else {
            // Move content to beginning
            libc::memmove(
                buf.content as *mut c_void,
                buf.content.wrapping_add(len as usize) as *const c_void,
                buf.use_ as usize + 1,
            );
        }

        len as c_int
    }
}

#[no_mangle]
pub extern "C" fn xmlBufFromBuffer(buffer: *mut XmlBuffer) -> XmlBufPtr {
    log_buf!("xmlBufFromBuffer(buffer={:p})", buffer);

    if buffer.is_null() {
        log_buf!("xmlBufFromBuffer FAILED - null buffer");
        return 0;
    }

    unsafe {
        let buf_struct = &*buffer;

        let xml_buf = if buf_struct.content.is_null() {
            // Create new buffer with default size
            match XmlBuf::new(50) {
                Ok(buf) => buf,
                Err(()) => return 0,
            }
        } else {
            // Create buffer from existing content
            let size = buf_struct.size as usize;
            let use_ = buf_struct.use_ as usize;

            let mut content = Vec::with_capacity(size + 1);
            if use_ > 0 {
                let slice = std::slice::from_raw_parts(buf_struct.content, use_);
                content.extend_from_slice(slice);
            }
            content.resize(size + 1, 0);

            XmlBuf {
                content,
                use_,
                size,
                max_size: usize::MAX - 1,
                flags: 0,
                content_offset: 0,
                static_mem: None,
            }
        };

        let mutex = get_buffers();
        let mut m = mutex.lock().unwrap();
        let handle = next_handle();
        m.insert(handle, Box::new(xml_buf));

        log_buf!("xmlBufFromBuffer SUCCESS - handle={}", handle);
        handle
    }
}

#[no_mangle]
pub extern "C" fn xmlBufBackToBuffer(buf: XmlBufPtr, ret: *mut XmlBuffer) -> c_int {
    log_buf!("xmlBufBackToBuffer(buf={}, ret={:p})", buf, ret);

    if buf == 0 || ret.is_null() {
        return -1;
    }

    let mutex = get_buffers();
    let mut m = mutex.lock().unwrap();

    if let Some(buffer) = m.remove(&buf) {
        if buffer.is_error() || buffer.is_static() || buffer.use_ >= i32::MAX as usize {
            unsafe {
                let ret_struct = &mut *ret;
                ret_struct.content = ptr::null_mut();
                ret_struct.content_io = ptr::null_mut();
                ret_struct.use_ = 0;
                ret_struct.size = 0;
            }
            return -1;
        }

        unsafe {
            let ret_struct = &mut *ret;
            ret_struct.use_ = buffer.use_ as u32;

            if buffer.size >= i32::MAX as usize {
                ret_struct.size = i32::MAX as u32;
            } else {
                ret_struct.size = buffer.size as u32 + 1;
            }

            ret_struct.alloc = 1; // XML_BUFFER_ALLOC_IO

            // Transfer ownership of content
            let mut content = buffer.content;
            content.shrink_to(buffer.use_ + 1);

            let ptr = content.as_mut_ptr();
            ret_struct.content = ptr.wrapping_add(buffer.content_offset);
            ret_struct.content_io = ptr;
            std::mem::forget(content); // Prevent deallocation

            0
        }
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn xmlBufResetInput(buf: XmlBufPtr, input: *mut XmlParserInput) -> c_int {
    log_buf!("xmlBufResetInput(buf={}, input={:p})", buf, input);
    xmlBufUpdateInput(buf, input, 0)
}

#[no_mangle]
pub extern "C" fn xmlBufUpdateInput(
    buf: XmlBufPtr,
    input: *mut XmlParserInput,
    pos: usize,
) -> c_int {
    log_buf!(
        "xmlBufUpdateInput(buf={}, input={:p}, pos={})",
        buf,
        input,
        pos
    );

    if buf == 0 || input.is_null() {
        log_buf!("xmlBufUpdateInput FAILED - invalid args");
        return -1;
    }

    let mutex = get_buffers();
    let m = mutex.lock().unwrap();
    if let Some(buffer) = m.get(&buf) {
        if buffer.is_error() {
            log_buf!("xmlBufUpdateInput FAILED - buffer in error state");
            return -1;
        }

        unsafe {
            let base_ptr = buffer.content_ptr();

            if base_ptr.is_null() {
                log_buf!("xmlBufUpdateInput FAILED - null content ptr");
                return -1;
            }

            (*input).base = base_ptr;
            (*input).cur = base_ptr.wrapping_add(pos);
            (*input).end = base_ptr.wrapping_add(buffer.use_);

            log_buf!(
                "xmlBufUpdateInput SUCCESS - base={:p}, cur={:p}, end={:p}, use={}",
                (*input).base,
                (*input).cur,
                (*input).end,
                buffer.use_
            );

            0
        }
    } else {
        log_buf!("xmlBufUpdateInput FAILED - handle {} not found", buf);
        -1
    }
}

#[allow(non_upper_case_globals)]
#[cfg(test)]
mod tests {
    use super::*;
    
    // For tests, provide xmlMalloc, xmlFree, and xmlRealloc as function pointers
    #[no_mangle]
    pub static xmlMalloc: XmlMallocFunc = libc::malloc;
    
    #[no_mangle]
    pub static xmlFree: XmlFreeFunc = libc::free;
    
    #[no_mangle]
    pub static xmlRealloc: XmlReallocFunc = libc::realloc;

    #[test]
    fn test_buf_create() {
        let buf = xmlBufCreate(100);
        assert_ne!(buf, 0);
        xmlBufFree(buf);
    }

    #[test]
    fn test_buf_add() {
        let buf = xmlBufCreate(100);
        assert_ne!(buf, 0);

        let test_str = b"Hello, World!\0";
        let result = xmlBufAdd(buf, test_str.as_ptr(), test_str.len() - 1);
        assert_eq!(result, 0);

        assert_eq!(xmlBufIsEmpty(buf), 0);

        xmlBufFree(buf);
    }

    #[test]
    fn test_buf_empty() {
        let buf = xmlBufCreate(100);
        assert_ne!(buf, 0);

        let test_str = b"Test\0";
        xmlBufAdd(buf, test_str.as_ptr(), test_str.len() - 1);
        assert_eq!(xmlBufIsEmpty(buf), 0);

        xmlBufEmpty(buf);
        assert_eq!(xmlBufIsEmpty(buf), 1);

        xmlBufFree(buf);
    }

    #[test]
    fn test_buf_grow() {
        let buf = xmlBufCreate(10);
        assert_ne!(buf, 0);

        let result = xmlBufGrow(buf, 100);
        assert_eq!(result, 0);

        assert!(xmlBufAvail(buf) >= 100);

        xmlBufFree(buf);
    }

    #[test]
    fn test_buf_cat() {
        let buf = xmlBufCreate(100);
        assert_ne!(buf, 0);

        let test_str = b"Hello\0";
        let result = xmlBufCat(buf, test_str.as_ptr());
        assert_eq!(result, 0);

        let test_str2 = b", World!\0";
        let result = xmlBufCat(buf, test_str2.as_ptr());
        assert_eq!(result, 0);

        xmlBufFree(buf);
    }

    #[test]
    fn test_buf_static() {
        let test_str = b"Static content\0";
        let buf = xmlBufCreateMem(test_str.as_ptr(), test_str.len() - 1, 1);
        assert_ne!(buf, 0);

        // Static buffers should not be modifiable
        let result = xmlBufAdd(buf, b"more\0".as_ptr(), 4);
        assert_eq!(result, -1);

        xmlBufFree(buf);
    }

    #[test]
    fn test_buf_detach() {
        let buf = xmlBufCreate(100);
        assert_ne!(buf, 0);

        let test_str = b"Detach me\0";
        xmlBufAdd(buf, test_str.as_ptr(), test_str.len() - 1);

        let detached = xmlBufDetach(buf);
        assert!(!detached.is_null());

        // Buffer should be empty after detach
        assert_eq!(xmlBufIsEmpty(buf), 1);

        // Free the detached content
        unsafe {
            xmlFree(detached as *mut c_void);
        }

        xmlBufFree(buf);
    }

    #[test]
    fn test_xml_buf_update_input() {
        let buf = xmlBufCreate(100);
        assert_ne!(buf, 0);

        let mut parser_input = XmlParserInput {
            buf: ptr::null_mut(),
            filename: ptr::null(),
            directory: ptr::null(),
            base: ptr::null(),
            cur: ptr::null(),
            end: ptr::null(),
            length: 0,
            line: 0,
            col: 0,
            consumed: 0,
            free: ptr::null_mut(),
            encoding: ptr::null(),
            version: ptr::null(),
            flags: 0,
            id: 0,
            parent_consumed: 0,
            entity: ptr::null_mut(),
        };

        let input = xmlBufResetInput(buf, &mut parser_input);
        assert_eq!(input, 0);
        assert!(!parser_input.base.is_null());
        assert!(!parser_input.cur.is_null());
        assert!(!parser_input.end.is_null());

        xmlBufUpdateInput(buf, &mut parser_input, 0);
        assert!(!parser_input.base.is_null());
        assert!(!parser_input.cur.is_null());
        assert!(!parser_input.end.is_null());

        xmlBufFree(buf);
    }
}
