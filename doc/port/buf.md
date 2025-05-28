# Porting buf.c/buf.h to Rust

## Module Analysis

The buf module (`buf.c` and `include/private/buf.h`) provides memory buffer management functionality for libxml2. This is a core utility module that handles dynamic memory allocation and manipulation for XML content.

### Structure

The module defines the `xmlBuf` structure (opaque to external users) with the following internal fields:
- `content`: Pointer to the current content position in the buffer
- `mem`: Pointer to the start of allocated memory
- `use`: Current number of bytes used in the buffer
- `size`: Total allocated size (excluding null terminator)
- `maxSize`: Maximum allowed buffer size
- `flags`: Status flags (OOM, overflow, static)

The module also provides legacy `xmlBuffer` API compatibility for older code.

### Public API Functions (from buf.h)

The private header exposes these functions that need Rust implementations:

1. **Buffer lifecycle:**
   - `xmlBufCreate(size_t size)` - Create new buffer with initial size
   - `xmlBufCreateMem(const xmlChar *mem, size_t size, int isStatic)` - Create buffer from existing memory
   - `xmlBufFree(xmlBuf *buf)` - Free buffer

2. **Buffer manipulation:**
   - `xmlBufEmpty(xmlBuf *buf)` - Empty buffer content
   - `xmlBufGrow(xmlBuf *buf, size_t len)` - Grow buffer capacity
   - `xmlBufAdd(xmlBuf *buf, const xmlChar *str, size_t len)` - Add data to buffer
   - `xmlBufCat(xmlBuf *buf, const xmlChar *str)` - Concatenate null-terminated string

3. **Buffer inspection:**
   - `xmlBufAvail(xmlBuf *buf)` - Get available space
   - `xmlBufIsEmpty(xmlBuf *buf)` - Check if buffer is empty
   - `xmlBufAddLen(xmlBuf *buf, size_t len)` - Increase used length

4. **Buffer extraction:**
   - `xmlBufDetach(xmlBuf *buf)` - Extract content and clear buffer

5. **Legacy compatibility:**
   - `xmlBufFromBuffer(xmlBuffer *buffer)` - Convert old buffer to new
   - `xmlBufBackToBuffer(xmlBuf *buf, xmlBuffer *ret)` - Convert back to old buffer

6. **Parser integration:**
   - `xmlBufResetInput(xmlBuf *buf, xmlParserInput *input)` - Reset parser input pointers
   - `xmlBufUpdateInput(xmlBuf *buf, xmlParserInput *input, size_t pos)` - Update parser input pointers

## Porting Approach

### Handle Management Strategy

Since `xmlBuf` is an opaque structure used extensively throughout libxml2, we'll use the handle-based approach recommended in the porting guidelines:

1. **Rust Structure:** Define `XmlBuf` struct containing the buffer state
2. **Handle Type:** Use `xmlBufPtr = usize` as the handle type
3. **Global Storage:** Use `HashMap<usize, Box<XmlBuf>>` with mutex protection
4. **FFI Safety:** All C interface functions will validate handles and return error codes

### Memory Management

The Rust implementation will:
- Use `Vec<u8>` for the underlying buffer storage
- Handle growth strategies similar to the C implementation (doubling size with limits)
- Implement static buffer support by marking buffers as read-only
- Provide proper error handling for OOM conditions

### Error Handling

Following C conventions:
- Return `NULL`/`0` for creation failures
- Return `-1` for operation failures
- Return `0` for success
- Set internal error flags for OOM/overflow conditions

### Testing Approach

1. **Unit Tests:** Test all buffer operations individually
2. **Fuzz Testing:** Generate random sequences of buffer operations to test edge cases
3. **FFI Tests:** C test program exercising the FFI interface
4. **Integration Tests:** Test with existing libxml2 parser code

**Fuzz testing rationale:** The buffer module takes variable-length input data and performs memory operations, making it an excellent candidate for fuzz testing to discover buffer overflows, integer overflows, and other memory safety issues.

## Implementation Challenges

1. **Handle Validation:** Need robust handle validation to prevent use-after-free
2. **Thread Safety:** The global HashMap requires proper synchronization
3. **Memory Layout Compatibility:** Parser input pointers must remain valid after buffer operations
4. **Legacy Buffer Conversion:** Complex conversion logic between old and new buffer formats
5. **Static Buffer Handling:** Read-only buffers with different memory management

## Dependencies

- Standard Rust collections (`HashMap`, `Vec`)
- Synchronization primitives (`Mutex`, `OnceLock`)
- C FFI types for `xmlChar`, `xmlBuffer`, `xmlParserInput`
- libxml2 memory allocation functions (`xmlMalloc`, `xmlFree`)

## Files to Create

1. `rust/src/buf.rs` - Main Rust implementation
2. `rust/src/buf_fuzz.rs` - Fuzz testing (if applicable)
3. `test_rust_ffi_buf.c` - C FFI test program

## Integration Points

The buf module interfaces with:
- Parser input handling (`xmlParserInput`)
- Legacy buffer structures (`xmlBuffer`)
- Memory management (`xmlMalloc`/`xmlFree`)
- Error reporting systems

This module is fundamental to libxml2's memory management and must maintain exact behavioral compatibility with the C implementation.