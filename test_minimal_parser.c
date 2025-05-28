/*
 * test_minimal_parser.c: Minimal test to isolate parser segfault
 *
 * This test attempts to reproduce the parser segfault with the simplest
 * possible XML document to isolate whether the issue is in buffer
 * operations or parser integration.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <libxml/parser.h>
#include <libxml/tree.h>

int main() {
    printf("Testing minimal XML parsing with Rust buffer implementation\n");
    
    // Simplest possible XML document
    const char *simple_xml = "<?xml version=\"1.0\"?><root/>";
    
    printf("1. Testing xmlReadMemory with simple XML...\n");
    xmlDocPtr doc = xmlReadMemory(simple_xml, strlen(simple_xml), "test.xml", NULL, 0);
    
    if (doc) {
        printf("SUCCESS: Document parsed successfully\n");
        xmlFreeDoc(doc);
    } else {
        printf("FAILED: Document parsing failed\n");
        return 1;
    }
    
    printf("2. Testing even simpler XML...\n");
    const char *minimal_xml = "<a/>";
    doc = xmlReadMemory(minimal_xml, strlen(minimal_xml), "minimal.xml", NULL, 0);
    
    if (doc) {
        printf("SUCCESS: Minimal document parsed successfully\n");
        xmlFreeDoc(doc);
    } else {
        printf("FAILED: Minimal document parsing failed\n");
        return 1;
    }
    
    printf("3. Testing parser context creation...\n");
    xmlParserCtxtPtr ctxt = xmlCreateDocParserCtxt((const unsigned char *)simple_xml);
    
    if (ctxt) {
        printf("SUCCESS: Parser context created\n");
        
        printf("4. Checking parser input state...\n");
        if (ctxt->input) {
            printf("Parser input exists\n");
            if (ctxt->input->cur) {
                printf("Parser input cur pointer: %p\n", ctxt->input->cur);
                printf("First character: 0x%02x ('%c')\n", *ctxt->input->cur, *ctxt->input->cur);
            } else {
                printf("ERROR: Parser input cur pointer is NULL\n");
            }
            if (ctxt->input->base) {
                printf("Parser input base pointer: %p\n", ctxt->input->base);
            } else {
                printf("ERROR: Parser input base pointer is NULL\n");
            }
            if (ctxt->input->end) {
                printf("Parser input end pointer: %p\n", ctxt->input->end);
            } else {
                printf("ERROR: Parser input end pointer is NULL\n");
            }
        } else {
            printf("ERROR: Parser input is NULL\n");
        }
        
        printf("5. Testing xmlParseDocument directly...\n");
        int result = xmlParseDocument(ctxt);
        
        if (result == 0) {
            printf("SUCCESS: xmlParseDocument completed\n");
        } else {
            printf("FAILED: xmlParseDocument returned %d\n", result);
        }
        
        xmlFreeParserCtxt(ctxt);
    } else {
        printf("FAILED: Parser context creation failed\n");
        return 1;
    }
    
    printf("All tests completed\n");
    return 0;
}