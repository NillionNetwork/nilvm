# The Nada Type System

## `NadaType`

`NadaType` represents a data type in Nada. It is an enumerated type that contains variants for every single type
in Nada. Nada types can be categorised in several ways:

- Scalar vs Compound
- Public vs Secret
- Shamir types vs non Shamir types

NadaType is used to define the data type. It is closely related to `NadaValue`. `NadaValue` is an enumerated type that represents values in Nada alongside its type.

### Scalar types

Scalar types represent simple numerical types. In Nada, scalar types can be:

- Public: the value contained is publicly visible to all the nodes involved in the computation
- Secret: The value is hidden from the compute nodes.
- ShamirShare types: Represent Shamir shares

The table below summarises the list of NadaType variants:

| Type                         | Description                                                         |
| ---------------------------- | ------------------------------------------------------------------- |
| `Integer`                    | Public signed integer                                               |
| `UnsignedInteger`            | Public unsigned integer                                             |
| `Boolean`                    | Public boolean                                                      |
| `SecretInteger`              | Secret signed integer                                               |
| `SecretUnsignedInteger`      | Secret unsigned integer                                             |
| `SecretBoolean`              | Secret boolean                                                      |
| `SecretBlob`                 | A secret binary blob of data (used only in storage, not in compute) |
| `ShamirShareInteger`         | Shamir Share signed integer                                         |
| `ShamirShareUnsignedInteger` | Shamir Share unsigned integer                                       |
| `ShamirShareBoolean`         | Shamir Share boolean                                                |

### Compound types

Nada supports two kind of compound types: public-size Arrays and Tuples.

#### Arrays

Arrays are compound types that represent a container of elements of the same type. They are immutable and the size (number of elements in the Array) is a public constant.

In other words, Nada does not currently support mutable arrays (arrays whose content changes during the execution of a program). Also, Nada does not support arrays whose size is secret or non-constant.

An `Array` is defined by two elements:

- the `inner_type` property: the `NadaType` of the elements of the array
- the `size`: Number of elements of the `Array`.

##### Example

```
Array { inner_type: SecretInteger, size: 10 }
```

#### Tuples

Tuples represent containers of two elements. The type of the left and the right side of a tuple can be of a different type.

## `NadaValue`

`NadaValue` represents Nada typed instances of values. `NadaValue` is an enumerated type, and the variants correspond to the variants
of `NadaType`.

For instance, `SecretInteger` is the `NadaType`, while `NadaValue` is a `SecretInteger` with a value of `-23`. There are utilities
that allow creating new instances of `NadaValue` from primitives. For instance `NadaValue::new_secret_integer(-23)` would construct
the `NadaValue` in this example.

### `Clear` and `Encrypted`

Depending on the stage in a Nillion computation, a `NadaValue` can be in either `Clear` or `Encrypted` form. This is outlined in the generic parameters for `NadaValue`:

| Variant                         | Description                                                 |
| ------------------------------- | ----------------------------------------------------------- |
| `NadaValue<Clear>`              | The `NadaValue` in clear text form                          |
| `NadaValue<Encrypted<T>>`       | The `NadaValue` in Encrypted, modular number generic        |
| `NadaValue<Encrypted<Encoded>>` | The `NadaValue` in Encrypted and non-generic modular number |

When the Dealer node provides values, or the Result node collects result values from a computation, the values are provided in "clear text" form. That is, the dealer provides a type for the value and the value is in clear form. For Secret values, this means that the
value is provided in the clear. The secret marker is used by the Dealer, which will perform conversion of the secret value into shares. At this point their type becomes `NadaValue<Encrypted<_>`. These shares are sent to the compute nodes. The compute nodes never see `NadaValue<Clear>`, just `Encrypted`.

Conversely, when the result of a computation arrives at the `Result` node, the values are shares of type `NadaValue<Encrypted<Encoded>>`. The `Result` node decrypts and decodes the shares into `NadaValue<Clear>` that represent the final resulting, clear text values, of the computation.

There are two variants of `Encrypted`: Modular and Encoded. They reflect the fact that internally, in the compute nodes, the compuntations are performed with the shares in Modular representation. The `Encrypted<Encoded>` variant is used to safely deliver values between the compute nodes and the dealer / result nodes.
