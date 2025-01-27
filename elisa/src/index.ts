import * as Sentry from "@sentry/bun";

Sentry.init({
    dsn: process.env.SENTRY_DSN,
    tracesSampleRate: 1.0,
    environment: process.env.NODE_ENV,
})

import { Elysia } from "elysia";
import Artifacts from "./artifacts";

const app = new Elysia()
    .trace(async ({ onHandle }) => {
        onHandle(({ begin, onStop }) => {
            onStop(({ end }) => {
                console.log("Request handled in", end - begin, "ms");
            })
        })
    })
    .onError(({ error, code }) => {
        switch (code) {
            case "NOT_FOUND":
                return;
            default:
                Sentry.captureException(error);
        }
    })
    .mount('/artifacts', Artifacts.fetch)
    .get("/", () => "Hello Elysia")
    .listen(3000);

console.log(
    `🦊 Elysia is running at ${app.server?.hostname}:${app.server?.port}`
);
