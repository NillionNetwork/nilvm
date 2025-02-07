# nada-value

Crate that models the data format behind nada.

The core type is `NadaValue` that represents a concrete value in nada. It is generic over the state of the data, it can
be `Clear` or `Encrypted`.
Also it models the different types supported by nada like SecretInteger, Integer, Bool, SecretBool.
Another key type is `NadaType` that represents the type of a value in nada, `NadaType` differs from `NadaValue` in that
it doesn't contain a concrete value it is only the type.