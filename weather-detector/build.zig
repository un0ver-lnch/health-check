const std = @import("std");
const builtin = @import("builtin");

const targets: []const std.Target.Query = &.{ .{ .cpu_arch = .wasm64, .os_tag = .wasi }, .{ .cpu_arch = .wasm32, .os_tag = .wasi }, .{ .cpu_arch = builtin.cpu.arch, .os_tag = builtin.os.tag, .abi = .gnu } };

pub fn build(b: *std.Build) !void {
    b.enable_wasmtime = true;

    for (targets) |target| {
        const exe = b.addExecutable(.{
            .name = "weather_decider_run",
            .root_source_file = b.path("src/main.zig"),
            .target = b.resolveTargetQuery(target),
        });

        const target_output = b.addInstallArtifact(exe, .{
            .dest_dir = .{
                .override = .{
                    .custom = try target.zigTriple(b.allocator),
                },
            },
        });
        // if null it means that is native
        if (builtin.cpu.arch == target.cpu_arch) {
            const run_exe = b.addRunArtifact(exe);
            const run_step = b.step("run", "Run the application");
            run_step.dependOn(&run_exe.step);
        }

        b.getInstallStep().dependOn(&target_output.step);
    }
}
