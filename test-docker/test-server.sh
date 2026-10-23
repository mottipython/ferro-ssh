#!/usr/bin/env bash
# Helper script to manage the FerroSSH test SSH container

CONTAINER_NAME="ferro-ssh-test"
PORT=2222

case "$1" in
    start)
        if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
            echo "Starting existing container ${CONTAINER_NAME}..."
            docker start "${CONTAINER_NAME}"
        else
            echo "Running new container ${CONTAINER_NAME} on port ${PORT}..."
            docker run -d --name "${CONTAINER_NAME}" -p "${PORT}:22" --restart unless-stopped ferro-ssh-test:latest
        fi
        echo "Test SSH server is running on 127.0.0.1:${PORT}"
        ;;
    stop)
        echo "Stopping ${CONTAINER_NAME}..."
        docker stop "${CONTAINER_NAME}"
        ;;
    status)
        docker ps -a --filter "name=${CONTAINER_NAME}"
        ;;
    logs)
        docker logs -f "${CONTAINER_NAME}"
        ;;
    reset-hostkey)
        echo "Regenerating SSH host keys inside container to re-trigger unknown fingerprint prompt..."
        docker exec "${CONTAINER_NAME}" rm -f /etc/ssh/ssh_host_*
        docker exec "${CONTAINER_NAME}" ssh-keygen -A
        docker restart "${CONTAINER_NAME}"
        # Remove old cached entry from local known_hosts if present
        ssh-keygen -R "[127.0.0.1]:${PORT}" 2>/dev/null || true
        echo "Host keys reset. Connecting now will trigger the TOFU fingerprint dialog again."
        ;;
    *)
        echo "Usage: $0 {start|stop|status|logs|reset-hostkey}"
        exit 1
        ;;
esac
