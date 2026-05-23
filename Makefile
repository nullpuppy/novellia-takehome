IMAGE_NAME ?= novellia-takehome
CONTAINER_NAME ?= novellia-takehome
PORT ?= 3100
DATASET ?= data/backend-takehome-fhir-resources.jsonl

.PHONY: docker-build docker-run docker

docker-build:
	docker build -t $(IMAGE_NAME) .

docker-run:
	@if docker ps --format '{{.Names}}' | grep -qx "$(CONTAINER_NAME)"; then \
		docker stop "$(CONTAINER_NAME)" >/dev/null; \
	fi

	@if docker ps -a --format '{{.Names}}' | grep -qx "$(CONTAINER_NAME)"; then \
		docker rm "$(CONTAINER_NAME)" >/dev/null; \
	fi

	@if [ ! -f "$(DATASET)" ]; then \
		echo "Dataset $(DATASET) not found" >@2; \
		exit 1; \
  	fi

	@DATASET_PATH="$$(cd "$$(dirname "$(DATASET)")" && pwd)/$$(basename "$(DATASET)")"; \
	DATASET_NAME="$$(basename "$(DATASET)")"; \
	docker run --rm \
	--name "$(CONTAINER_NAME)" \
	-p $(PORT):3100 \
	-v "$${DATASET_PATH}:/data/$${DATASET_NAME}:ro" \
	$(IMAGE_NAME) "/data/$${DATASET_NAME}"

docker: docker-build docker-run

build-debug:
	cargo build --debug

run-debug:
	./target/debug/novellia-takehome $(DATASET)

build-release:
	cargo build --release

run-release:
	./target/release/novellia-takehome $(DATASET)

test:
	cargo test
