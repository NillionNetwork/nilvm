# Node Operator Questionnaire

## Requirements

### Application

#### Certificate Provisioning

_A Nillion node runs a gRPC server which requires certificates signed by a trusted authority to
establish TLS connections. Do you have the infrastructure to provision and renew certificates signed
by a trusted root or intermediate CA?_

#### Container Orchestration

_A Nillion node is a single (Rust) binary packaged into a Docker image built for Linux/amd64 and
aarch64 platforms. Do you have the infrastructure to run container workloads for one of these
platforms?_

#### DNS

_A Nillion node requires a DNS record from a public zone. Do you have the infrastructure to
provision public DNS records and associate them with a node? Do you have any custom resolver
settings or cache policies that may affect the freshness of your records?_

#### Block Storage

_A Nillion node uses a Sqlite database for bookkeeping and temporary data. This data must persisted
on an encrypted block storage volume that survives restarts/upgrades. Do you have infrastructure
that meets these requirements?_

#### Network Ports

_A Nillion node runs a gRPC server on port `14311` (default) and exposes Prometheus metrics on
`34111` (default). The gRPC server needs to be exposed publicly. Can you configure your
infrastructure to meet these requirements?_

#### Object Storage

_A Nillion node requires an S3 API compatible object storage system such as [MinIO](https://min.io)
to store pre-processing elements, programs and user data. Do you have infrastructure to meet this
requirement? Describe your backup/restore procedures and replication strategy._

### Hardware

_Nillion runs its own testnet/mainnet nodes with `c6i.xlarge` EC2 instances which have 4 vCPUs, 8
GiB memory, up to 12.5 Gbps network bandwidth and up to 10 Gbps disk bandwidth. Can you run virtual
or bare metal machines at least as large as this on your infrastructure?_

### Hosting

#### Geo-Location

_A Nillion Network requires geo-diversity. Please describe the geo-locations you support._

#### Hosting Provider

_A Nillion Network requires host diversity. This can be a public or private cloud, or self-owned or
rented datacenter space. Nillion runs its own nodes on AWS. Can you share which providers you use?_

### Observability

#### Log Collection and Storage

_A Nillion node logs to stdout/stderr. Logs must be stored for 30 days for offline inspection. Do
you have the infrastructure to collect, store and secure them? Are they shipped to and stored by a
3rd party?_

#### Metrics Collection and Storage

_A Nillion node serves Prometheus metrics on port `34111`. These metrics as well as system, VM, and
container metrics should be collected and stored for 30 days for offline inspection. Do you have the
infrastructure to collect, store and secure them? Are they shipped to and stored by a 3rd party?_

### Reliability and Security

#### Asset Management

_A Nillion node must be operated within a confidential and tamper-proof environment. Do you run nodes
on shared infrastructure? At which levels -- e.g. container, VM, network -- do you have the ability
to isolate workloads?_

#### Data Security

_A Nillion node must have its data encrypted in-transit and at-rest.  Do you guarantee
confidentiality, integrity and availability? How do you manage cryptographic keys? Please describe
your encryption strategy/implementation._

#### Identity and Access Management

_A Nillion node must not be susceptible to unexpected or unfettered access by an operator. How do
you manage identities and credentials in your infrastructure? Do you enforce phishing resistant
multi-factor authentication, least privilege access, role-based access control, re-authentication
and session expiration? What is your process for joiners/movers/leavers?_

#### Incident Detection and Response

_Node operators must be available in a timely manner during service-impacting or security incidents.
How quickly can you respond? Do you monitor networks and services to detect potential outages and
malicious events? Do you monitor identity and cryptographic key usage to detect potentially
unauthorized activity?_

#### Node Availability

_A Nillion node must be highly available. How do you guarantee resilience during normal and adverse
situations?_

#### Remote Access

_A Nillion node must not be susceptible to unexpected or unfettered remote access. How do you manage
physical and logical access to networks? How do you logically create trust boundaries within your
networks? Do you logically separate external networks from internal networks?_

#### Risk Assessment

_Node operators must be aware of vulnerabilities present within their infrastructure. Do you have
any vulnerabililty management software running in hosts?_
