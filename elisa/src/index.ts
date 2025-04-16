import * as Sentry from "@sentry/bun";

Sentry.init({
    dsn: Bun.env.SENTRY_DSN,
    tracesSampleRate: 1.0,
    environment: Bun.env.NODE_ENV,
})

import { Elysia, t } from "elysia";
import swagger from "@elysiajs/swagger";

const ARTIFACTS_PATH = Bun.env.ARTIFACTS_PATH || "./artifacts";

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
            Sentry.addBreadcrumb({
                category: 'upload',
                message: `Starting upload for file: ${name}`,
                level: 'info',
            });

            const path = `${Bun.env.MODULES_DOWNLOAD}/${name}`;
            const checkingFile = Bun.file(path);
            try {
                await checkingFile.delete();
                Sentry.addBreadcrumb({
                    category: 'upload',
                    message: `Deleted existing file: ${name}`,
                    level: 'info',
                });
            } catch (error) {
                Sentry.addBreadcrumb({
                    category: 'upload',
                    message: `No existing file to delete: ${name}`,
                    level: 'info',
                });
                Sentry.captureException(error);
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
                if (percentage % 25 === 0) { // Add breadcrumb every 25%
                    Sentry.addBreadcrumb({
                        category: 'upload',
                        message: `Upload progress: ${percentage}%`,
                        level: 'info',
                        data: {
                            percentage,
                            bytes_copied: current_copied,
                            total_size: file.size
                        }
                    });
                }
                yield { percentage };
            }

            writer.flush();
            writer.end();
            Sentry.captureMessage(`Upload completed for file: ${name}`);
        },
        {
            body:
                t.Object({
                    name: t.String(),
                    file: t.File()
                })
        }
    )
    .put("/artifact/:name", async function* ({ params, body }) {
        console.log("Uploading artifact", params.name);
        Sentry.addBreadcrumb({
            category: 'upload',
            message: `Starting upload for artifact: ${params.name}`,
            level: 'info',
        });
        const path = `${ARTIFACTS_PATH}/${params.name}`;
        const checkingFile = Bun.file(path);
        try {
            await checkingFile.delete();
            Sentry.addBreadcrumb({
                category: 'upload',
                message: `Deleted existing artifact: ${params.name}`,
                level: 'info',
            });
        } catch (error) {
            Sentry.addBreadcrumb({
                category: 'upload',
                message: `No existing artifact to delete: ${params.name}`,
                level: 'info',
            });
            Sentry.captureException(error);
        }
        console.log("Deleting existing artifact", params.name);
        var percentage = 0;
        var current_copied = 0;
        const fileDescriptor = Bun.file(path);
        await Bun.write(fileDescriptor, "");
        const writer = fileDescriptor.writer();
        console.log("Writing to", path);
        console.log("Writer generated")

        for await (const chunk of body.file.stream()) {
            writer.write(chunk);
            current_copied += chunk.length;
            percentage = Math.floor((current_copied / body.file.size) * 100);
            Sentry.addBreadcrumb({
                category: 'upload',
                message: `Upload progress: ${percentage}%`,
                level: 'info',
                data: {
                    percentage,
                    bytes_copied: current_copied,
                    total_size: body.file.size
                }
            });
            yield { percentage };
        }

        writer.flush();
        writer.end();
        Sentry.captureMessage(`Upload completed for artifact: ${params.name}`);
    }, {
        params: t.Object({ name: t.String() }),
        body: t.Object({ file: t.File() })
    })
    .get("/artifact/:name", async ({ params, set }) => {
        const { name } = params;
        const artifactPath = `${ARTIFACTS_PATH}/${name}`;
        Sentry.addBreadcrumb({
            category: 'artifact-download',
            message: `Download requested for artifact: ${name}`,
            level: 'info',
        });
        try {
            const file = Bun.file(artifactPath);
            if (!(await file.exists())) {
                Sentry.addBreadcrumb({
                    category: 'artifact-download',
                    message: `Artifact not found: ${name}`,
                    level: 'error',
                });
                set.status = 404;
                return { error: "Artifact not found" };
            }
            set.headers["Content-Disposition"] = `attachment; filename=\"${name}\"`;
            set.headers["Content-Type"] = "application/octet-stream";
            Sentry.addBreadcrumb({
                category: 'artifact-download',
                message: `Artifact found and sent: ${name}`,
                level: 'info',
            });
            return file.stream();
        } catch (error) {
            Sentry.captureException(error);
            set.status = 500;
            return { error: "Internal server error" };
        }
    }, {
        params: t.Object({ name: t.String() })
    })
    .post("/sentry/notify", ({ body }) => {
        Sentry.captureMessage(body.message);
        return "OK";
    }, {
        body:
            t.Object({
                message: t.String(),
            })
    })
    .listen(3000);

console.log(
    `🦊 Elysia is running at ${app.server?.hostname}:${app.server?.port}`
);
