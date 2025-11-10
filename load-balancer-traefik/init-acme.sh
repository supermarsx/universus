#!/bin/sh
# Initializes acme.json for Traefik Let's Encrypt cert storage
# Usage: ./init-acme.sh
set -e
if [ ! -f acme.json ]; then
  touch acme.json
  chmod 600 acme.json
  echo "acme.json created with permissions 600."
else
  echo "acme.json already exists."
fi
