FROM oven/bun:1 as builder
WORKDIR /app

# Copy package files
COPY package.json bun.lockb ./

# Install dependencies
RUN bun install

# Copy source code
COPY src ./src
COPY tsconfig.json ./

# Build if needed (uncomment if you have a build step)
RUN bun run build

FROM oven/bun:1-slim
WORKDIR /app

COPY --from=builder /app/package.json ./
RUN bun install --production

# Copy from builder
COPY --from=builder /app/dist ./dist

# Set environment variables
ENV NODE_ENV=production

# Expose port
EXPOSE 3000

# Run the application
CMD ["bun", "run", "start"]
