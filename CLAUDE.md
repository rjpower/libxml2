# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

libxml2 supports three build systems: Autotools, CMake, and Meson.

### Autotools (recommended for Unix-like systems)
```bash
# From git repository
./autogen.sh
./configure
make
make check  # Run tests
make install

# Common configure options:
./configure --with-python  # Enable Python bindings
./configure --with-zlib    # Enable zlib support
./configure --with-lzma    # Enable lzma support
./configure --with-readline --with-history  # Enable readline for xmllint shell
```

### CMake (mainly for Windows)
```bash
cmake -S . -B build
cmake --build build
ctest --test-dir build  # Run tests
cmake --install build
```

### Meson
```bash
meson setup build
ninja -C build
meson test -C build  # Run tests
ninja -C build install
```

## Testing

### Running all tests
- Autotools: `make check`
- CMake: `ctest --test-dir build`
- Meson: `meson test -C build`

### Individual test programs
- `./runtest` - Main regression test suite
- `./testapi` - API tests
- `./testchar` - Character handling tests
- `./testdict` - Dictionary tests
- `./testparser` - Parser tests
- `./testrecurse` - Recursion tests
- `./testlimits` - Limits tests
- `./runxmlconf -d xmlconf` - XML conformance tests
- `./runsuite` - Test suite runner

### Running a single test
```bash
# Run with specific XML file
./xmllint test/valid/dtd1.xml
```

### Valgrind testing
```bash
make check-valgrind
# or
make CHECKER='valgrind -q' check
```

## Code Architecture

libxml2 is a C library for parsing and manipulating XML documents. The main components are:

### Core Modules
- **parser.c/parserInternals.c**: XML parser implementation
- **tree.c**: DOM tree structure and manipulation
- **encoding.c**: Character encoding support
- **xmlIO.c**: Input/output handling
- **xmlmemory.c**: Memory management
- **error.c**: Error handling
- **valid.c**: DTD validation
- **xpath.c**: XPath 1.0 implementation

### Additional Features
- **HTMLparser.c/HTMLtree.c**: HTML parser
- **xmlschemas.c/xmlschemastypes.c**: XML Schema validation
- **relaxng.c**: RELAX NG validation
- **schematron.c**: Schematron validation
- **xinclude.c**: XInclude support
- **c14n.c**: Canonicalization
- **xmlreader.c**: Streaming API
- **xmlwriter.c**: Document generation API
- **catalog.c**: XML Catalog support

### Build Configuration
The library uses conditional compilation based on feature flags:
- `WITH_*_SOURCES` flags in Autotools control which modules are built
- `LIBXML2_WITH_*` options in CMake
- Similar options in meson_options.txt

### Thread Safety
The library maintains global state in `globals.c`. Thread-local storage can be enabled with appropriate build options (`--with-threads` and `--with-thread-alloc` for Autotools).

### Testing Infrastructure
- Test XML files are in `test/` directory
- Expected results are in `result/` directory
- Test scripts are in `test/scripts/`
- Regression tests compare output against expected results

## Important Notes

- This is a security-critical library - be careful with untrusted input
- The code must conform to C89 standard
- All changes should include regression tests when possible
- Memory safety is crucial - use the library's memory management functions
- The library maintains strict ABI compatibility