# Program Auditor

Performs an audit for a Nada Program. It checks that the program complies with policies:

- Passes MIR Validation
- Does not exceed a maximum number of operations
- Does not exceed a maximum memory size
- Does not require more pre-processing elements than necessary

In order to achieve this, it makes use of already existing tools:

- MIR Validator - Validate program syntax
- JIT Compiler - Calculate number of operations and memory size
- Program Analyzer - Count pre-processing elements

# Maximum instruction policy

It is possible to set policies for maximum amount of instructions for any given instruction. Only those instructions defined in the configuration
will be given a limit. That is, no limit is applied for instructions that are not listed in the configuration. If the configuration element is empty
(in YAML `{}`), no maximum instructions limit is defined.

In order to add an instruction limit to the configuration
an entry must be added to `max_instructions` section with the name of the instruction (see [`Protocol`](../../libs/execution-engine/jit-compiler/src/models/protocols/mod.rs) definition in `jit_compiler`).

For examples of configurations see below.

**TIP:** You can use `nada list-protocols` to check the instructions (protocols) currently implemented in the VM.

# Usage

The main structure is `ProgramAuditor`. The constructor takes an instance of `ProgramAuditorConfig`. The `audit` method is invoked with an instance of `ProgramMIR`.

```rust
let auditor = ProgramAuditor::new(config);
let auditor_request = ProgramAuditorRequest::from_mir(&mir)?;
let audit_result = auditor.audit(auditor_request);
```

This is what the configuration looks like:

```rust
ProgramAuditorConfig {
        max_memory_size: 100,
        max_instructions: vec![("Addition".to_string(), 100u64), ("MultiplicationShares".to_string(), 100u64)]
            .into_iter()
            .collect(),
        max_preprocessing: ProgramRequirements::default()
            .with_compare_elements(10)
            .with_division_integer_secret_elements(10)
            .with_equals_integer_secret_elements(10)
            .with_modulo_elements(10)
            .with_public_output_equality_elements(10)
            .with_trunc_elements(10)
            .with_truncpr_elements(10),
    }
```

In YAML:

```yaml
max_memory_size: 100
max_instructions:
  Addition: 100
  MultiplicationShares: 100
max_preprocessing:
  runtime_elements:
    TruncPr: 10
    Trunc: 10
    Modulo: 10
    DivisionIntegerSecret: 10
    PublicOutputEquality: 10
    Compare: 10
    EqualsIntegerSecret: 10
```
