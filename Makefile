IMAGE_NAME ?= novellia-takehome
PORT ?= 3100
DATASET ?= data/backend-takehome-fhir-resources.jsonl

.PHONY: docker-build docker-run docker

docker-build:
	docker build -t $(IMAGE_NAME) .

docker-run:
	docker run --rm -p $(PORT):3100 $(IMAGE_NAME) $(DATASET)

docker: docker-build docker-run
