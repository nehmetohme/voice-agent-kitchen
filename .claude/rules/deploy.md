---
paths:
  - "deploy/**"
---

# Deploy

- Infrastructure as code only: the Databricks bundle (`deploy/databricks/`) and the local stack (`deploy/local/`).
- Production runs only on Databricks: Apps for hosting, Model Serving for the LLM, STT and TTS, and Lakebase for Postgres. Local development uses local models and Postgres behind the same interfaces, switched by configuration only.
- No secrets in files. CI authenticates to Databricks with OIDC. Local secrets live in gitignored `.env` files.
- Prod changes deploy only from CI, from a signed release tag. Staging deploys from `main`.
- The local stack starts with one command (`just dev` or `just dev-light`) and stays in step with the bundle.
