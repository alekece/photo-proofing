# syntax=docker/dockerfile:1
# Dev-oriented: serves the SvelteKit dev server (the project has no production adapter yet).
FROM node:22-bookworm-slim
RUN corepack enable && corepack prepare pnpm@10.13.1 --activate
WORKDIR /app
COPY web/package.json web/pnpm-lock.yaml web/pnpm-workspace.yaml web/.npmrc ./
RUN --mount=type=cache,target=/root/.local/share/pnpm/store pnpm install --frozen-lockfile
COPY web ./
ENV EG_API_PROXY_TARGET=http://api:8080
EXPOSE 5173
HEALTHCHECK --interval=5s --timeout=3s --start-period=15s --retries=10 \
    CMD node -e "fetch('http://127.0.0.1:5173/').then(r=>process.exit(r.ok?0:1),()=>process.exit(1))"
CMD ["pnpm", "exec", "vite", "dev", "--host", "0.0.0.0", "--port", "5173", "--strictPort"]
