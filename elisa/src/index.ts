import * as Sentry from "@sentry/bun";

Sentry.init({
    dsn: Bun.env.SENTRY_DSN,
    tracesSampleRate: 1.0,
    environment: Bun.env.NODE_ENV,
})

import { Elysia, t } from "elysia";
import swagger from "@elysiajs/swagger";

const app = new Elysia({
    precompile: true
})
    .use(swagger())
    .trace(async ({ onHandle, context }) => {
        console.log("Route", context.path);
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
    .put("/upload",
        async function* ({ body: { name, file } }) {
            const fileDescriptor = Bun.file(`${Bun.env.MODULES_DOWNLOAD}/${name}`);
            var percentage = 0;
            var current_copied = 0;
            for await (const chunk of file.stream()) {
                await Bun.write(fileDescriptor, chunk);
                current_copied += chunk.length;
                percentage = Math.floor((current_copied / file.size) * 100);
                console.log("Percentage:", percentage);
                yield { percentage };
            }
        },
        {
            body:
                t.Object({
                    name: t.String(),
                    file: t.File()
                })
        }
    )
    .listen(3000);

console.log(
    `🦊 Elysia is running at ${app.server?.hostname}:${app.server?.port}`
);
