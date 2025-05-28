# Bug Report: Heap Buffer Overflow in xmlMemFree After xmlBufDetach

## Date: 2025-05-28

## Summary
AddressSanitizer detects a heap-buffer-overflow when xmlMemFree attempts to free memory returned by xmlBufDetach. The crash occurs when running the runtest suite.

## Program Output
```
==10281==ERROR: AddressSanitizer: heap-buffer-overflow on address 0x6060000020b0 at pc 0x000103569170 bp 0x00016d5d50f0 sp 0x00016d5d50e8
READ of size 4 at 0x6060000020b0 thread T0
    #0 0x00010356916c in xmlMemFree xmlmemory.c:198
    #1 0x0001034ed3c0 in xmlFreeNodeList tree.c:3360
    ...
    #5 0x00010380dbc0 in xml2buf::buf::XmlBuf::detach::h7af947ded4f0c98f buf.rs:314
    #6 0x00010380f784 in xmlBufDetach buf.rs:578
    #7 0x0001034eecb8 in xmlNodeParseContentInternal tree.c:1183
```

## Analysis

### Understanding How libxml2 Memory Management Works

libxml2 uses a custom memory allocator that adds a header (MEMHDR) before each allocated block:

```c
typedef struct memnod {
    unsigned int   mh_tag;      // Magic tag (0x5aa5)
    size_t         mh_size;     // Size of allocation
} MEMHDR;

#define RESERVE_SIZE (((sizeof(MEMHDR) + ALIGN_SIZE - 1) / ALIGN_SIZE ) * ALIGN_SIZE)
#define CLIENT_2_HDR(a) ((void *) (((char *) (a)) - RESERVE_SIZE))
#define HDR_2_CLIENT(a) ((void *) (((char *) (a)) + RESERVE_SIZE))
```

When xmlMemMalloc allocates memory, it:
1. Allocates `RESERVE_SIZE + requested_size`
2. Writes the MEMHDR at the beginning
3. Returns a pointer offset by RESERVE_SIZE (the "client" pointer)

When xmlMemFree frees memory, it:
1. Takes the client pointer and subtracts RESERVE_SIZE to find the header
2. Reads the header to verify the magic tag
3. Frees the original allocation

### The Problem

Our Rust implementation of xmlBufDetach is returning a pointer allocated by Rust's allocator (via Vec<u8>), not by xmlMemMalloc. When this pointer is later freed with xmlMemFree:

1. xmlMemFree tries to read the MEMHDR at `ptr - RESERVE_SIZE`
2. This location is outside the bounds of our Rust allocation
3. AddressSanitizer detects the out-of-bounds read

The stack trace shows:
- xmlBufDetach (Rust) returns a pointer from `content.shrink_to(self.use_ + 1)`
- This pointer is eventually passed to xmlMemFree
- xmlMemFree reads at offset -16 (or similar) causing the overflow

### Expected Behavior

xmlBufDetach should return memory that can be safely freed with xmlMemFree. This means:
1. The memory must have been allocated with xmlMemMalloc
2. The returned pointer must point to the client area (after the header)
3. The header must contain valid metadata

### Root Cause

The Rust implementation incorrectly assumes it can return a pointer from Rust-allocated memory (Vec<u8>) that will be freed by C code using xmlMemFree. This violates the memory allocation contract - memory must be allocated and freed by the same allocator.

## Solution

After examining the original C implementation, the fix is clear:

1. When content is offset (content != mem), the C version uses xmlStrndup which allocates with xmlMalloc
2. When content is not offset, it returns the raw pointer which was originally allocated with xmlMalloc
3. Our Rust version must use xmlMalloc for allocations that will be freed by C code

The key insight is that ALL memory that crosses the FFI boundary and will be freed by C code must be allocated with xmlMalloc, not Rust's allocator.

## Next Steps

1. Import xmlMalloc and xmlFree functions in the Rust FFI
2. Modify xmlBufDetach to use xmlMalloc when creating the detached buffer
3. Ensure all buffer memory that might be detached is allocated with xmlMalloc from the start