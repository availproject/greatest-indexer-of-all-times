FROM node:20-slim AS base

RUN corepack enable && corepack prepare pnpm@9 --activate

FROM base AS builder

WORKDIR /app

COPY package.json pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile

COPY . .

FROM base AS runner

WORKDIR /app

COPY --from=builder /app .

ENV NODE_ENV=production

EXPOSE 42069

CMD ["pnpm", "start"]
