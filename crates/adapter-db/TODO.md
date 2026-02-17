# adapter-db TODO

- Implement concrete adapters for Postgres, MySQL, and a JSON file loader.
- Surface a unified transaction trait that services can call regardless of backend.
- Provide runtime config (env/JSON) to auto-register adapters for each tenant.
