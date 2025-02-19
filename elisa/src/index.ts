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
                Sentry.captureMessage(`Request ${context.path} handled in ${end - begin}ms`);
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
            const path = `${Bun.env.MODULES_DOWNLOAD}/${name}`;
            const checkingFile = Bun.file(path);
            try {
                await checkingFile.delete();
            } catch (error) {
                console.log(error);
            }
            var percentage = 0;
            var current_copied = 0;
            const fileDescriptor = Bun.file(path);
            await Bun.write(fileDescriptor, "");
            const writer = fileDescriptor.writer();
            for await (const chunk of file.stream()) {
                writer.write(chunk);
                current_copied += chunk.length;
                percentage = Math.floor((current_copied / file.size) * 100);
                console.log("Percentage:", percentage);
                yield { percentage };
            }
            writer.flush();
            writer.end();
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
