# Porting C to Rust -- Guidelines.

When asked to port a C module to Rust, follow these guidelines.

A skeleton Rust module has been created for you in the `rust` directory and
wired into CMake. You will need to adjust it as you add new Rust files or
replace the C module.

# Document first

First analyze the module, and following the guidelines below, write a document
which outlines the port. This document should be named
`doc/port/<module_name>.md`. It should outline the overall layout of the module
in question, the approach you will take, challenges to consider and any other
relevant information. You should outline which of the testing approaches you
will take and your reason for skipping a test approach.

# Writing good CFFIs

* Unless otherwise specified, the C interface exposed from Rust must be identical to the original interface.
* The Rust implementation must, as closely as possible, be a one-to-one match to the C implementation.
  - Prefer to use similar function names and style - don't switch to idiomatic Rust without a clear reason.
* The Rust/C interface must be _safe_:
  - Don't return raw pointers to the C API, instead you must return a handle which is mapped to a Rust object.

Let's assume we have a C interface like:

```c
// foo.h
xmlFoo* xmlFooCreate(void);
void xmlFooFree(xmlFoo* foo);
void xmlFooPrint(xmlFoo* foo);
```

In Rust, we must expose the same CFFI:

```rust
// foo.rs
// Create a buffer
#[no_mangle]
pub extern "C" fn xmlFooCreate() -> xmlFooPtr
pub extern "C" fn xmlFooFree(foo: xmlFooPtr)
pub extern "C" fn xmlFooPrint(foo: xmlFooPtr)
```

But instead of returning a raw pointer, we must return a handle which is mapped to a Rust object. In this case we'll use a HashMap to map the C handles to Rust objects.

```rust
pub struct XmlFoo {}
pub type XmlFooPtr = usize;

static FOOS: OnceLock<Mutex<
    HashMap<XmlFooPtr, Box<XmlFoo>, BuildHasherDefault<DefaultHasher>>
>> = OnceLock::new();


#[no_mangle]
pub extern "C" fn xmlFooCreate() -> XmlFooPtr {
  let mutex = FOOS.get_or_init(|| {
      Mutex::new(HashMap::with_hasher(BuildHasherDefault::new()))
  });
  let mut m = mutex.lock().unwrap();
  let sz = m.len().try_into().unwrap();
  m.insert(sz, Box::new(XmlFoo {}));
  return sz;
}

#[no_mangle]
pub extern "C" fn xmlFooFree(foo: XmlFooPtr) {
  let mutex = FOOS.get_or_init(|| {
      Mutex::new(HashMap::with_hasher(BuildHasherDefault::new()))
  });
  let mut m = mutex.lock().unwrap();
  m.remove(&foo);
}

#[no_mangle]
pub extern "C" fn xmlFooPrint(foo: XmlFooPtr) {
  let mutex = FOOS.get_or_init(|| {
      Mutex::new(HashMap::with_hasher(BuildHasherDefault::new()))
  });
  let m = mutex.lock().unwrap();
  let foo = m.get(&foo).expect(&format!("foo not found at index {} was it freed?", foo));
  foo.print();
}
```

This works well for opaque C structures; if the C structure is not opaque, we
need a different strategy. If our API returns by value or fills a value, we can
of course simply fill the appropriate fields in our call:

```rust
#[repr(C)]
pub struct XmlBar {
  pub x: i32,
  pub y: i32,
  pub z: xmlFooPtr,
}

#[no_mangle]
pub extern "C" fn xmlBarCreate(x: i32, y: i32, z: xmlFooPtr) -> XmlBarPtr {
  XmlBar { x, y, z }
}

// or equivalently - this is unsafe, but it's the only way to create a struct with a pointer to a C object.
#[no_mangle]
pub extern "C" fn xmlBarCreate(bar: &mut XmlBar) -> XmlBarPtr {
  bar.z = xmlFooCreate();
  bar.x = 0;
  bar.y = 0;
  bar
}

```

If our API is not exposed externally, then we can change our API itself to be opaque, and for example switch to using accessor functions to access individual fields:

```rust
#[no_mangle]
pub extern "C" fn xmlBarGetX(bar: XmlBarPtr) -> i32 {
  let mutex = FOOS.get_or_init(|| {
      Mutex::new(HashMap::with_hasher(BuildHasherDefault::new()))
  });
  let m = mutex.lock().unwrap();
  let bar = m.get(&bar).expect(&format!("bar not found at index {} was it freed?", bar));
  bar.x
}
```

# Porting, Building, Testing, and Debugging.

When porting a module, follow these steps:

## Initial Port

Following the guidelines above, write your new Rust module which duplicates the API of the C module.
Your Rust module should have a source file name which matches the C module name.
You may define common helper modules for e.g. FFI, error handling, etc as well.

## Rust Testing

Always build Rust in debug mode with sanitizers enabled. We will perform release
testing after the port is complete.

### Unit tests

Write inline unit tests in your Rust module as part of your initial port. These
should exercise all functions in the module. You may now move on to fuzz testing.

### Fuzz testing

(Fuzz testing may not be relevant for modules which do not take variable input.
Document this in your port document.)

Write a `{filename}_fuzz.rs` file which uses `rust-fuzz` to fuzz your Rust
module. Define the appropriate tests which use the `fuzz_target!` macro to
define the fuzz target. You may use the `arbitrary` crate to help generate test
data.

### C/FFI testing

You may now proceed to write a C test module which exercises your individual
Rust module. This should be named `test_rust_ffi_{filename}.c` and should be a
minimal C program which exercises the CFFI interface of your Rust module.

### Integration tests

Once you have a working Rust module, you may now build the overall project.
Always build with sanitizers enabled and debug mode enabled e.g. 

cmake .. -DLIBXML2_WITH_TESTS=ON -DCMAKE_BUILD_TYPE=Debug -DCMAKE_C_FLAGS="-fsanitize=address -fno-omit-frame-pointer" -DCMAKE_EXE_LINKER_FLAGS="-fsanitize=address"

### Fixing bugs

When you encounter a bug at any step in this process, you must first write a new
document outlining your theory for the codebase and the bug. Write the document
into `doc/port/bugs/<timestamp_bug_name>.md`. Your document should include a paste of
the program output and expected output, followed by a description of your
understanding of how the codebase _should_ have worked and what you think went
wrong.
