#!/usr/bin/env bash
set -euo pipefail

IMAGE_NAME="${IMAGE_NAME:-novellia-takehome}"
CONTAINER_NAME="${CONTAINER_NAME:-novellia-takehome}"
PORT=${PORT:-3100}
DATASET="${1:-data/backend-takehome-fhir-resources.jsonl}"

docker build -t "${IMAGE_NAME}" .

if docker ps -a --format '{{.Names}}' | grep -qx "${CONTAINER_NAME}"; then
  docker rm -f "${CONTAINER_NAME}" >/dev/null
fi

if [[ ! -f "${DATASET}" ]]; then
  echo "Dataset not found ${DATASET}" >&2
  exit
fi

DATASET_ABS="$(cd "$(dirname "${DATASET}")" && pwd)/$(basename "${DATASET}")"
DATASET_NAME="$(basename "${DATASET}")"

docker run --rm \
  --name "${CONTAINER_NAME}" \
  -p "${PORT}:3100" \
  -v "${DATASET_ABS}:/data/${DATASET_NAME}:ro" \
    "${IMAGE_NAME}" \
  "/data/${DATASET_NAME}"
