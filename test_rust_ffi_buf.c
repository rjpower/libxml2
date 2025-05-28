/*
 * test_rust_ffi_buf.c: Test program for Rust FFI buffer implementation
 *
 * This program exercises the Rust implementation of the xmlBuf API
 * to ensure it maintains compatibility with the C interface.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

// Type definitions matching the Rust FFI
typedef unsigned char xmlChar;
typedef size_t xmlBufPtr;

// Function declarations - these will be provided by the Rust library
extern xmlBufPtr xmlBufCreate(size_t size);
extern xmlBufPtr xmlBufCreateMem(const xmlChar *mem, size_t size, int isStatic);
extern void xmlBufFree(xmlBufPtr buf);
extern void xmlBufEmpty(xmlBufPtr buf);
extern int xmlBufGrow(xmlBufPtr buf, size_t len);
extern int xmlBufAdd(xmlBufPtr buf, const xmlChar *str, size_t len);
extern int xmlBufCat(xmlBufPtr buf, const xmlChar *str);
extern size_t xmlBufAvail(xmlBufPtr buf);
extern int xmlBufIsEmpty(xmlBufPtr buf);
extern int xmlBufAddLen(xmlBufPtr buf, size_t len);
extern xmlChar *xmlBufDetach(xmlBufPtr buf);

// Test results tracking
static int tests_run = 0;
static int tests_passed = 0;

#define TEST_ASSERT(condition, message) do { \
    tests_run++; \
    if (condition) { \
        tests_passed++; \
        printf("PASS: %s\n", message); \
    } else { \
        printf("FAIL: %s\n", message); \
    } \
} while(0)

void test_buf_create_free() {
    printf("\n=== Testing xmlBufCreate/xmlBufFree ===\n");
    
    xmlBufPtr buf = xmlBufCreate(100);
    TEST_ASSERT(buf != 0, "xmlBufCreate should return non-zero handle");
    
    xmlBufFree(buf);
    // No crash = success
    TEST_ASSERT(1, "xmlBufFree should not crash");
    
    // Test with zero size
    buf = xmlBufCreate(0);
    TEST_ASSERT(buf != 0, "xmlBufCreate with size 0 should succeed");
    xmlBufFree(buf);
}

void test_buf_create_mem() {
    printf("\n=== Testing xmlBufCreateMem ===\n");
    
    const xmlChar *test_str = (const xmlChar *)"Hello, World!";
    size_t len = strlen((const char *)test_str);
    
    // Test non-static buffer
    xmlBufPtr buf = xmlBufCreateMem(test_str, len, 0);
    TEST_ASSERT(buf != 0, "xmlBufCreateMem non-static should succeed");
    xmlBufFree(buf);
    
    // Test static buffer (need null-terminated string)
    const xmlChar *static_str = (const xmlChar *)"Static content\0";
    buf = xmlBufCreateMem(static_str, 14, 1); // 14 = length without null terminator
    TEST_ASSERT(buf != 0, "xmlBufCreateMem static should succeed");
    xmlBufFree(buf);
    
    // Test with NULL memory
    buf = xmlBufCreateMem(NULL, 10, 0);
    TEST_ASSERT(buf == 0, "xmlBufCreateMem with NULL should fail");
}

void test_buf_add_cat() {
    printf("\n=== Testing xmlBufAdd/xmlBufCat ===\n");
    
    xmlBufPtr buf = xmlBufCreate(100);
    TEST_ASSERT(buf != 0, "Buffer creation should succeed");
    
    const xmlChar *test_str1 = (const xmlChar *)"Hello";
    int result = xmlBufAdd(buf, test_str1, 5);
    TEST_ASSERT(result == 0, "xmlBufAdd should succeed");
    
    const xmlChar *test_str2 = (const xmlChar *)", World!";
    result = xmlBufCat(buf, test_str2);
    TEST_ASSERT(result == 0, "xmlBufCat should succeed");
    
    int empty = xmlBufIsEmpty(buf);
    TEST_ASSERT(empty == 0, "Buffer should not be empty after adding content");
    
    xmlBufFree(buf);
}

void test_buf_empty() {
    printf("\n=== Testing xmlBufEmpty ===\n");
    
    xmlBufPtr buf = xmlBufCreate(100);
    
    const xmlChar *test_str = (const xmlChar *)"Test content";
    xmlBufAdd(buf, test_str, strlen((const char *)test_str));
    
    int empty = xmlBufIsEmpty(buf);
    TEST_ASSERT(empty == 0, "Buffer should not be empty after adding content");
    
    xmlBufEmpty(buf);
    empty = xmlBufIsEmpty(buf);
    TEST_ASSERT(empty == 1, "Buffer should be empty after xmlBufEmpty");
    
    xmlBufFree(buf);
}

void test_buf_grow() {
    printf("\n=== Testing xmlBufGrow ===\n");
    
    xmlBufPtr buf = xmlBufCreate(10);
    
    size_t avail_before = xmlBufAvail(buf);
    
    int result = xmlBufGrow(buf, 100);
    TEST_ASSERT(result == 0, "xmlBufGrow should succeed");
    
    size_t avail_after = xmlBufAvail(buf);
    TEST_ASSERT(avail_after >= 100, "Available space should increase after grow");
    TEST_ASSERT(avail_after > avail_before, "Available space should be larger than before");
    
    xmlBufFree(buf);
}

void test_buf_detach() {
    printf("\n=== Testing xmlBufDetach ===\n");
    
    xmlBufPtr buf = xmlBufCreate(100);
    
    const xmlChar *test_str = (const xmlChar *)"Detach this content";
    xmlBufAdd(buf, test_str, strlen((const char *)test_str));
    
    xmlChar *detached = xmlBufDetach(buf);
    TEST_ASSERT(detached != NULL, "xmlBufDetach should return non-NULL");
    
    if (detached) {
        TEST_ASSERT(strcmp((const char *)detached, (const char *)test_str) == 0, 
                   "Detached content should match original");
        free(detached); // Free the detached memory
    }
    
    int empty = xmlBufIsEmpty(buf);
    TEST_ASSERT(empty == 1, "Buffer should be empty after detach");
    
    xmlBufFree(buf);
}

void test_buf_add_len() {
    printf("\n=== Testing xmlBufAddLen ===\n");
    
    xmlBufPtr buf = xmlBufCreate(100);
    
    // Add some initial content
    const xmlChar *test_str = (const xmlChar *)"Hello";
    xmlBufAdd(buf, test_str, 5);
    
    // Use xmlBufAddLen to extend the used length (simulating direct writing)
    int result = xmlBufAddLen(buf, 3);
    TEST_ASSERT(result == 0, "xmlBufAddLen should succeed");
    
    xmlBufFree(buf);
}

void test_buf_static_restrictions() {
    printf("\n=== Testing static buffer restrictions ===\n");
    
    const xmlChar *static_str = (const xmlChar *)"Static content\0";
    xmlBufPtr buf = xmlBufCreateMem(static_str, 14, 1);
    TEST_ASSERT(buf != 0, "Static buffer creation should succeed");
    
    // Static buffers should not allow modifications
    const xmlChar *add_str = (const xmlChar *)"more";
    int result = xmlBufAdd(buf, add_str, 4);
    TEST_ASSERT(result == -1, "xmlBufAdd on static buffer should fail");
    
    result = xmlBufGrow(buf, 100);
    TEST_ASSERT(result == -1, "xmlBufGrow on static buffer should fail");
    
    xmlChar *detached = xmlBufDetach(buf);
    TEST_ASSERT(detached == NULL, "xmlBufDetach on static buffer should fail");
    
    xmlBufFree(buf);
}

void test_error_conditions() {
    printf("\n=== Testing error conditions ===\n");
    
    // Test operations on invalid handle
    int result = xmlBufAdd(0, (const xmlChar *)"test", 4);
    TEST_ASSERT(result == -1, "xmlBufAdd on invalid handle should fail");
    
    result = xmlBufGrow(0, 100);
    TEST_ASSERT(result == -1, "xmlBufGrow on invalid handle should fail");
    
    int empty = xmlBufIsEmpty(0);
    TEST_ASSERT(empty == -1, "xmlBufIsEmpty on invalid handle should return -1");
    
    size_t avail = xmlBufAvail(0);
    TEST_ASSERT(avail == 0, "xmlBufAvail on invalid handle should return 0");
    
    // Free invalid handle should not crash
    xmlBufFree(0);
    TEST_ASSERT(1, "xmlBufFree on invalid handle should not crash");
}

int main() {
    printf("Starting Rust FFI buffer tests...\n");
    
    test_buf_create_free();
    test_buf_create_mem();
    test_buf_add_cat();
    test_buf_empty();
    test_buf_grow();
    test_buf_detach();
    test_buf_add_len();
    test_buf_static_restrictions();
    test_error_conditions();
    
    printf("\n=== Test Summary ===\n");
    printf("Tests run: %d\n", tests_run);
    printf("Tests passed: %d\n", tests_passed);
    printf("Tests failed: %d\n", tests_run - tests_passed);
    
    if (tests_passed == tests_run) {
        printf("All tests PASSED!\n");
        return 0;
    } else {
        printf("Some tests FAILED!\n");
        return 1;
    }
}