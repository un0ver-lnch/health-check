import * as Sentry from "@sentry/bun";

Sentry.init({
    dsn: process.env.SENTRY_DSN,
    tracesSampleRate: 1.0,
    environment: process.env.NODE_ENV,
})

import { Elysia } from "elysia";
import { artifacts } from "./artifacts";
import swagger from "@elysiajs/swagger";

const app = new Elysia()
    .use(swagger())
    .trace(async ({ onHandle }) => {
        onHandle(({ begin, onStop }) => {
            onStop(({ end }) => {
                console.log("Request handled in", end - begin, "ms");
            })
        })
    })
    .onError(({ error, code }) => {
        console.log("Error", code);
        switch (code) {
            case "NOT_FOUND":
                console.log(error);
                return;
            default:
                console.log(error);
                Sentry.captureException(error);
        }
    })
    .get("/healthz", () => "OK")
    .get("/", () => "Hello Elysia")
    .mount('/artifacts', artifacts.fetch)
    .listen(3000);

console.log(
    `🦊 Elysia is running at ${app.server?.hostname}:${app.server?.port}`
);
