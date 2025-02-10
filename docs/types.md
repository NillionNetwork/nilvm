```mermaid
flowchart TD
    NadaType["NadaType { String, Integer, UnsignedInteger, Rational{digits}... }"]
    NadaType -- contains --> Secret["Secret(NadaType)"]
    NadaType -- contains --> Public["Public(NadaType)"]
    NadaType -- contains --> EncodedSecret["EncodedSecret(NadaType)"]
    NadaType -- contains --> EncodedPublic["EncodedPublic(NadaType)"]
    Variable["Variable { Secret, Public }"]
    Secret --> Variable
    Public --> Variable
    EncodedVariable["EncodedVariable { EncodedSecret, EncodedPublic }"]
    EncodedSecret --> EncodedVariable
    EncodedPublic --> EncodedVariable
    Variable <-- " encode/decode() " --> EncodedVariable
    Secret <-- " encode/decode() " --> EncodedSecret
    Public <-- " encode/decode() " --> EncodedPublic
    SecretPublicOperations["+, *"]
    EncodedPublic --> SecretPublicOperations
    EncodedSecret --> SecretPublicOperations
    SecretPublicOperations --> NewSecret["new EncodedSecret"]
    EncodedSecrets -- " map of " --> EncodedSecret
    SecretSecretOperations["+, *"]
    EncodedSecret -- 2x --> SecretSecretOperations
    SecretSecretOperations --> NewSecret["new EncodedSecret"]
    PublicPublicOperations["+, -, *, /, %, &lt;&lt;, &gt;&gt;"]
    Public -- 2x --> PublicPublicOperations
    PublicPublicOperations --> NewPublic["new Public"]
    UnitSecret <-- " exchangeable " --> ModularNumber
    EncodedSecret -- " one or more " --> UnitSecret
    EncodedUnitSecret <-- " exchangeable " --> EncodedModularNumber
    UnitSecret <-- " encode/decode() " --> EncodedUnitSecret
    ModularNumber <-- " encode/decode() " --> EncodedModularNumber
    EncodedModularNumber -- " contains " --> EncodedModulo
    classDef encoded stroke: #dd0000;
    classDef op stroke: #00dd00;
    class EncodedModulo, EncodedVariable, EncodedSecret, EncodedPublic, EncodedUnitSecret, EncodedModularNumber, EncodedSecrets, NewSecret encoded;
    class SecretSecretOperations, PublicPublicOperations, SecretPublicOperations op;
```

| FLOW               | client                                                                 |                                                             nodes |                                                                   |
|--------------------|------------------------------------------------------------------------|------------------------------------------------------------------:|-------------------------------------------------------------------|
| STATE              | plain text inputs                                                      |                                           encrypted inputs w/ bfs | encrypted without bfs                                             |
| CURRENT NAMES      | store_values(Secret)<br/>compute(Secret, PublicVariable)               |                                                                   |                                                                   |
| CURRENT COLLECTION | store_values(Secrets)<br/>compute(Variables{Secrets, PublicVariables}) |                                                                   |                                                                   |
| NAMES              | NadaValue<Clear<T>>  NadaValue<Clear<Encoded>>                         | NadaValue<Encrypted<T: SafePrime>>  NadaValue<Encrypted<Encoded>> | NadaValue<Encrypted<T: SafePrime>>  NadaValue<Encrypted<Encoded>> |
| COLLECTIONS        | HashMap<K, NadaValue<Clear<T>>>                                        |                    HashMap<K, NadaValue<Encrypted<T: SafePrime>>> |                                                                   |                                                                   |
