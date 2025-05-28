#![cfg(feature = "fuzz")]
use crate::buf::*;
use arbitrary::{Arbitrary, Unstructured};

#[cfg(feature = "fuzz")]
#[derive(Debug, Clone, Arbitrary)]
enum BufOperation {
    Create { size: u16 },
    CreateMem { data: Vec<u8>, is_static: bool },
    Add { data: Vec<u8> },
    Cat { data: String },
    Grow { len: u16 },
    Empty,
    AddLen { len: u8 },
    Detach,
    Free,
}

#[cfg(feature = "fuzz")]
#[derive(Debug, Clone, Arbitrary)]
struct FuzzInput {
    operations: Vec<BufOperation>,
}

#[cfg(feature = "fuzz")]
pub fn fuzz_buffer_operations(data: &[u8]) -> Result<(), arbitrary::Error> {
    let mut u = Unstructured::new(data);
    let input: FuzzInput = FuzzInput::arbitrary(&mut u)?;
    
    let mut active_buffers: Vec<XmlBufPtr> = Vec::new();
    
    for op in input.operations {
        match op {
            BufOperation::Create { size } => {
                let buf = xmlBufCreate(size as usize);
                if buf != 0 {
                    active_buffers.push(buf);
                }
            }
            
            BufOperation::CreateMem { mut data, is_static } => {
                if !data.is_empty() {
                    // Ensure null termination for static buffers
                    if is_static && data.last() != Some(&0) {
                        data.push(0);
                    }
                    
                    let buf = xmlBufCreateMem(
                        data.as_ptr(), 
                        data.len().saturating_sub(if is_static { 1 } else { 0 }),
                        if is_static { 1 } else { 0 }
                    );
                    if buf != 0 {
                        active_buffers.push(buf);
                    }
                }
            }
            
            BufOperation::Add { data } => {
                if let Some(&buf) = active_buffers.last() {
                    if !data.is_empty() {
                        xmlBufAdd(buf, data.as_ptr(), data.len());
                    }
                }
            }
            
            BufOperation::Cat { data } => {
                if let Some(&buf) = active_buffers.last() {
                    let mut null_terminated = data.into_bytes();
                    null_terminated.push(0);
                    xmlBufCat(buf, null_terminated.as_ptr());
                }
            }
            
            BufOperation::Grow { len } => {
                if let Some(&buf) = active_buffers.last() {
                    xmlBufGrow(buf, len as usize);
                }
            }
            
            BufOperation::Empty => {
                if let Some(&buf) = active_buffers.last() {
                    xmlBufEmpty(buf);
                }
            }
            
            BufOperation::AddLen { len } => {
                if let Some(&buf) = active_buffers.last() {
                    xmlBufAddLen(buf, len as usize);
                }
            }
            
            BufOperation::Detach => {
                if let Some(&buf) = active_buffers.last() {
                    let ptr = xmlBufDetach(buf);
                    if !ptr.is_null() {
                        unsafe {
                            libc::free(ptr as *mut libc::c_void);
                        }
                    }
                }
            }
            
            BufOperation::Free => {
                if let Some(buf) = active_buffers.pop() {
                    xmlBufFree(buf);
                }
            }
        }
        
        // Perform some consistency checks on active buffers
        for &buf in &active_buffers {
            xmlBufIsEmpty(buf);
            xmlBufAvail(buf);
        }
    }
    
    // Clean up any remaining buffers
    for buf in active_buffers {
        xmlBufFree(buf);
    }
    
    Ok(())
}

#[cfg(feature = "fuzz")]
use libfuzzer_sys::fuzz_target;

#[cfg(feature = "fuzz")]
fuzz_target!(|data: &[u8]| {
    let _ = fuzz_buffer_operations(data);
});

#[cfg(feature = "fuzz")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzz_with_empty_input() {
        let result = fuzz_buffer_operations(&[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_fuzz_with_minimal_input() {
      
        let result = fuzz_buffer_operations(&[0, 1, 2, 3]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_fuzz_buffer_growth() {
        let result = fuzz_buffer_operations(&[4, 5, 6, 7, 8, 9]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_fuzz_static_buffer() {
        let result = fuzz_buffer_operations(&[10, 11, 12, 13, 14, 15, 16]);
        assert!(result.is_ok());
    }
}