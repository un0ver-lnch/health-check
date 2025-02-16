import swagger from "@elysiajs/swagger";
import Elysia, { t } from "elysia";

const artifacts = new Elysia()
    .use(swagger())
    .get("/healthz", () => "OK")
    .post("/upload",
        async ({ body }) => {
            console.log(body.name, body.description);
            return "Uploaded"
        },
        {
            body: t.Object({
                name: t.String({ description: "Name of the artifact" }),
                description: t.String({ description: "Description of the artifact" }),
            }, { description: "Body to manage the file uploads", title: "Artifacts Upload" })
        }
    )


export {
    artifacts
};