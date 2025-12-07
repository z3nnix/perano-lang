# Perano Language Specification

## Overview
Perano is a statically-typed systems programming language with Rust-like syntax, designed for low-level programming and operating system development.

## Basic Syntax

### Program Structure
```perano
package main

import "stdio"
import "math"

fn main() {
    // Program entry point
    return 0
}
```

### Comments
```perano
// Single-line comment
```

## Data Types

### Primitive Types
- `i64` - 64-bit signed integer
- `string` - String literal

### Type Annotations
```perano
var x: i64 = 42
var name: string = "Hello"
```

## Variables

### Declaration
```perano
var x: i64 = 10
var y: i64 = 20
```

### Assignment
```perano
x = 42
```

## Arrays

### Declaration
```perano
var arr: [i64; 10]
```

### Access
```perano
arr[0] = 100
var value: i64 = arr[0]
```

## Pointers

### Address-of Operator
```perano
var x: i64 = 42
var ptr: i64 = &x
```

### Dereference Operator
```perano
var value: i64 = *ptr
*ptr = 100
```

## Control Flow

### If Statement
```perano
if x > 10 {
    stdio.Println(x)
} else {
    stdio.Println(0)
}
```

### For Loop
```perano
for var i: i64 = 0; i < 10; i = i + 1 {
    stdio.Println(i)
}
```

## Functions

### Function Definition
```perano
fn add(a: i64, b: i64) -> i64 {
    return a + b
}
```

### Function Call
```perano
var result: i64 = add(10, 20)
```

## Operators

### Arithmetic
- `+` Addition
- `-` Subtraction
- `*` Multiplication
- `/` Division
- `%` Modulo

### Comparison
- `==` Equal
- `!=` Not equal
- `<` Less than
- `<=` Less than or equal
- `>` Greater than
- `>=` Greater than or equal

### Logical
- `&&` Logical AND
- `||` Logical OR
- `!` Logical NOT

### Unary
- `-` Negation
- `&` Address-of
- `*` Dereference

## Modules

### Import
```perano
import "stdio"
import "math"
```

### Module Functions
```perano
stdio.Print(42)
stdio.Println(100)
stdio.PrintStr("Hello")
stdio.PrintlnStr("World")
```

## Standard Library

### stdio Module
- `Print(i64)` - Print integer
- `Println(i64)` - Print integer with newline
- `PrintStr(string)` - Print string
- `PrintlnStr(string)` - Print string with newline

### math Module
- Mathematical operations (implementation-defined)

### string Module
- String operations (implementation-defined)

## Compilation Targets

Perano supports three compilation targets:

### PE (Windows)
```bash
perano-lang program.per
```

### ELF (Linux)
```bash
perano-lang program.per --elf
```

### NovariaOS application
```bash
perano-lang program.per --novaria
```

### Novaria Virtual Machine bytecode
```bash
perano-lang program.per --nvm-code
```

## Example Program

```perano
package main

import "stdio"

fn swap(a: i64, b: i64) {
    var temp: i64 = *a
    *a = *b
    *b = temp
}

fn main() {
    var x: i64 = 10
    var y: i64 = 20
    
    stdio.Println(x)
    stdio.Println(y)
    
    swap(&x, &y)
    
    stdio.Println(x)
    stdio.Println(y)
    
    return 0
}
```

## Language Features

### Supported
- Variables and type annotations
- Integers (i64)
- Strings
- Arrays
- Pointers (address-of and dereference)
- Functions with parameters and return values
- If/else statements
- For loops
- Module system
- Standard library

### Limitations (currently)
- No structures/records
- No enums
- No floating-point numbers
- No generics
- No closures
- Limited string operations