#![no_main]

use libfuzzer_sys::fuzz_target;

use xml2buf::*;
use arbitrary::Arbitrary;
use std::collections::HashMap;

// Provide memory management functions for fuzzing
#[no_mangle]
pub static xmlMalloc: unsafe extern "C" fn(usize) -> *mut libc::c_void = libc::malloc;

#[no_mangle]
pub static xmlFree: unsafe extern "C" fn(*mut libc::c_void) = libc::free;

#[no_mangle]
pub static xmlRealloc: unsafe extern "C" fn(*mut libc::c_void, usize) -> *mut libc::c_void = libc::realloc;

#[derive(Debug, Clone, Arbitrary)]
enum BufferType {
    XmlBuf,
    XmlBuffer,
}

#[derive(Debug, Clone, Arbitrary)]
enum BufOperation {
    // Creation operations
    CreateBuf { size: u16 },
    CreateBufMem { data: Vec<u8>, is_static: bool },
    CreateBuffer,
    CreateBufferSize { size: u16 },
    CreateBufferStatic { data: Vec<u8> },
    
    // Destruction operations
    FreeBuf { id: u8 },
    FreeBuffer { id: u8 },
    
    // XmlBuf operations
    BufAdd { id: u8, data: Vec<u8> },
    BufCat { id: u8, data: String },
    BufGrow { id: u8, len: u16 },
    BufEmpty { id: u8 },
    BufAddLen { id: u8, len: u8 },
    BufDetach { id: u8 },
    BufShrink { id: u8, len: u16 },
    BufIsEmpty { id: u8 },
    BufAvail { id: u8 },
    BufUse { id: u8 },
    BufContent { id: u8 },
    BufEnd { id: u8 },
    
    // XmlBuffer operations
    BufferAdd { id: u8, data: Vec<u8> },
    BufferAddHead { id: u8, data: Vec<u8> },
    BufferCat { id: u8, data: String },
    BufferCCat { id: u8, data: String },
    BufferWriteCHAR { id: u8, data: String },
    BufferWriteChar { id: u8, data: String },
    BufferWriteQuotedString { id: u8, data: String },
    BufferGrow { id: u8, len: u16 },
    BufferShrink { id: u8, len: u16 },
    BufferResize { id: u8, size: u16 },
    BufferEmpty { id: u8 },
    BufferDetach { id: u8 },
    BufferLength { id: u8 },
    BufferContent { id: u8 },
}

#[derive(Debug, Clone, Arbitrary)]
pub struct FuzzInput {
    operations: Vec<BufOperation>,
}

struct FuzzState {
    xml_bufs: HashMap<u8, XmlBufPtr>,
    xml_buffers: HashMap<u8, *mut XmlBuffer>,
}

impl FuzzState {
    fn new() -> Self {
        Self {
            xml_bufs: HashMap::new(),
            xml_buffers: HashMap::new(),
        }
    }
    
    fn get_buf(&self, id: u8) -> Option<XmlBufPtr> {
        self.xml_bufs.get(&id).copied()
    }
    
    fn get_buffer(&self, id: u8) -> Option<*mut XmlBuffer> {
        self.xml_buffers.get(&id).copied()
    }
    
    fn store_buf(&mut self, id: u8, buf: XmlBufPtr) {
        if buf != 0 {
            self.xml_bufs.insert(id, buf);
        }
    }
    
    fn store_buffer(&mut self, id: u8, buffer: *mut XmlBuffer) {
        if !buffer.is_null() {
            self.xml_buffers.insert(id, buffer);
        }
    }
    
    fn remove_buf(&mut self, id: u8) -> Option<XmlBufPtr> {
        self.xml_bufs.remove(&id)
    }
    
    fn remove_buffer(&mut self, id: u8) -> Option<*mut XmlBuffer> {
        self.xml_buffers.remove(&id)
    }
}

pub fn fuzz_buffer_operations(input: FuzzInput) -> Result<(), arbitrary::Error> {
    let mut state = FuzzState::new();
    let mut next_id: u8 = 0;
    
    for (_op_idx, op) in input.operations.iter().enumerate() {
        
        match op {
            // Creation operations
            BufOperation::CreateBuf { size } => {
                let buf = xmlBufCreate(*size as usize);
                state.store_buf(next_id, buf);
                next_id = next_id.wrapping_add(1);
            }
            
            BufOperation::CreateBufMem { data, is_static } => {
                if !data.is_empty() {
                    let mut data = data.clone();
                    // Ensure null termination for static buffers
                    if *is_static && data.last() != Some(&0) {
                        data.push(0);
                    }
                    
                    let buf = xmlBufCreateMem(
                        data.as_ptr(), 
                        data.len().saturating_sub(if *is_static { 1 } else { 0 }),
                        if *is_static { 1 } else { 0 }
                    );
                    state.store_buf(next_id, buf);
                    next_id = next_id.wrapping_add(1);
                }
            }
            
            BufOperation::CreateBuffer => {
                let buffer = xmlBufferCreate();
                state.store_buffer(next_id, buffer);
                next_id = next_id.wrapping_add(1);
            }
            
            BufOperation::CreateBufferSize { size } => {
                let buffer = xmlBufferCreateSize(*size as usize);
                state.store_buffer(next_id, buffer);
                next_id = next_id.wrapping_add(1);
            }
            
            BufOperation::CreateBufferStatic { data } => {
                if !data.is_empty() {
                    let mut data = data.clone();
                    // Ensure null termination
                    if data.last() != Some(&0) {
                        data.push(0);
                    }
                    let buffer = xmlBufferCreateStatic(
                        data.as_ptr() as *mut libc::c_void,
                        data.len().saturating_sub(1)
                    );
                    state.store_buffer(next_id, buffer);
                    next_id = next_id.wrapping_add(1);
                }
            }
            
            // Destruction operations - these remove from tracking to prevent UAF
            BufOperation::FreeBuf { id } => {
                if let Some(buf) = state.remove_buf(*id) {
                    xmlBufFree(buf);
                }
            }
            
            BufOperation::FreeBuffer { id } => {
                if let Some(buffer) = state.remove_buffer(*id) {
                    xmlBufferFree(buffer);
                }
            }
            
            // XmlBuf operations
            BufOperation::BufAdd { id, data } => {
                if let Some(buf) = state.get_buf(*id) {
                    if !data.is_empty() {
                        xmlBufAdd(buf, data.as_ptr(), data.len());
                    }
                }
            }
            
            BufOperation::BufCat { id, data } => {
                if let Some(buf) = state.get_buf(*id) {
                    let mut null_terminated = data.as_bytes().to_vec();
                    null_terminated.push(0);
                    xmlBufCat(buf, null_terminated.as_ptr());
                }
            }
            
            BufOperation::BufGrow { id, len } => {
                if let Some(buf) = state.get_buf(*id) {
                    xmlBufGrow(buf, *len as usize);
                }
            }
            
            BufOperation::BufEmpty { id } => {
                if let Some(buf) = state.get_buf(*id) {
                    xmlBufEmpty(buf);
                }
            }
            
            BufOperation::BufAddLen { id, len } => {
                if let Some(buf) = state.get_buf(*id) {
                    xmlBufAddLen(buf, *len as usize);
                }
            }
            
            BufOperation::BufDetach { id } => {
                if let Some(buf) = state.remove_buf(*id) {
                    let ptr = xmlBufDetach(buf);
                    if !ptr.is_null() {
                        unsafe {
                            libc::free(ptr as *mut libc::c_void);
                        }
                    }
                }
            }
            
            BufOperation::BufShrink { id, len } => {
                if let Some(buf) = state.get_buf(*id) {
                    xmlBufShrink(buf, *len as usize);
                }
            }
            
            BufOperation::BufIsEmpty { id } => {
                if let Some(buf) = state.get_buf(*id) {
                    xmlBufIsEmpty(buf);
                }
            }
            
            BufOperation::BufAvail { id } => {
                if let Some(buf) = state.get_buf(*id) {
                    xmlBufAvail(buf);
                }
            }
            
            BufOperation::BufUse { id } => {
                if let Some(buf) = state.get_buf(*id) {
                    xmlBufUse(buf);
                }
            }
            
            BufOperation::BufContent { id } => {
                if let Some(buf) = state.get_buf(*id) {
                    xmlBufContent(buf);
                }
            }
            
            BufOperation::BufEnd { id } => {
                if let Some(buf) = state.get_buf(*id) {
                    xmlBufEnd(buf);
                }
            }
            
            // XmlBuffer operations
            BufOperation::BufferAdd { id, data } => {
                if let Some(buffer) = state.get_buffer(*id) {
                    if !data.is_empty() {
                        xmlBufferAdd(buffer, data.as_ptr(), data.len() as libc::c_int);
                    }
                }
            }
            
            BufOperation::BufferAddHead { id, data } => {
                if let Some(buffer) = state.get_buffer(*id) {
                    if !data.is_empty() {
                        xmlBufferAddHead(buffer, data.as_ptr(), data.len() as libc::c_int);
                    }
                }
            }
            
            BufOperation::BufferCat { id, data } => {
                if let Some(buffer) = state.get_buffer(*id) {
                    let mut null_terminated = data.as_bytes().to_vec();
                    null_terminated.push(0);
                    xmlBufferCat(buffer, null_terminated.as_ptr());
                }
            }
            
            BufOperation::BufferCCat { id, data } => {
                if let Some(buffer) = state.get_buffer(*id) {
                    let mut null_terminated = data.as_bytes().to_vec();
                    null_terminated.push(0);
                    xmlBufferCCat(buffer, null_terminated.as_ptr() as *const i8);
                }
            }
            
            BufOperation::BufferWriteCHAR { id, data } => {
                if let Some(buffer) = state.get_buffer(*id) {
                    let mut null_terminated = data.as_bytes().to_vec();
                    null_terminated.push(0);
                    xmlBufferWriteCHAR(buffer, null_terminated.as_ptr());
                }
            }
            
            BufOperation::BufferWriteChar { id, data } => {
                if let Some(buffer) = state.get_buffer(*id) {
                    let mut null_terminated = data.as_bytes().to_vec();
                    null_terminated.push(0);
                    xmlBufferWriteChar(buffer, null_terminated.as_ptr() as *const i8);
                }
            }
            
            BufOperation::BufferWriteQuotedString { id, data } => {
                if let Some(buffer) = state.get_buffer(*id) {
                    let mut null_terminated = data.as_bytes().to_vec();
                    null_terminated.push(0);
                    xmlBufferWriteQuotedString(buffer, null_terminated.as_ptr());
                }
            }
            
            BufOperation::BufferGrow { id, len } => {
                if let Some(buffer) = state.get_buffer(*id) {
                    xmlBufferGrow(buffer, *len as libc::c_int);
                }
            }
            
            BufOperation::BufferShrink { id, len } => {
                if let Some(buffer) = state.get_buffer(*id) {
                    xmlBufferShrink(buffer, *len as u32);
                }
            }
            
            BufOperation::BufferResize { id, size } => {
                if let Some(buffer) = state.get_buffer(*id) {
                    xmlBufferResize(buffer, *size as u32);
                }
            }
            
            BufOperation::BufferEmpty { id } => {
                if let Some(buffer) = state.get_buffer(*id) {
                    xmlBufferEmpty(buffer);
                }
            }
            
            BufOperation::BufferDetach { id } => {
                if let Some(buffer) = state.remove_buffer(*id) {
                    let ptr = xmlBufferDetach(buffer);
                    if !ptr.is_null() {
                        unsafe {
                            libc::free(ptr as *mut libc::c_void);
                        }
                    }
                }
            }
            
            BufOperation::BufferLength { id } => {
                if let Some(buffer) = state.get_buffer(*id) {
                    xmlBufferLength(buffer);
                }
            }
            
            BufOperation::BufferContent { id } => {
                if let Some(buffer) = state.get_buffer(*id) {
                    xmlBufferContent(buffer);
                }
            }
        }
    }
    
    
    // Clean up any remaining buffers
    let bufs_to_free: Vec<(u8, XmlBufPtr)> = state.xml_bufs.drain().collect();
    for (_id, buf) in bufs_to_free {
        xmlBufFree(buf);
    }
    let buffers_to_free: Vec<(u8, *mut XmlBuffer)> = state.xml_buffers.drain().collect();
    for (_id, buffer) in buffers_to_free {
        xmlBufferFree(buffer);
    }
    
    Ok(())
}

fuzz_target!(|data: FuzzInput| {
    let _ = fuzz_buffer_operations(data);
});