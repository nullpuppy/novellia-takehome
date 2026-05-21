# Novellia Takehome Coding Challenge

A small Rust/Axum API that loads JSONL files containing FHIR resources into a
basic in-memory data-store. This API is largely focused on patients
at the moment and allows looking up each patient resource individually for
a given patient.

Additional points of interest, a patient timeline has been implemented
and returns a date-ordered listing of all patient resources starting
from the most recent. Also, an audit of parsed/loaded resources is
done during dataset loading. Audit results are preserved in-memory and retrievable via
 their own endpoint.

## Tech stack

- [Rust](http://www.rust-lang.org) - language/build environment
- [Axum](https://docs.rs/axum/latest/axum/) – HTTP routing and serving resources
- [Tower](https://docs.rs/tower/latest/tower/) - http middleware for tracing/logging, path normalization, and panic recovery
- [Tokio](https://docs.rs/tokio/latest/tokio/) – async runtime
- [Serde](https://docs.rs/serde/latest/serde)/[serde_json](https://docs.rs/serde_json/latest/serde_json/) - JSON serialization, deserialization
- [Base64](https://docs.rs/base64/latest/base64) - Base64 resource decoding

I chose this stack as I've been working using rust for side projects, I'm
comfortable working with Rust and in that ecosystem. I've been using rust for
side-projects for several years now and enjoy coding with it as much as possible.

## Running locally

Requires [Rust/Cargo](http://rust-lang.org) - v1.95 was used during development.

If you do not have Rust installed, the project can also be built and run with Docker.

### Docker

A script (`run-docker.sh`) and a `Makefile` are both present for running via docker.

#### `run-script.sh`
```bash
# Build the docker image and run the API using the default dataset
> ./run-docker.sh
# Or run with another dataset included in the repo
> ./run-docker.sh data/generated-fhir-resources.jsonl
```
#### `Makefile`
```bash
# Build docker image
> make docker-build
# Run image
> make docker-run
# Build, and then run
> make docker
```

To build and run manually:
```bash
> docker build -t novellia-takehome .
> docker run --rm -p 3100:3100 novellia-takehome
```


The API will be available at
```bash
http://localhost:3100
```

to run with alternative datasets, place it in the data/ directory and start the container with the path to the dataset.

### With local Rust

```bash
> cd /path/to/repo
> cargo run
> cargo run -- <path/to/alternative/dataset> # default dataset is ./data/backend-takehome-fhir-resources.jsonl
```

```bash
# Full set of commands for building, testing, and running from built binary.
# Build binary. 
#   In debug mode, the binary will be at ./target/debug/novellia-takehome
#   In release mode, the binary will be at ./target/release/novellia-takehome
> cargo build
> cargo build --release

# Optionally, run tests
> cargo test

# Run the service with default dataset
> ./target/debug/novellia-takehome 
> ./target/debug/novellia-takehome <path/to/dataset>
# or
> ./target/release/novellia-takehome 
> ./target/release/novellia-takehome <path/to/dataset>
```

## API overview

This API was modeled around patient review workflows; first, find a patient, then
inspect their clinical history by resource type or as a unified timeline.

### Patients
- http://localhost:3100/patients 
- http://localhost:3100/patients/<patient_id>

### Patient clinical resources
- http://localhost:3100/patients/<patient_id>/conditions 
- http://localhost:3100/patients/<patient_id>/conditions/{condition_id}
- http://localhost:3100/patients/<patient_id>/medications 
- http://localhost:3100/patients/<patient_id>/medications/{medication_id}
- http://localhost:3100/patients/<patient_id>/observations 
- http://localhost:3100/patients/<patient_id>/observations/{observation_id}
- http://localhost:3100/patients/<patient_id>/procedures
- http://localhost:3100/patients/<patient_id>/procedures/{procedure_id}

### Patient Documents
- http://localhost:3100/patients/<patient_id>/documents 
- http://localhost:3100/patients/<patient_id>/documents/<document-id> 

### Patient timeline/history
- http://localhost:3100/patients/<patient_id>/timeline 

### Binaries.
- http://localhost:3100/binaries/
- http://localhost:3100/binaries/<binary_id>

### Problems in ingested data quality
- http://localhost:3100/data-quality

Real world data is always messy and often unreliable, malformed, or
just incomplete, and depending on how data gets to you can change
wildly if updates come in. As such, while parsing the JSONL input,
parsing errors are added to the collection of data quality issues.
After parsing is complete, every top-level resource is audited
for required fields, resource linking between some resources, etc.

- JSON parse errors
- missing required fields
- invalid field formats
- unresolvable references
- mismatch references due to the wrong case pattern
- duplicate/amended observation
- non-standard or unknown resource types

All parsed resources are still loaded regardless of data quality issues.

### API testing via curl (using data from the provided data set)

```bash
> # happy path
> curl http://localhost:3100/patients/noah-wyle 
> curl http://localhost:3100/patients/NOAH-WYLE/conditions 
> curl http://localhost:3100/patients/NoAh-WyLe/medications 
> curl http://localhost:3100/patients/NOAH-wyle/observations 
> curl http://localhost:3100/patients/noah-WYLE/procedures 
> curl http://localhost:3100/patients/noah-wyle/documents 
> curl http://localhost:3100/patients/noah-wyle/documents/docref-001
> curl http://localhost:3100/patients/noah-wyle/timeline 
> curl http://localhost:3100/binaries
> curl http://localhost:3100/binaries/binary-001
```

```bash
> # failure examples
> curl http://localhost:3100/patients/patient-does-not-exist # 404
> curl http://localhost:3100/patients/noah-wyle/conditions/cont-nw-001 # 404
> curl http://localhost:3100/patients/noah-wyle/medicat # 404
> curl http://localhost:3100/patients/no-user/documents # 404 
> curl http://localhost:3100/patients/noah-wyle/documents/docref-002 # 404
> curl http://localhost:3100/patients/nw/timeline # 404
> curl http://localhost:3100/binaries/bin-001 # 404
```

## Architecture notes

All parsable resources are loaded into memory at startup.
- `fhir.rs` contains FHIR-shaped input models
- `store.rs` loads, parses, and indexes resources by normalized (lowercased) patient id and binary id
- `audit.rs` audits resources for data quality before indexing
- `route.rs` single source of truth for all routes
- `api/binary.rs` contains binary route handlers
- `api/patient/models.rs` contains API response DTOs
- `api/patient/document.rs` document summary and content handling
- `api/patient/timeline.rs` handles resource conversion and collection for the timeline
- `api/patient.rs` contains patient route handlers

## Error handling

All request-ending errors return JSON-formatted errors.
Invalid resources related to document loading are raised as
Bad Request errors (status 400). Panics are recovered by middleware and
will return Internal Service Error errors (status 500).
Not Found errors are returned as 404's.

## Tradeoffs / Future work

### Data modeling and validation
- Parse dates/datetimes into proper temporal types instead of strings
- Expand supported FHIR resource types and field coverage.
- Add more extensive parsing and auditing support.

### Querying and retrieval
- Add pagination, caller-controlled results per page, and caller-controlled sort order.
- Add patient/resource search.
- Index orphaned resources and make them retrievable.
- Allow lookup of arbitrary resources. (e.g., parsed, unknown resources)

### Operations and production readiness
- Add authentication/authorization.
- Stream document content instead of loading it fully into memory.
- Support adding resources while the service is running.
- Stream input loading instead of reading the whole file into memory.
- Automate OpenAPI generation.

## AI usage

### AI Tools:
- JetBrains AI Assistant. Some claude chats, as well as whatever JetBrains AI Assistant's model is
  - code review
  - naming/refactoring discussions
  - test coverage and documentation discussions and drafting
  - reasoning FHIR models and parsing sample data to generate a base schema

Some code generation was done in chat with the above discussions, but there wasn't any use of agent-driven code
generation, and only a single, small, usage of agent editing. Agent mode was used to change the field visibility
on the [FHIR structs](src/fhir.rs) as I had initially written them with the wrong visibility. (All the agent did
was add pub to the beginning of each field on those structs.)

- Postman AI
  - Generated openapi.yaml by inspecting the code base.
    - I suppose this could be considered a second use of an agent, but not for code generation unless you
    consider YAML to be code.
