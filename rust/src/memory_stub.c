/*
 * Temporary stub for xmlMalloc/xmlFree to break circular dependency
 * This will be replaced when we properly integrate memory management
 */

#include <stdlib.h>

void* xmlMalloc(size_t size) {
    return malloc(size);
}

void xmlFree(void* ptr) {
    free(ptr);
}